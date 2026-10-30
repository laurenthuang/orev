use crate::config::{RootConfig, WorkspaceConfig, WorkspaceContext, find_config_root};
use crate::ops::case::{read_config, validate_name};
use chrono::{DateTime, SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashSet};
use std::env;
use std::ffi::CString;
use std::fs::{self, DirBuilder, File, Metadata, OpenOptions};
use std::io::{self, Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt};
use std::path::{Component, Path, PathBuf};

const MANIFEST: &str = "stage.yaml";
const SCRATCH_PREFIX: &str = ".orev-ingest-";
const DIRECTORY_DOMAIN: &[u8] = b"orev-directory-sha256-v1\0";

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum Sample {
    File {
        stored_path: String,
        byte_size: u64,
        ingested_at: String,
        sha256: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        source_modified_at: Option<String>,
    },
    Directory {
        stored_path: String,
        byte_size: u64,
        ingested_at: String,
        sha256: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        source_modified_at: Option<String>,
        entries: Vec<DirectoryEntry>,
    },
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum DirectoryEntry {
    File {
        path: String,
        byte_size: u64,
        sha256: String,
    },
    Directory {
        path: String,
    },
}

impl DirectoryEntry {
    fn path(&self) -> &str {
        match self {
            Self::File { path, .. } | Self::Directory { path } => path,
        }
    }
}

struct Source {
    path: PathBuf,
    name: String,
    metadata: Metadata,
    entries: Vec<SourceEntry>,
}

struct SourceEntry {
    path: String,
    metadata: Metadata,
}

struct Fingerprint {
    byte_size: u64,
    sha256: String,
    entries: Vec<DirectoryEntry>,
}

struct Manifest {
    samples: Vec<Sample>,
    original: Option<(Metadata, Vec<u8>)>,
}

struct PublishedSample {
    path: PathBuf,
    metadata: Metadata,
}

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message.into())
}

fn utf8(path: &Path) -> io::Result<&str> {
    path.to_str()
        .ok_or_else(|| invalid(format!("sample paths must be UTF-8: `{}`", path.display())))
}

fn sample_name(name: &str) -> io::Result<()> {
    if !safe_relative(name)
        || Path::new(name).components().count() != 1
        || name == MANIFEST
        || name.starts_with(SCRATCH_PREFIX)
    {
        return Err(invalid(format!("reserved or invalid sample name `{name}`")));
    }
    Ok(())
}

fn safe_relative(path: &str) -> bool {
    !path.is_empty()
        && path.split('/').all(|part| !matches!(part, "" | "." | ".."))
        && Path::new(path)
            .components()
            .all(|part| matches!(part, Component::Normal(_)))
        && !path.contains('\0')
}

fn parse_config(root: &Path) -> io::Result<RootConfig> {
    RootConfig::parse(&read_config(root)?)
        .map_err(|error| invalid(format!("invalid orev.toml: {error}")))
}

fn named_workspace_case(
    workspace_root: &Path,
    workspace: &WorkspaceConfig,
    name: &str,
) -> io::Result<PathBuf> {
    if !workspace.workspace.cases.iter().any(|case| case == name) {
        return Err(invalid(format!(
            "case `{name}` is not registered in the workspace"
        )));
    }
    let workspace_root = fs::canonicalize(workspace_root)?;
    let cases_root = workspace_root.join("cases");
    let case_root = cases_root.join(name);
    for path in [&cases_root, &case_root] {
        if !fs::symlink_metadata(path)?.is_dir() {
            return Err(invalid(format!(
                "named case selection requires a directory, not a symlink or file: `{}`",
                path.display()
            )));
        }
    }
    let RootConfig::Case(config) = parse_config(&case_root)? else {
        return Err(invalid(format!(
            "case `{name}` has a workspace configuration"
        )));
    };
    if config.case.name != name {
        return Err(invalid(format!(
            "case `{name}` has a different configured name `{}`",
            config.case.name
        )));
    }
    let associated = config
        .workspace_root
        .as_ref()
        .ok_or_else(|| invalid(format!("case `{name}` has no workspace_root association")))?;
    if fs::canonicalize(case_root.join(associated))? != workspace_root {
        return Err(invalid(format!(
            "case `{name}` is associated with a different workspace"
        )));
    }
    config.resolve_resources(
        &case_root,
        Some(WorkspaceContext {
            root: &workspace_root,
            config: workspace,
        }),
    )?;
    fs::canonicalize(case_root)
}

