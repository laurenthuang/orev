use crate::config::{
    CaseConfig, RootConfig, StorageConfig, WorkspaceConfig, WorkspaceContext, WorkspaceInfo,
};
use crate::ops::init::{WORKSPACE_IGNORES, WORKSPACE_README, initialize_case};
use std::env;
use std::ffi::OsStr;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message.into())
}

fn check_movable_tree(path: &Path) -> io::Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() {
        return Err(invalid(format!(
            "conversion cannot relocate a symlink safely: `{}`; resolve its references before conversion",
            path.display()
        )));
    }
    if metadata.is_dir() {
        for entry in fs::read_dir(path)? {
            check_movable_tree(&entry?.path())?;
        }
    }
    Ok(())
}

fn is_shared_entry(filename: &std::ffi::OsStr) -> bool {
    [
        "orev.toml",
        "kb",
        "store",
        "tools",
        "corpus",
        ".git",
        ".orev-case-add.lock",
    ]
    .iter()
    .any(|reserved| filename == OsStr::new(reserved))
}

pub(super) fn validate_name(name: &str) -> io::Result<()> {
    if name.trim().is_empty()
        || matches!(name, "." | "..")
        || name
            .chars()
            .any(|c| c.is_control() || matches!(c, '/' | '\\' | ':'))
    {
        return Err(invalid(format!(
            "invalid case name `{name}`: expected a single directory name"
        )));
    }
    Ok(())
}

pub(super) fn read_config(config_root: &Path) -> io::Result<String> {
    let path = config_root.join("orev.toml");
    let metadata = fs::symlink_metadata(&path)?;
    if !metadata.file_type().is_file() {
        return Err(invalid(format!(
            "case management requires a regular configuration file: `{}`",
            path.display()
        )));
    }
    fs::read_to_string(path)
}

fn discover(start: &Path) -> io::Result<PathBuf> {
    for config_root in start.ancestors() {
        match fs::symlink_metadata(config_root.join("orev.toml")) {
            Ok(_) => {
                let text = read_config(config_root)?;
                return match RootConfig::parse(&text).map_err(invalid_data)? {
                    RootConfig::Workspace(_) => Ok(config_root.to_path_buf()),
                    RootConfig::Case(config) => match config.workspace_root {
                        Some(workspace_root) => fs::canonicalize(config_root.join(workspace_root)),
                        None => Ok(config_root.to_path_buf()),
                    },
                };
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
    }
    Err(invalid(
        "no orev.toml found in the current directory or its ancestors",
    ))
}

fn invalid_data(error: impl std::fmt::Display) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        format!("invalid orev.toml: {error}"),
    )
}

fn require_directory(path: &Path) -> io::Result<PathBuf> {
    let resolved = fs::canonicalize(path).map_err(|error| {
        io::Error::new(
            error.kind(),
            format!("cannot resolve directory `{}`: {error}", path.display()),
        )
    })?;
    if !resolved.is_dir() {
        return Err(invalid(format!(
            "expected a directory at `{}`",
            path.display()
        )));
    }
    Ok(resolved)
}

fn require_absent(path: &Path) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(_) => Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!("case destination already exists: `{}`", path.display()),
        )),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

pub fn run_add(name: &str) -> io::Result<()> {
    let result = (|| {
        validate_name(name)?;
        let config_root = discover(&env::current_dir()?)?;
        let lock = config_root.join(".orev-case-add.lock");
        fs::create_dir(&lock).map_err(|error| {
            io::Error::new(
                error.kind(),
                format!(
                    "cannot acquire workspace lock `{}`: {error}",
                    lock.display()
                ),
            )
        })?;
        let operation = add_locked(&config_root, name, replace_config);
        let cleanup = fs::remove_dir(&lock);
        if let Err(error) = &cleanup {
            eprintln!(
                "Error: Failed to remove workspace lock `{}`: {}",
                lock.display(),
                error
            );
        }
        operation?;
        cleanup?;
        println!(
            "Success! Case `{name}` created at `{}`.",
            config_root.join("cases").join(name).display()
        );
        Ok(())
    })();
    result.inspect_err(|error| eprintln!("Error: Failed to add case: {}", error))
}

