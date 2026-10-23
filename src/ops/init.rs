// src/ops/init.rs
use crate::config::{CaseInfo, Config, StorageConfig, TargetsConfig};
use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::Path;

/// 核心业务函数：在指定路径初始化二进制分析案例工作区
///
/// # 参数
/// - `path`: 可选的目标案例工作区路径。如果为 `None`，则使用当前进程目录。
/// - `name`: 新案件的可选名称；已有配置时必须与其中的名称一致。
pub fn run_init(path: Option<String>, name: Option<String>) -> io::Result<()> {
    // 1. 确定案例工作区的根目录
    let current_dir = match env::current_dir() {
        Ok(dir) => dir,
        Err(e) => {
            eprintln!("Error: Failed to get current directory: {}", e);
            return Err(e);
        }
    };

    //💡路径组合与解析
    // 如果用户传了 path，就用 current_dir.join(path) 拼接成绝对或相对目标路径；否则直接用当前目录。
    let workspace_dir = match path {
        Some(ref p) => current_dir.join(p),
        None => current_dir.clone(),
    };

    // 如果目标案例工作区目录不存在，递归创建它
    if let Err(e) = fs::create_dir_all(&workspace_dir) {
        eprintln!(
            "Error: Failed to create case directory `{}`: {}",
            workspace_dir.display(),
            e
        );
        return Err(e);
    }

    // 2. 确定新案例的默认名称
    // 优先级：用户通过 --name 传入的显式名称 > 目标路径文件夹名 > 默认名称
    // 【Rust 基础概念：闭包与 unwrap_or_else】
    // 如果 name 是 None，就执行闭包(|| ...) 里的备用逻辑（获取当前文件夹名）。
    let case_name = name
        .as_deref()
        .unwrap_or_else(|| {
            workspace_dir
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("binary_case")
        })
        .to_string();

    println!("Initializing standalone analysis case...");
    println!("Case Workspace Root: {}", workspace_dir.display());

    // 3. 创建或验证配置文件
    let config_path = workspace_dir.join("orev.toml");

    let config_created = if !existing_regular_file(&config_path)? {
        let config = Config {
            stages: [
                "00_inbox",
                "10_unpack",
                "15_patch",
                "20_static",
                "30_decompile",
                "40_dynamic",
                "50_findings",
            ]
            .into_iter()
            .map(String::from)
            .collect(),
            case: CaseInfo {
                name: case_name.clone(),
                version: "0.1.0".to_string(),
                investigators: Vec::new(),
            },
            targets: TargetsConfig { files: Vec::new() },
            storage: StorageConfig::default(),
        };

        let config_content = match toml::to_string_pretty(&config) {
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

    let config: Config = match toml::from_str(&config_content) {
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

    if let Some(requested_name) = name.as_deref()
        && requested_name != config.case.name
    {
        let error = io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "--name `{requested_name}` conflicts with configured case name `{}`",
                config.case.name
            ),
        );
        eprintln!("Error: {}", error);
        return Err(error);
    }

    println!("Case Name: {}", config.case.name);

    let store_root = match config.storage.resolve_store_root(&workspace_dir) {
        Ok(path) => path,
        Err(e) => {
            eprintln!("Error: Failed to resolve storage.store_root: {}", e);
            return Err(e);
        }
    };
    let tools_root = match config.storage.resolve_tools_root(&workspace_dir) {
        Ok(path) => path,
        Err(e) => {
            eprintln!("Error: Failed to resolve storage.tools_root: {}", e);
            return Err(e);
        }
    };

    // 4. 创建标准案例工作区子目录
    let local_dirs = [
        "kb/techniques",
        "kb/signatures",
        "kb/pitfalls",
        "scripts",
        "state",
        ".orev",
    ];

    for dir in &local_dirs {
        let dir_path = workspace_dir.join(dir);
        let already_directory = dir_path.is_dir();

        // `fs::create_dir_all` 确保：
        // - 不存在时创建。
        // - 已有目录时正常成功，包括指向有效目录的链接。
        // - 已有普通文件时失败。
        if let Err(e) = fs::create_dir_all(&dir_path) {
            eprintln!(
                "Error: Failed to ensure directory `{}`: {}",
                dir_path.display(),
                e
            );
            return Err(e);
        }

        if already_directory {
            println!("  [exists]  {}/", dir);
        } else {
            println!("  [created] {}/", dir);
        }
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

    for stage in ["60_diff", "90_report"] {
        if !config.stages.iter().any(|enabled| enabled == stage) {
            continue;
        }

        let stage_dir = workspace_dir.join(stage);

        for slot in ["out", "log"] {
            let directory = stage_dir.join(slot);
            let already_directory = directory.is_dir();

            if let Err(e) = fs::create_dir_all(&directory) {
                eprintln!(
                    "Error: Failed to ensure directory `{}`: {}",
                    directory.display(),
                    e
                );
                return Err(e);
            }

            if already_directory {
                println!("  [exists]  {}/{}/", stage, slot);
            } else {
                println!("  [created] {}/{}/", stage, slot);
            }
        }

        let stage_path = stage_dir.join("stage.yaml");

        let content = format!(
            "schema: orev/stage@v1.2\n\
stage: {stage}\n\
target_id: null\n\
status: pending\n\
started_at: null\n\
finished_at: null\n\
duration_s: null\n\
tool: null\n\
inputs: []\n\
outputs: []\n\
notes: \"\"\n"
        );

        if create_file_if_absent(&stage_path, |file| file.write_all(content.as_bytes()))? {
            println!("  [created] {}/stage.yaml", stage);
        } else {
            println!("  [exists]  {}/stage.yaml", stage);
        }
    }

    // 5. 生成高层导引 CASE.md
    let case_md_path = workspace_dir.join("CASE.md");
    let case_md_content = format!(
        r#"# Analysis Case: {}

## 1. Core Question (要回答什么问题)
- 

## 2. Executive Conclusion (核心结论)
- Status: In Progress
- Summary: TBD

## 3. Reference Links
- Investigation Threads: `state/investigation.jsonl`
- Findings (glob, relative to case root): `*/*/50_findings/out/findings.jsonl`
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
    for relative_path in ["state/index.jsonl", "state/investigation.jsonl"] {
        let file_path = workspace_dir.join(relative_path);

        if create_file_if_absent(&file_path, |_| Ok(()))? {
            println!("  [created] {}", relative_path);
        } else {
            println!("  [exists]  {}", relative_path);
        }
    }

    // 7. 生成专属 .gitignore
    let gitignore_path = workspace_dir.join(".gitignore");
    let gitignore_content = r#"# Orev Case Workspace Ignores
.orev/
store/
.cache/
log/
state/env.md

# Original inputs stay out of Git; metadata remains trackable
/*/*/00_inbox/*
!/*/*/00_inbox/stage.yaml
!/*/*/00_inbox/provenance.json
!/*/*/00_inbox/checksums.sha256

# Extracted filesystems
/*/*/10_unpack/out/filesystem/

# Tool working data; scripts and extensions remain trackable
/tools/*/working/
*.gpr
*.rep/
*.i64
*.bndb
*.rzdb

# Large runtime artifacts
/*/*/40_dynamic/out/trace/
/*/*/40_dynamic/out/queue/
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

    println!("\nSuccess! Standalone Binary Analysis Case workspace is ready.");
    println!("Use `orev target add <path>` to import binary/firmware/sample targets.");
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