fn resolve_case(case_name: Option<&str>) -> io::Result<PathBuf> {
    if let Some(name) = case_name {
        validate_name(name)?;
    }
    let root = find_config_root(&env::current_dir()?)?;
    match parse_config(&root)? {
        RootConfig::Case(config) => {
            if let Some(name) = case_name {
                if let Some(workspace_root) = &config.workspace_root {
                    let workspace_root = fs::canonicalize(root.join(workspace_root))?;
                    let RootConfig::Workspace(workspace) = parse_config(&workspace_root)? else {
                        return Err(invalid(
                            "workspace_root must point to a workspace configuration",
                        ));
                    };
                    return named_workspace_case(&workspace_root, &workspace, name);
                }
                if config.case.name != name {
                    return Err(invalid(format!(
                        "standalone case is named `{}`, not `{name}`",
                        config.case.name
                    )));
                }
            }
            config.resolve_resources(&root, None)?;
            fs::canonicalize(root)
        }
        RootConfig::Workspace(workspace) => {
            let name = case_name
                .ok_or_else(|| invalid("sample ingest requires a case, not a workspace"))?;
            named_workspace_case(&root, &workspace, name)
        }
    }
}

fn regular_metadata(path: &Path) -> io::Result<Metadata> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_file() && !metadata.is_dir() {
        return Err(invalid(format!(
            "only regular files and directories are supported, not symlinks or special files: `{}`",
            path.display()
        )));
    }
    Ok(metadata)
}

fn source_entries(root: &Path) -> io::Result<Vec<SourceEntry>> {
    fn visit(root: &Path, directory: &Path, entries: &mut Vec<SourceEntry>) -> io::Result<()> {
        for entry in fs::read_dir(directory)? {
            let path = entry?.path();
            let metadata = regular_metadata(&path)?;
            let relative = utf8(path.strip_prefix(root).expect("entry is below root"))?;
            if !safe_relative(relative) {
                return Err(invalid("invalid relative sample path"));
            }
            entries.push(SourceEntry {
                path: relative.to_owned(),
                metadata: metadata.clone(),
            });
            if metadata.is_dir() {
                visit(root, &path, entries)?;
            }
        }
        Ok(())
    }
    let mut entries = Vec::new();
    visit(root, root, &mut entries)?;
    entries.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(entries)
}

fn preflight(paths: &[PathBuf], intake: &Path) -> io::Result<Vec<Source>> {
    if paths.is_empty() {
        return Err(invalid("at least one sample path is required"));
    }
    let cwd = env::current_dir()?;
    let mut names = HashSet::new();
    let mut sources = Vec::new();
    for input in paths {
        utf8(input)?;
        let absolute = cwd.join(input);
        // Check before canonicalizing so an ancestor symlink cannot disappear.
        let mut ancestor = PathBuf::new();
        for component in absolute.components() {
            ancestor.push(component);
            if fs::symlink_metadata(&ancestor)?.file_type().is_symlink() {
                return Err(invalid(format!(
                    "sample source contains a symlink: `{}`",
                    ancestor.display()
                )));
            }
        }
        let path = fs::canonicalize(&absolute)?;
        utf8(&path)?;
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| invalid("a sample source must have a UTF-8 basename"))?
            .to_owned();
        sample_name(&name)?;
        if !names.insert(name.clone()) {
            return Err(invalid(format!("duplicate sample basename `{name}`")));
        }
        let metadata = regular_metadata(&path)?;
        if path.starts_with(intake) || (metadata.is_dir() && intake.starts_with(&path)) {
            return Err(invalid(format!(
                "sample source overlaps 00_intake: `{}`",
                path.display()
            )));
        }
        let entries = if metadata.is_dir() {
            source_entries(&path)?
        } else {
            Vec::new()
        };
        sources.push(Source {
            path,
            name,
            metadata,
            entries,
        });
    }
    Ok(sources)
}