fn workspace_case(name: &str) -> CaseConfig {
    let mut config = CaseConfig::new_case(name.to_string());
    config.workspace_root = Some(PathBuf::from("../.."));
    config
}

fn read_workspace(config_root: &Path) -> io::Result<(WorkspaceConfig, Option<CaseConfig>)> {
    let text = read_config(config_root)?;
    let (workspace, original) = match RootConfig::parse(&text).map_err(invalid_data)? {
        RootConfig::Workspace(workspace) => (workspace, None),
        RootConfig::Case(original) => {
            if original.workspace_root.is_some() {
                return Err(invalid(
                    "workspace_root must point to a multi-case workspace configuration",
                ));
            }
            validate_name(&original.case.name)?;
            (
                WorkspaceConfig {
                    workspace: WorkspaceInfo {
                        cases: vec![original.case.name.clone()],
                    },
                    storage: original.storage.clone(),
                },
                Some(original),
            )
        }
    };
    for registered in &workspace.workspace.cases {
        validate_name(registered)?;
    }
    Ok((workspace, original))
}

fn add_locked(
    config_root: &Path,
    name: &str,
    publish: impl FnOnce(&Path, &str) -> io::Result<()>,
) -> io::Result<()> {
    let (mut workspace, original) = read_workspace(config_root)?;
    if let Some(original) = &original {
        if original.case.name == name {
            return Err(invalid(
                "new case name conflicts with the original case name",
            ));
        }
        require_absent(&config_root.join("cases"))?;
    }
    let mut seen: Vec<&str> = Vec::new();
    for registered in &workspace.workspace.cases {
        if seen.contains(&registered.as_str()) {
            return Err(invalid(format!("duplicate registered case `{registered}`")));
        }
        seen.push(registered);
    }
    if workspace.workspace.cases.iter().any(|case| case == name) {
        return Err(invalid(format!("case `{name}` is already registered")));
    }
    let cases = config_root.join("cases");
    if original.is_none() {
        let metadata = fs::symlink_metadata(&cases)?;
        if !metadata.file_type().is_dir() {
            return Err(invalid("cases/ must be a real directory, not a symlink"));
        }
    }
    require_absent(&cases.join(name))?;
    let resources = workspace
        .resolve_resources(config_root)?
        .require_existing()?;
    let entries = if original.is_some() {
        fs::read_dir(config_root)?
            .map(|entry| entry.map(|entry| entry.path()))
            .collect::<io::Result<Vec<_>>>()?
    } else {
        Vec::new()
    };
    for entry in &entries {
        if !is_shared_entry(
            entry
                .file_name()
                .ok_or_else(|| invalid("entry has no name"))?,
        ) {
            check_movable_tree(entry)?;
        }
    }
    if original.is_some() {
        let resolved_config_root = require_directory(config_root)?;
        let mut shared_paths = vec![
            resources.kb.path().to_path_buf(),
            resources.store.path().to_path_buf(),
            resources.tools.path().to_path_buf(),
        ];
        for entry in &entries {
            if is_shared_entry(
                entry
                    .file_name()
                    .ok_or_else(|| invalid("entry has no name"))?,
            ) && fs::symlink_metadata(entry)?.file_type().is_symlink()
            {
                shared_paths.push(fs::canonicalize(entry)?);
            }
        }
        for shared in shared_paths {
            if let Ok(relative) = shared.strip_prefix(&resolved_config_root)
                && let Some(first) = relative.components().next()
                && !["kb", "store", "tools", "corpus", ".git"]
                    .iter()
                    .any(|name| first.as_os_str() == OsStr::new(name))
            {
                return Err(invalid(format!(
                    "shared asset `{}` points into data being relocated",
                    shared.display()
                )));
            }
        }
    }
    workspace.workspace.cases.push(name.to_string());
    let workspace_text = toml::to_string_pretty(&workspace).map_err(invalid_data)?;
    let mut transaction = Transaction::new(config_root)?;
    let result = (|| {
        let new = transaction.staging.path().join("new");
        fs::create_dir(&new)?;
        initialize_case(
            &new,
            workspace_case(name),
            Some(WorkspaceContext {
                root: config_root,
                config: &workspace,
            }),
        )?;
        if let Some(mut original) = original {
            let original_name = original.case.name.clone();
            let old = transaction.staging.path().join("original");
            fs::create_dir(&old)?;
            original.workspace_root = Some(PathBuf::from("../.."));
            original.storage = StorageConfig::default();
            fs::write(
                old.join("orev.toml"),
                toml::to_string_pretty(&original).map_err(invalid_data)?,
            )?;
            for path in entries {
                let filename = path
                    .file_name()
                    .ok_or_else(|| invalid("entry has no name"))?;
                if is_shared_entry(filename) {
                    continue;
                }
                transaction.move_path(&path, &old.join(filename))?;
            }
            fs::create_dir(&cases)?;
            transaction.created_cases = Some(cases.clone());
            transaction.move_path(&old, &cases.join(&original_name))?;
            for (filename, content) in [
                ("README.md", WORKSPACE_README),
                (".gitignore", WORKSPACE_IGNORES),
            ] {
                let staged = transaction.staging.path().join(filename);
                fs::write(&staged, content)?;
                transaction.move_path(&staged, &config_root.join(filename))?;
            }
        }
        transaction.move_path(&new, &cases.join(name))?;
        publish(&config_root.join("orev.toml"), &workspace_text)?;
        Ok(())
    })();
    match result {
        Ok(()) => transaction.staging.close().map_err(|error| {
            io::Error::new(
                error.kind(),
                format!("case creation committed, but staging cleanup failed: {error}"),
            )
        }),
        Err(error) => {
            transaction.rollback().map_err(|rollback| {
                io::Error::new(rollback.kind(), format!("{error}; {rollback}"))
            })?;
            Err(error)
        }
    }
}

