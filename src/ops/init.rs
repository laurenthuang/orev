// src/ops/init.rs
use crate::config::{CaseConfig, WorkspaceConfig};
use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::Path;

pub(super) const WORKSPACE_IGNORES: &str = r#"# Orev Workspace Ignores
/.orev-case-add.lock/
/.orev-case-stage-*/

# Tool working data; scripts and extensions remain trackable
*.gpr
*.rep/
*.i64
*.bndb
*.rzdb

# Large runtime artifacts
*.pcap
*.pcapng
*.dmp
*.mem

# Storage and temporary state
store/
tools/*/working/
.cache/
log/
state/env.md
/cases/*/state/env.md
/cases/*/.orev/
"#;
pub(super) const WORKSPACE_README: &str = "# Orev Workspace\n\n\
Run `orev case add <name>` here or in a subdirectory to create a case in `cases/`.\n\
The root `orev.toml` registers cases. `kb/`, `store/`, and `tools/` are shared.\n\
`corpus/` is optional. There is no automatic analysis or tool execution.\n\
Run `orev status` here to list registered cases, or from a case root for case details.\n\
Run `orev doctor` here for shared directories, or from a case root for case directories.\n";

/// 初始化多案件工作区及空案件登记表。
pub fn run_workspace_init(path: Option<String>) -> io::Result<()> {
    let current_dir = env::current_dir().inspect_err(|error| {
        eprintln!("Error: Failed to get current directory: {}", error);
    })?;
    let workspace_root = match path {
        Some(path) => current_dir.join(path),
        None => current_dir,
    };

    let config_path = workspace_root.join("orev.toml");
    let config_exists = existing_regular_file(&config_path)?;
    if config_exists {
        let content = fs::read_to_string(&config_path).inspect_err(|e| {
            eprintln!("Error: Failed to read `{}`: {}", config_path.display(), e);
        })?;
        let _: WorkspaceConfig = toml::from_str(&content).map_err(|e| {
            let error = io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "not a multi-case workspace configuration at `{}`: {e}",
                    config_path.display()
                ),
            );
            eprintln!("Error: {}", error);
            error
        })?;
    }
    {
        let marker_path = workspace_root.join("CASE.md");
        match fs::symlink_metadata(&marker_path) {
            Ok(_) => {
                let error = io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    format!(
                        "cannot initialize a multi-case workspace at a case root: `{}` exists",
                        marker_path.display()
                    ),
                );
                eprintln!("Error: {}", error);
                return Err(error);
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => {
                eprintln!(
                    "Error: Failed to inspect `{}`: {}",
                    marker_path.display(),
                    error
                );
                return Err(error);
            }
        }
    }

    println!("Initializing workspace...");
    println!("Workspace Root: {}", workspace_root.display());
    ensure_directory(&workspace_root)?;
    for directory in [
        "cases",
        "kb/techniques",
        "kb/signatures",
        "kb/pitfalls",
        "store",
        "tools",
    ] {
        ensure_directory(&workspace_root.join(directory))?;
    }

    if !config_exists {
        let content = toml::to_string_pretty(&WorkspaceConfig::default())
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        create_file_if_absent(&config_path, |file| file.write_all(content.as_bytes()))?;
    }
    for (relative, content) in [
        ("README.md", WORKSPACE_README),
        (".gitignore", WORKSPACE_IGNORES),
    ] {
        if create_file_if_absent(&workspace_root.join(relative), |file| {
            file.write_all(content.as_bytes())
        })? {
            println!("  [created] {}", relative);
        } else {
            println!("  [exists]  {}", relative);
        }
    }

    println!("\nSuccess! Workspace skeleton is ready.");
    println!("Use `orev case add <name>` to create and register a case.");
    Ok(())
}

/// 核心业务函数：在指定路径初始化二进制分析案件
///
/// # 参数
/// - `path`: 可选的目标案件路径。如果为 `None`，则使用当前进程目录。
pub fn run_init(path: Option<String>) -> io::Result<()> {
    // 1. 确定案件的根目录
    let current_dir = match env::current_dir() {
        Ok(dir) => dir,
        Err(e) => {
            eprintln!("Error: Failed to get current directory: {}", e);
            return Err(e);
        }
    };

    //💡路径组合与解析
    // 如果用户传了 path，就用 current_dir.join(path) 拼接成绝对或相对目标路径；否则直接用当前目录。
    let case_root = match path {
        Some(ref p) => current_dir.join(p),
        None => current_dir.clone(),
    };

    // 如果目标案件目录不存在，递归创建它
    if let Err(e) = fs::create_dir_all(&case_root) {
        eprintln!(
            "Error: Failed to create case directory `{}`: {}",
            case_root.display(),
            e
        );
        return Err(e);
    }

    // 2. 确定新案例的默认名称
    let case_name = case_root
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("binary_case")
        .to_string();

    initialize_case(&case_root, CaseConfig::new_case(case_name))?;
    println!("\nSuccess! Analysis case is ready.");
    Ok(())
}

pub(super) fn initialize_case(case_root: &Path, initial: CaseConfig) -> io::Result<()> {
    println!("Initializing analysis case...");
    println!("Case Root: {}", case_root.display());

    // 3. 创建或验证配置文件
    let config_path = case_root.join("orev.toml");

    let config_created = if !existing_regular_file(&config_path)? {
        let config_content = match toml::to_string_pretty(&initial) {
            Ok(content) => content,
            Err(e) => {
                eprintln!("Error: Failed to serialize orev.toml: {}", e);
                return Err(io::Error::new(io::ErrorKind::InvalidData, e));
            }
        };
        create_file_if_absent(&config_path, |file| {
            file.write_all(config_content.as_bytes())
        })?
    } else {
        false
    };

    let config_content = match fs::read_to_string(&config_path) {
        Ok(content) => content,
        Err(e) => {
            eprintln!("Error: Failed to read orev.toml: {}", e);
            return Err(e);
        }
    };

    let config: CaseConfig = match toml::from_str(&config_content) {
        Ok(config) => config,
        Err(e) => {
            eprintln!("Error: Failed to parse orev.toml: {}", e);
            return Err(io::Error::new(io::ErrorKind::InvalidData, e));
        }
    };

    if config_created {
        println!("  [created] orev.toml");
    } else {
        println!("  [exists]  orev.toml (validated)");
    }

    println!("Case Name: {}", config.case.name);

    let store_root = match config.storage.resolve_store_root(case_root) {
        Ok(path) => path,
        Err(e) => {
            eprintln!("Error: Failed to resolve storage.store_root: {}", e);
            return Err(e);
        }
    };
    let tools_root = match config.storage.resolve_tools_root(case_root) {
        Ok(path) => path,
        Err(e) => {
            eprintln!("Error: Failed to resolve storage.tools_root: {}", e);
            return Err(e);
        }
    };

    // 4. 创建标准案件子目录
    let kb_root = config.resolve_kb_root(case_root).inspect_err(|e| {
        eprintln!("Error: Failed to resolve workspace knowledge base: {}", e);
    })?;
    if config.workspace_root.is_none() {
        for child in ["techniques", "signatures", "pitfalls"] {
            ensure_directory(&kb_root.join(child))?;
        }
    }
    let local_dirs = ["scripts", "state", ".orev"];

    for dir in &local_dirs {
        ensure_directory(&case_root.join(dir))?;
    }

    for (kind, root, external) in [
        ("store", &store_root, config.storage.store_root.is_some()),
        ("tools", &tools_root, config.storage.tools_root.is_some()),
    ] {
        if external {
            println!("  [external] {}", root.display());
            continue;
        }

        let already_directory = root.is_dir();

        if let Err(e) = fs::create_dir_all(root) {
            eprintln!(
                "Error: Failed to create local {kind} `{}`: {}",
                root.display(),
                e
            );
            return Err(e);
        }

        if already_directory {
            println!("  [exists]  {}", root.display());
        } else {
            println!("  [created] {}", root.display());
        }
    }

    // 5. 生成高层导引 CASE.md
    let case_md_path = case_root.join("CASE.md");
    let case_md_content = format!(
        r#"# Analysis Case: {}

## 1. Core Question (要回答什么问题)
- 

## 2. Executive Conclusion (核心结论)
- Status: In Progress
- Summary: TBD

## 3. Reference Links
- Investigation Threads: `state/investigation.yaml`
- Case-specific Scripts: `scripts/<tool>/`
"#,
        config.case.name
    );
    if create_file_if_absent(&case_md_path, |file| {
        file.write_all(case_md_content.as_bytes())
    })? {
        println!("  [created] CASE.md");
    } else {
        println!("  [exists]  CASE.md");
    }

    // 6. 初始化状态管理用文件
    for relative_path in ["state/index.yaml", "state/investigation.yaml"] {
        let file_path = case_root.join(relative_path);

        if create_file_if_absent(&file_path, |file| file.write_all(b"[]\n"))? {
            println!("  [created] {}", relative_path);
        } else {
            println!("  [exists]  {}", relative_path);
        }
    }

    // 7. 生成专属 .gitignore
    let gitignore_path = case_root.join(".gitignore");
    let gitignore_content = r#"# Orev Case Ignores
.orev/
store/
.cache/
log/
state/env.md

# Tool working data; scripts and extensions remain trackable
/tools/*/working/
*.gpr
*.rep/
*.i64
*.bndb
*.rzdb