fn same_identity(left: &Metadata, right: &Metadata) -> bool {
    left.dev() == right.dev() && left.ino() == right.ino() && left.file_type() == right.file_type()
}

fn same_source(left: &Metadata, right: &Metadata) -> bool {
    same_identity(left, right)
        && left.len() == right.len()
        && left.mtime() == right.mtime()
        && left.mtime_nsec() == right.mtime_nsec()
        && left.ctime() == right.ctime()
        && left.ctime_nsec() == right.ctime_nsec()
}

fn open_regular(path: &Path) -> io::Result<File> {
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)?;
    if !file.metadata()?.is_file() {
        return Err(invalid(format!(
            "expected a regular file: `{}`",
            path.display()
        )));
    }
    Ok(file)
}

fn hash_file(path: &Path) -> io::Result<(u64, String)> {
    let mut file = open_regular(path)?;
    let mut hash = Sha256::new();
    let mut size = 0u64;
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        size = size
            .checked_add(count as u64)
            .ok_or_else(|| invalid("sample byte size exceeds u64"))?;
        hash.update(&buffer[..count]);
    }
    Ok((size, format!("{:x}", hash.finalize())))
}

fn hash_bytes(value: &str) -> io::Result<[u8; 32]> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(invalid(
            "SHA-256 must contain 64 lowercase hexadecimal digits",
        ));
    }
    let mut bytes = [0; 32];
    for (index, byte) in bytes.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&value[index * 2..index * 2 + 2], 16)
            .map_err(|error| invalid(error.to_string()))?;
    }
    Ok(bytes)
}

fn directory_fingerprint(entries: &[DirectoryEntry], root: &str) -> io::Result<(u64, String)> {
    let mut hash = Sha256::new();
    hash.update(DIRECTORY_DOMAIN);
    let mut total = 0u64;
    let mut seen = BTreeMap::new();
    let mut previous: Option<&str> = None;
    for entry in entries {
        let path = entry
            .path()
            .strip_prefix(root)
            .and_then(|path| path.strip_prefix('/'))
            .ok_or_else(|| invalid("directory entry must be below its sample's stored_path"))?;
        if !safe_relative(path) || previous.is_some_and(|previous| previous >= path) {
            return Err(invalid(
                "directory entries must have unique, sorted, safe relative paths",
            ));
        }
        if let Some(parent) = Path::new(path)
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            && seen.get(utf8(parent)?) != Some(&true)
        {
            return Err(invalid("directory entry is missing its directory parent"));
        }
        let is_directory = matches!(entry, DirectoryEntry::Directory { .. });
        hash.update(if is_directory { b"D" } else { b"F" });
        hash.update((path.len() as u64).to_be_bytes());
        hash.update(path.as_bytes());
        if let DirectoryEntry::File {
            byte_size, sha256, ..
        } = entry
        {
            hash.update(byte_size.to_be_bytes());
            hash.update(hash_bytes(sha256)?);
            total = total
                .checked_add(*byte_size)
                .ok_or_else(|| invalid("sample byte size exceeds u64"))?;
        }
        seen.insert(path, is_directory);
        previous = Some(path);
    }
    Ok((total, format!("{:x}", hash.finalize())))
}

fn fingerprint(path: &Path) -> io::Result<Fingerprint> {
    let metadata = regular_metadata(path)?;
    if metadata.is_file() {
        let (byte_size, sha256) = hash_file(path)?;
        return Ok(Fingerprint {
            byte_size,
            sha256,
            entries: Vec::new(),
        });
    }
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| invalid("stored sample must have a UTF-8 basename"))?;
    let mut entries = Vec::new();
    for entry in source_entries(path)? {
        let stored_path = format!("{name}/{}", entry.path);
        if entry.metadata.is_dir() {
            entries.push(DirectoryEntry::Directory { path: stored_path });
        } else {
            let (byte_size, sha256) = hash_file(&path.join(&entry.path))?;
            entries.push(DirectoryEntry::File {
                path: stored_path,
                byte_size,
                sha256,
            });
        }
    }
    let (byte_size, sha256) = directory_fingerprint(&entries, name)?;
    Ok(Fingerprint {
        byte_size,
        sha256,
        entries,
    })
}