fn replace_config(path: &Path, content: &str) -> io::Result<()> {
    let mut file = tempfile::NamedTempFile::new_in(
        path.parent()
            .ok_or_else(|| invalid("configuration has no parent"))?,
    )?;
    file.as_file()
        .set_permissions(fs::metadata(path)?.permissions())?;
    file.write_all(content.as_bytes())?;
    file.as_file().sync_all()?;
    file.persist(path).map_err(|error| error.error)?;
    Ok(())
}

struct Transaction {
    staging: tempfile::TempDir,
    moves: Vec<(PathBuf, PathBuf)>,
    created_cases: Option<PathBuf>,
}

impl Transaction {
    fn new(config_root: &Path) -> io::Result<Self> {
        Ok(Self {
            staging: tempfile::Builder::new()
                .prefix(".orev-case-stage-")
                .tempdir_in(config_root)?,
            moves: Vec::new(),
            created_cases: None,
        })
    }

    fn move_path(&mut self, from: &Path, to: &Path) -> io::Result<()> {
        require_absent(to)?;
        fs::rename(from, to).map_err(|error| {
            io::Error::new(
                error.kind(),
                format!(
                    "cannot move `{}` to `{}`: {error}",
                    from.display(),
                    to.display()
                ),
            )
        })?;
        self.moves.push((from.to_path_buf(), to.to_path_buf()));
        Ok(())
    }