# Large runtime artifacts
*.dmp
*.pcap
*.pcapng
*.mem
"#;
    if create_file_if_absent(&gitignore_path, |file| {
        file.write_all(gitignore_content.as_bytes())
    })? {
        println!("  [created] .gitignore");
    } else {
        println!("  [exists]  .gitignore");
    }

    Ok(())
}

fn ensure_directory(path: &Path) -> io::Result<()> {
    let already_directory = path.is_dir();
    fs::create_dir_all(path).inspect_err(|error| {
        eprintln!(
            "Error: Failed to ensure directory `{}`: {}",
            path.display(),
            error
        );
    })?;
    if already_directory {
        println!("  [exists]  {}/", path.display());
    } else {
        println!("  [created] {}/", path.display());
    }
    Ok(())
}

fn create_file_if_absent(
    path: &Path,
    write: impl FnOnce(&mut fs::File) -> io::Result<()>,
) -> io::Result<bool> {
    let result: io::Result<bool> = (|| {
        if existing_regular_file(path)? {
            return Ok(false);
        }

        let parent = path.parent().ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "file path has no parent")
        })?;
        let mut builder = tempfile::Builder::new();
        builder.prefix(".orev-init-");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            // Preserve normal file creation permissions, including the process umask.
            builder.permissions(fs::Permissions::from_mode(0o666));
        }
        let mut staged = builder.tempfile_in(parent)?;

        if let Err(error) = write(staged.as_file_mut()).and_then(|()| staged.as_file().sync_all()) {
            if let Err(cleanup_error) = staged.close() {
                eprintln!(
                    "Error: Failed to remove staging file for `{}`: {}",
                    path.display(),
                    cleanup_error
                );
            }
            return Err(error);
        }

        match staged.persist_noclobber(path) {
            Ok(_) => Ok(true),
            Err(tempfile::PersistError { error, file }) => {
                file.close().map_err(|cleanup_error| {
                    eprintln!(
                        "Error: Failed to remove staging file for `{}` after publish error `{}`: {}",
                        path.display(),
                        error,
                        cleanup_error
                    );
                    cleanup_error
                })?;
                if error.kind() == io::ErrorKind::AlreadyExists && existing_regular_file(path)? {
                    Ok(false)
                } else {
                    Err(error)
                }
            }
        }
    })();

    result.inspect_err(|error| {
        eprintln!("Error: Failed to create `{}`: {}", path.display(), error);
    })
}