fn utc_timestamp(timestamp: &str) -> io::Result<()> {
    let time =
        DateTime::parse_from_rfc3339(timestamp).map_err(|error| invalid(error.to_string()))?;
    if time.offset().local_minus_utc() != 0 {
        return Err(invalid("sample timestamps must be UTC"));
    }
    Ok(())
}

impl Sample {
    fn validate(&self, intake: &Path, names: &mut HashSet<String>) -> io::Result<()> {
        let (Self::File {
            stored_path,
            byte_size,
            ingested_at,
            sha256,
            source_modified_at,
        }
        | Self::Directory {
            stored_path,
            byte_size,
            ingested_at,
            sha256,
            source_modified_at,
            ..
        }) = self;
        sample_name(stored_path)?;
        if !names.insert(stored_path.clone()) {
            return Err(invalid("duplicate stored_path in stage.yaml"));
        }
        utc_timestamp(ingested_at)?;
        if let Some(modified) = source_modified_at {
            utc_timestamp(modified)?;
        }
        hash_bytes(sha256)?;
        let path = intake.join(stored_path);
        let metadata = regular_metadata(&path)?;
        if metadata.is_dir() != matches!(self, Self::Directory { .. }) {
            return Err(invalid(
                "stage.yaml sample kind does not match the stored artifact",
            ));
        }
        if let Self::Directory { entries, .. } = self {
            let (size, digest) = directory_fingerprint(entries, stored_path)?;
            if size != *byte_size || digest != *sha256 {
                return Err(invalid("inconsistent directory fingerprint in stage.yaml"));
            }
        }
        let actual = fingerprint(&path)?;
        if actual.byte_size != *byte_size
            || actual.sha256 != *sha256
            || matches!(self, Self::Directory { entries, .. } if *entries != actual.entries)
        {
            return Err(invalid(
                "stage.yaml fingerprint does not match the stored artifact",
            ));
        }
        Ok(())
    }

    fn from_copy(source: &Source, copied: Fingerprint) -> io::Result<Self> {
        let stored_path = source.name.clone();
        let byte_size = copied.byte_size;
        let sha256 = copied.sha256;
        let ingested_at = Utc::now().to_rfc3339_opts(SecondsFormat::Nanos, true);
        let source_modified_at = Some(
            DateTime::<Utc>::from(source.metadata.modified()?)
                .to_rfc3339_opts(SecondsFormat::Nanos, true),
        );
        Ok(if source.metadata.is_dir() {
            Self::Directory {
                stored_path,
                byte_size,
                ingested_at,
                sha256,
                source_modified_at,
                entries: copied.entries,
            }
        } else {
            Self::File {
                stored_path,
                byte_size,
                ingested_at,
                sha256,
                source_modified_at,
            }
        })
    }
}

fn read_manifest(intake: &Path) -> io::Result<Manifest> {
    let path = intake.join(MANIFEST);
    match fs::symlink_metadata(&path) {
        Ok(metadata) => {
            if !metadata.is_file() {
                return Err(invalid("stage.yaml must be a regular file, not a symlink"));
            }
            let mut file = open_regular(&path)?;
            let metadata = file.metadata()?;
            let mut bytes = Vec::new();
            file.read_to_end(&mut bytes)?;
            let samples: Vec<Sample> = serde_yaml_ng::from_slice(&bytes)
                .map_err(|error| invalid(format!("invalid stage.yaml: {error}")))?;
            let mut names = HashSet::new();
            for sample in &samples {
                sample.validate(intake, &mut names)?;
            }
            Ok(Manifest {
                samples,
                original: Some((metadata, bytes)),
            })
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Manifest {
            samples: Vec::new(),
            original: None,
        }),
        Err(error) => Err(error),
    }
}