    fn rollback(mut self) -> io::Result<()> {
        let mut failure = None;
        for (from, to) in self.moves.iter().rev() {
            if let Err(error) = require_absent(from).and_then(|()| fs::rename(to, from)) {
                eprintln!(
                    "Error: Rollback failed for `{}` -> `{}`: {}",
                    to.display(),
                    from.display(),
                    error
                );
                failure = Some(error);
                break;
            }
        }
        if failure.is_none()
            && let Some(cases) = &self.created_cases
            && let Err(error) = fs::remove_dir(cases)
        {
            failure = Some(error);
        }
        if let Some(error) = failure {
            let preserved = self.staging.keep();
            return Err(io::Error::new(
                error.kind(),
                format!(
                    "rollback incomplete; recovery data retained at `{}`: {error}",
                    preserved.display()
                ),
            ));
        }
        self.moves.clear();
        self.staging.close()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn case_names_reject_paths_and_control_characters() {
        for name in [
            "",
            " ",
            ".",
            "..",
            "../escape",
            "/absolute",
            "nested/case",
            "a\\b",
            "a:b",
        ] {
            assert!(validate_name(name).is_err(), "{name:?}");
        }
        for code in (0..=31).chain([0x7f, 0x85, 0x9f]) {
            let name = format!("control{}name", char::from_u32(code).unwrap());
            assert!(validate_name(&name).is_err(), "{name:?}");
        }
    }

    #[test]
    fn shared_names_are_case_sensitive() {
        assert!(is_shared_entry(OsStr::new("tools")));
        assert!(!is_shared_entry(OsStr::new("tools-backup")));
        assert!(!is_shared_entry(OsStr::new("TOOLS")));
    }

    #[cfg(unix)]
    #[test]
    fn conversion_rejects_relocated_shared_assets_with_noncanonical_root() {
        let directory = tempfile::tempdir().unwrap();
        let config_root = directory.path();
        initialize_case(config_root, CaseConfig::new_case("original".into()), None).unwrap();
        fs::create_dir(config_root.join("alias-component")).unwrap();
        fs::create_dir(config_root.join("notes")).unwrap();
        fs::rename(config_root.join("kb"), config_root.join("notes/knowledge")).unwrap();
        std::os::unix::fs::symlink("notes/knowledge", config_root.join("kb")).unwrap();
        fs::write(config_root.join("notes/knowledge/evidence"), b"preserve").unwrap();
        let original_config = fs::read(config_root.join("orev.toml")).unwrap();
        let mixed_root = config_root.join("alias-component/..");
        let error = add_locked(&mixed_root, "second", |_, _| {
            panic!("unsafe conversion must fail before registry publication");
        })
        .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("points into data being relocated")
        );
        assert_eq!(
            fs::read(config_root.join("orev.toml")).unwrap(),
            original_config
        );
        assert_eq!(
            fs::read(config_root.join("notes/knowledge/evidence")).unwrap(),
            b"preserve"
        );
        assert!(!config_root.join("cases").exists());
        assert!(
            fs::symlink_metadata(config_root.join("kb"))
                .unwrap()
                .is_symlink()
        );
    }