fn existing_regular_file(path: &Path) -> io::Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(_) => {}
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            return Ok(false);
        }
        Err(e) => {
            eprintln!("Error: Failed to inspect `{}`: {}", path.display(), e);
            return Err(e);
        }
    }

    let metadata = match fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(e) => {
            eprintln!(
                "Error: Failed to inspect target of `{}`: {}",
                path.display(),
                e
            );
            return Err(e);
        }
    };

    if !metadata.is_file() {
        let e = io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!("Expected a regular file at `{}`", path.display()),
        );
        eprintln!("Error: {}", e);
        return Err(e);
    }

    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_publication_preserves_existing_files_without_writing() {
        let directory = tempfile::tempdir().expect("failed to create test directory");
        let path = directory.path().join("metadata");
        fs::write(&path, b"existing").expect("failed to create existing file");

        let created = create_file_if_absent(&path, |_| {
            panic!("writer must not run for an existing file");
        })
        .expect("failed to preserve existing file");

        assert!(!created);
        assert_eq!(
            fs::read(path).expect("failed to read existing file"),
            b"existing"
        );
    }

    #[test]
    fn file_publication_exposes_only_complete_content() {
        let directory = tempfile::tempdir().expect("failed to create test directory");
        let path = directory.path().join("metadata");

        let created = create_file_if_absent(&path, |file| {
            assert!(!path.try_exists()?);
            file.write_all(b"first half\n")?;
            assert!(!path.try_exists()?);
            file.write_all(b"second half\n")
        })
        .expect("failed to publish metadata");

        assert!(created);
        assert_eq!(
            fs::read(&path).expect("failed to read metadata"),
            b"first half\nsecond half\n",
        );
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    }

    #[test]
    fn file_publication_cleans_up_after_partial_write_failure() {
        let directory = tempfile::tempdir().expect("failed to create test directory");
        let path = directory.path().join("metadata");

        let error = create_file_if_absent(&path, |file| {
            file.write_all(b"incomplete metadata")?;
            Err(io::Error::other("injected write failure"))
        })
        .expect_err("partial writes must fail");

        assert_eq!(error.to_string(), "injected write failure");
        assert!(!path.try_exists().expect("failed to inspect destination"));
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 0);
    }

    #[test]
    fn file_publication_preserves_a_racing_creator() {
        let directory = tempfile::tempdir().expect("failed to create test directory");
        let path = directory.path().join("metadata");

        let created = create_file_if_absent(&path, |file| {
            file.write_all(b"candidate")?;
            fs::write(&path, b"racing creator")
        })
        .expect("failed to preserve racing file");

        assert!(!created);
        assert_eq!(fs::read(&path).unwrap(), b"racing creator");
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    }

    #[test]
    fn file_publication_rejects_a_racing_directory() {
        let directory = tempfile::tempdir().expect("failed to create test directory");
        let path = directory.path().join("metadata");

        create_file_if_absent(&path, |file| {
            file.write_all(b"candidate")?;
            fs::create_dir(&path)
        })
        .expect_err("a racing directory must not be accepted as a file");

        assert!(path.is_dir());
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    }

    #[cfg(unix)]
    #[test]
    fn file_publication_rejects_a_racing_dangling_symlink() {
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir().expect("failed to create test directory");
        let path = directory.path().join("metadata");
        let missing = directory.path().join("missing");

        create_file_if_absent(&path, |file| {
            file.write_all(b"candidate")?;
            symlink(&missing, &path)
        })
        .expect_err("a racing dangling symlink must not be accepted");

        assert!(
            fs::symlink_metadata(&path)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert!(
            !missing
                .try_exists()
                .expect("failed to inspect symlink target")
        );
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    }

    #[cfg(unix)]
    #[test]
    fn file_publication_preserves_normal_file_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let directory = tempfile::tempdir().expect("failed to create test directory");
        let reference = directory.path().join("reference");
        let path = directory.path().join("metadata");
        fs::write(&reference, b"reference").expect("failed to create reference file");

        create_file_if_absent(&path, |file| file.write_all(b"metadata"))
            .expect("failed to publish metadata");

        assert_eq!(
            fs::metadata(path).unwrap().permissions().mode() & 0o777,
            fs::metadata(reference).unwrap().permissions().mode() & 0o777,
        );
    }
}