fn require_absent(path: &Path) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(_) => Err(invalid(format!(
            "sample destination already exists: `{}`",
            path.display()
        ))),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

fn private_directory(path: &Path) -> io::Result<()> {
    DirBuilder::new().mode(0o700).create(path)
}

fn intake_lock(intake: &Path) -> io::Result<File> {
    match private_directory(intake) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(error),
    }
    let directory = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW)
        .open(intake)?;
    loop {
        if unsafe { libc::flock(directory.as_raw_fd(), libc::LOCK_EX) } == 0 {
            break;
        }
        let error = io::Error::last_os_error();
        if error.kind() != io::ErrorKind::Interrupted {
            return Err(error);
        }
    }
    check_intake(intake, &directory)?;
    Ok(directory)
}

fn check_intake(intake: &Path, directory: &File) -> io::Result<()> {
    if !same_identity(&directory.metadata()?, &fs::symlink_metadata(intake)?) {
        return Err(invalid("00_intake changed while ingest was running"));
    }
    Ok(())
}

fn copy_file(source: &Path, expected: &Metadata, target: &Path) -> io::Result<()> {
    let mut input = open_regular(source)?;
    if !same_source(expected, &input.metadata()?) {
        return Err(invalid(format!(
            "source changed during ingest: `{}`",
            source.display()
        )));
    }
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(target)?;
    let size = io::copy(&mut input, &mut output)?;
    output.sync_all()?;
    if size != expected.len()
        || !same_source(expected, &input.metadata()?)
        || !same_source(expected, &fs::symlink_metadata(source)?)
    {
        return Err(invalid(format!(
            "source changed during ingest: `{}`",
            source.display()
        )));
    }
    Ok(())
}

fn copy_sample(source: &Source, target: &Path) -> io::Result<()> {
    if source.metadata.is_file() {
        return copy_file(&source.path, &source.metadata, target);
    }
    private_directory(target)?;
    for entry in &source.entries {
        let destination = target.join(&entry.path);
        if entry.metadata.is_dir() {
            private_directory(&destination)?;
        } else {
            copy_file(
                &source.path.join(&entry.path),
                &entry.metadata,
                &destination,
            )?;
        }
    }
    let after = source_entries(&source.path)?;
    if !same_source(&source.metadata, &fs::symlink_metadata(&source.path)?)
        || after.len() != source.entries.len()
        || after.iter().zip(&source.entries).any(|(after, before)| {
            after.path != before.path || !same_source(&after.metadata, &before.metadata)
        })
    {
        return Err(invalid(format!(
            "source directory changed during ingest: `{}`",
            source.path.display()
        )));
    }
    for entry in source
        .entries
        .iter()
        .rev()
        .filter(|entry| entry.metadata.is_dir())
    {
        File::open(target.join(&entry.path))?.sync_all()?;
    }
    File::open(target)?.sync_all()
}