    #[test]
    fn conversion_restores_original_files_when_registry_publication_fails() {
        let config_root = tempfile::tempdir().unwrap();
        initialize_case(
            config_root.path(),
            CaseConfig::new_case("original".into()),
            None,
        )
        .unwrap();
        fs::write(config_root.path().join("notes.md"), b"analyst notes\n").unwrap();
        let original_config = fs::read(config_root.path().join("orev.toml")).unwrap();
        let original_guide = fs::read(config_root.path().join("CASE.md")).unwrap();
        let original_ignore = fs::read(config_root.path().join(".gitignore")).unwrap();
        let error = add_locked(config_root.path(), "second", |_, _| {
            Err(io::Error::other("injected registry publication failure"))
        })
        .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("injected registry publication failure")
        );
        assert_eq!(
            fs::read(config_root.path().join("orev.toml")).unwrap(),
            original_config
        );
        assert_eq!(
            fs::read(config_root.path().join("CASE.md")).unwrap(),
            original_guide
        );
        assert_eq!(
            fs::read(config_root.path().join(".gitignore")).unwrap(),
            original_ignore
        );
        assert_eq!(
            fs::read(config_root.path().join("notes.md")).unwrap(),
            b"analyst notes\n"
        );
        assert!(!config_root.path().join("cases").exists());
        for entry in fs::read_dir(config_root.path()).unwrap() {
            assert!(
                !entry
                    .unwrap()
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".orev-case-stage-")
            );
        }
    }

    #[test]
    fn conversion_keeps_the_standalone_config_until_transaction_publication() {
        let config_root = tempfile::tempdir().unwrap();
        initialize_case(
            config_root.path(),
            CaseConfig::new_case("original".into()),
            None,
        )
        .unwrap();
        let original = fs::read(config_root.path().join("orev.toml")).unwrap();
        add_locked(config_root.path(), "second", |path, workspace| {
            assert_eq!(fs::read(path)?, original);
            assert!(matches!(
                RootConfig::parse(&fs::read_to_string(path)?).unwrap(),
                RootConfig::Case(_)
            ));
            for name in ["original", "second"] {
                let content = fs::read_to_string(
                    config_root
                        .path()
                        .join("cases")
                        .join(name)
                        .join("orev.toml"),
                )?;
                assert!(content.contains("workspace_root = \"../..\""));
                assert!(!content.contains("[storage]"));
            }
            replace_config(path, workspace)
        })
        .unwrap();
        assert!(matches!(
            RootConfig::parse(&fs::read_to_string(config_root.path().join("orev.toml")).unwrap())
                .unwrap(),
            RootConfig::Workspace(_)
        ));
    }

    #[test]
    fn adding_case_preserves_registry_and_existing_cases_when_publication_fails() {
        let config_root = tempfile::tempdir().unwrap();
        for directory in ["cases/existing", "kb", "store", "tools"] {
            fs::create_dir_all(config_root.path().join(directory)).unwrap();
        }
        let workspace = WorkspaceConfig {
            workspace: WorkspaceInfo {
                cases: vec!["existing".into()],
            },
            storage: StorageConfig::default(),
        };
        let content = toml::to_string_pretty(&workspace).unwrap();
        fs::write(config_root.path().join("orev.toml"), &content).unwrap();
        fs::write(config_root.path().join("cases/existing/note"), b"preserve").unwrap();
        add_locked(config_root.path(), "second", |_, _| {
            Err(io::Error::other("injected registry publication failure"))
        })
        .unwrap_err();
        assert_eq!(
            fs::read_to_string(config_root.path().join("orev.toml")).unwrap(),
            content
        );
        assert_eq!(
            fs::read(config_root.path().join("cases/existing/note")).unwrap(),
            b"preserve"
        );
        assert!(!config_root.path().join("cases/second").exists());
    }

    #[test]
    fn incomplete_rollback_preserves_recovery_data_without_overwriting_new_files() {
        let config_root = tempfile::tempdir().unwrap();
        initialize_case(
            config_root.path(),
            CaseConfig::new_case("original".into()),
            None,
        )
        .unwrap();
        fs::write(config_root.path().join("notes.md"), b"original notes").unwrap();
        let original_config = fs::read(config_root.path().join("orev.toml")).unwrap();
        let error = add_locked(config_root.path(), "second", |_, _| {
            fs::write(
                config_root.path().join("notes.md"),
                b"concurrent analyst notes",
            )?;
            Err(io::Error::other("injected publication failure"))
        })
        .unwrap_err();
        let message = error.to_string();
        assert!(message.contains("injected publication failure"));
        assert!(message.contains("rollback incomplete"));
        assert_eq!(
            fs::read(config_root.path().join("notes.md")).unwrap(),
            b"concurrent analyst notes"
        );
        assert_eq!(
            fs::read(config_root.path().join("orev.toml")).unwrap(),
            original_config
        );
        let recovery = fs::read_dir(config_root.path())
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .find(|path| {
                path.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with(".orev-case-stage-")
            })
            .expect("recovery directory must be retained");
        assert!(message.contains(&recovery.display().to_string()));
        assert_eq!(
            fs::read(recovery.join("original/notes.md")).unwrap(),
            b"original notes"
        );
    }
}