fn publish_noclobber(source: &Path, target: &Path) -> io::Result<()> {
    let source =
        CString::new(source.as_os_str().as_bytes()).map_err(|error| invalid(error.to_string()))?;
    let target =
        CString::new(target.as_os_str().as_bytes()).map_err(|error| invalid(error.to_string()))?;
    // rename's default directory semantics can replace an unrelated empty directory.
    let result = unsafe {
        libc::renameat2(
            libc::AT_FDCWD,
            source.as_ptr(),
            libc::AT_FDCWD,
            target.as_ptr(),
            libc::RENAME_NOREPLACE,
        )
    };
    if result == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

fn manifest_unchanged(intake: &Path, manifest: &Manifest) -> io::Result<()> {
    let path = intake.join(MANIFEST);
    match &manifest.original {
        None => require_absent(&path),
        Some((metadata, bytes)) => {
            let mut file = open_regular(&path)?;
            let mut current = Vec::new();
            file.read_to_end(&mut current)?;
            if !same_source(metadata, &file.metadata()?) || current != *bytes {
                return Err(invalid("stage.yaml changed while ingest was running"));
            }
            Ok(())
        }
    }
}

fn remove_owned(sample: &PublishedSample) -> io::Result<()> {
    let current = fs::symlink_metadata(&sample.path)?;
    if !same_identity(&sample.metadata, &current) {
        return Err(invalid(format!(
            "refusing to clean up replaced path `{}`",
            sample.path.display()
        )));
    }
    if current.is_dir() {
        fs::remove_dir_all(&sample.path)
    } else {
        fs::remove_file(&sample.path)
    }
}

fn ingest_locked(intake: &Path, directory: &File, sources: &[Source]) -> io::Result<()> {
    let mut manifest = read_manifest(intake)?;
    for source in sources {
        require_absent(&intake.join(&source.name))?;
    }
    let staging = tempfile::Builder::new()
        .prefix(SCRATCH_PREFIX)
        .tempdir_in(intake)?
        .keep();
    let staging_metadata = fs::symlink_metadata(&staging)?;
    let mut published = Vec::new();
    let mut committed = false;
    let operation = (|| {
        for source in sources {
            let target = staging.join(&source.name);
            copy_sample(source, &target)?;
            manifest
                .samples
                .push(Sample::from_copy(source, fingerprint(&target)?)?);
        }
        let next_manifest = staging.join(MANIFEST);
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&next_manifest)?;
        serde_yaml_ng::to_writer(&mut file, &manifest.samples)
            .map_err(|error| invalid(error.to_string()))?;
        file.flush()?;
        file.sync_all()?;
        check_intake(intake, directory)?;
        manifest_unchanged(intake, &manifest)?;
        for source in sources {
            let target = intake.join(&source.name);
            let staged = staging.join(&source.name);
            let metadata = fs::symlink_metadata(&staged)?;
            publish_noclobber(&staged, &target)?;
            published.push(PublishedSample {
                path: target,
                metadata,
            });
        }
        directory.sync_all()?;
        manifest_unchanged(intake, &manifest)?;
        if manifest.original.is_some() {
            fs::rename(&next_manifest, intake.join(MANIFEST))?;
        } else {
            publish_noclobber(&next_manifest, &intake.join(MANIFEST))?;
        }
        committed = true;
        directory.sync_all()
    })();
    let mut cleanup_errors = Vec::new();
    if operation.is_err() && !committed {
        for sample in published.iter().rev() {
            if let Err(error) = remove_owned(sample) {
                cleanup_errors.push(format!(
                    "cleanup `{}` failed: {error}",
                    sample.path.display()
                ));
            }
        }
    }
    if let Err(error) = remove_owned(&PublishedSample {
        path: staging.clone(),
        metadata: staging_metadata,
    }) {
        cleanup_errors.push(format!("cleanup `{}` failed: {error}", staging.display()));
    }
    if !cleanup_errors.is_empty() {
        return Err(io::Error::other(format!(
            "{}; {}",
            operation
                .err()
                .map_or_else(|| "ingest committed".to_owned(), |error| error.to_string()),
            cleanup_errors.join("; ")
        )));
    }
    operation.map_err(|error| {
        if committed {
            io::Error::other(format!(
                "ingest committed, but final directory synchronization failed: {error}"
            ))
        } else {
            error
        }
    })
}

pub fn run_ingest(paths: &[PathBuf], case_name: Option<&str>) -> io::Result<()> {
    let result = (|| {
        let root = resolve_case(case_name)?;
        let intake = root.join("00_intake");
        let sources = preflight(paths, &intake)?;
        let directory = intake_lock(&intake)?;
        ingest_locked(&intake, &directory, &sources)?;
        println!(
            "Success! Ingested {} sample(s) into `{}`.",
            sources.len(),
            intake.display()
        );
        Ok(())
    })();
    result.inspect_err(|error| eprintln!("Error: Failed to ingest samples: {error}"))
}
