use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

mod support;

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "status: {}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}

fn assert_init_path_conflict(workspace: &Path, conflict: &Path) {
    let output = support::orev()
        .arg("init")
        .arg(workspace)
        .output()
        .expect("failed to run orev init");
    assert_eq!(
        output.status.code(),
        Some(1),
        "status: {}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );

    let relative = conflict
        .strip_prefix(workspace)
        .expect("conflict must be inside the case")
        .components()
        .collect::<PathBuf>();
    let stderr = String::from_utf8_lossy(&output.stderr);
    let relative = relative.display().to_string();
    assert!(
        stderr.contains("Error:") && stderr.contains(&relative),
        "error must identify the conflicting path:\n{stderr}",
    );
    assert!(!String::from_utf8_lossy(&output.stdout).contains("Success!"));
}

fn run_git(workspace: &Path, empty_config: &Path, args: &[&str]) -> Output {
    Command::new("git")
        .current_dir(workspace)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", empty_config)
        .env_remove("GIT_CONFIG_PARAMETERS")
        .env_remove("GIT_CONFIG_COUNT")
        .env_remove("GIT_DIR")
        .env_remove("GIT_COMMON_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .arg("-c")
        .arg(format!("core.excludesFile={}", empty_config.display()))
        .args(args)
        .output()
        .expect("failed to run git; Git must be installed to run this test")
}

fn assert_ignored(workspace: &Path, empty_config: &Path, relative_path: &str, expected: bool) {
    let file_path = workspace.join(relative_path);
    fs::create_dir_all(file_path.parent().expect("fixture must have a parent"))
        .expect("failed to create fixture directory");
    fs::write(&file_path, b"test fixture\n").expect("failed to write fixture");

    let output = run_git(
        workspace,
        empty_config,
        &["check-ignore", "--no-index", relative_path],
    );

    assert!(
        matches!(output.status.code(), Some(0) | Some(1)),
        "git check-ignore failed for {relative_path}: {}",
        String::from_utf8_lossy(&output.stderr),
    );

    assert_eq!(
        output.status.code() == Some(0),
        expected,
        "unexpected ignore result for {relative_path}\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}

#[test]
fn init_gitignore_excludes_temporary_data_but_preserves_case_files_and_scripts() {
    let temp = tempfile::tempdir().expect("failed to create temporary directory");
    let workspace = temp.path().join("case");
    let empty_config = temp.path().join("empty-git-config");
    fs::write(&empty_config, b"").expect("failed to create empty Git config");

    let output = support::orev()
        .arg("init")
        .arg(&workspace)
        .output()
        .expect("failed to run orev init");
    assert_success(&output);

    let output = run_git(
        &workspace,
        &empty_config,
        &["init", "--quiet", "--template="],
    );
    assert_success(&output);

    for path in [
        ".orev/cache",
        "store/data",
        ".cache/temp",
        "log/session.txt",
        "state/env.md",
        "tools/custom/working/project.db",
    ] {
        assert_ignored(&workspace, &empty_config, path, true);
    }

    for path in [
        "scripts/helper.py",
        "scripts/ghidra/helper.py",
        "tools/custom/scripts/helper.py",
        "tools/custom/extensions/plugin.txt",
        "kb/techniques/example.md",
        "state/index.yaml",
        "state/investigation.yaml",
        "attachments/report.txt",
    ] {
        assert_ignored(&workspace, &empty_config, path, false);
    }
}

#[test]
fn init_workspace_creates_only_multi_case_scaffolding() {
    for mode in ["absolute", "relative", "current"] {
        let temp = tempfile::tempdir().expect("failed to create temporary directory");
        let workspace = temp.path().join("project");
        let mut command = support::orev();
        command.args(["init", "--workspace"]);
        match mode {
            "absolute" => {
                command.arg(&workspace);
            }
            "relative" => {
                command.current_dir(temp.path()).arg("project");
            }
            _ => {
                fs::create_dir(&workspace).expect("failed to create workspace directory");
                command.current_dir(&workspace);
            }
        }
        let output = command.output().expect("failed to initialize workspace");
        assert_success(&output);

        let mut entries = fs::read_dir(&workspace)
            .expect("failed to inspect workspace")
            .map(|entry| entry.expect("failed to inspect entry").file_name())
            .collect::<Vec<_>>();
        entries.sort();
        let expected: Vec<std::ffi::OsString> = [
            ".gitignore",
            "README.md",
            "cases",
            "kb",
            "orev.toml",
            "store",
            "tools",
        ]
        .into_iter()
        .map(Into::into)
        .collect();
        assert_eq!(entries, expected);
        for directory in [
            "cases",
            "kb/techniques",
            "kb/signatures",
            "kb/pitfalls",
            "store",
            "tools",
        ] {
            assert!(workspace.join(directory).is_dir(), "{directory} must exist");
            assert_eq!(
                fs::read_dir(workspace.join(directory)).unwrap().count(),
                0,
                "{directory} must be empty at initialization",
            );
        }
        let readme = fs::read_to_string(workspace.join("README.md")).unwrap();
        assert!(readme.contains("cases/"));
        assert!(readme.contains("no automatic"));
        assert!(String::from_utf8_lossy(&output.stdout).contains("Success!"));
    }
}

#[test]
fn init_workspace_rejects_removed_name_option_before_creating_resources() {
    let temp = tempfile::tempdir().expect("failed to create temporary directory");
    let workspace = temp.path().join("project");
    let output = support::orev()
        .args(["init", "--workspace", "--name", "project"])
        .arg(&workspace)
        .output()
        .expect("failed to run orev init");
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("unexpected argument '--name'"));
    assert!(!workspace.exists());
}

#[test]
fn init_help_exposes_only_path_and_workspace_mode() {
    let output = support::orev()
        .args(["init", "--help"])
        .output()
        .expect("failed to read init help");
    assert_success(&output);
    let help = String::from_utf8_lossy(&output.stdout);
    assert!(help.contains("[PATH]") && help.contains("--workspace"));
    assert!(!help.contains("--name"));
}

#[test]
fn init_workspace_preserves_existing_files_and_cases_on_reinit() {
    let temp = tempfile::tempdir().expect("failed to create temporary directory");
    let workspace = temp.path().join("project");
    fs::create_dir_all(workspace.join("cases/existing")).unwrap();
    let fixtures = [
        ("README.md", "Existing project notes\n"),
        (".gitignore", "existing-pattern/\n"),
        ("cases/existing/CASE.md", "Existing case guide\n"),
        ("cases/existing/orev.toml", "existing case config\n"),
    ];
    for (path, content) in fixtures {
        fs::write(workspace.join(path), content).unwrap();
    }
    for _ in 0..2 {
        let output = support::orev()
            .args(["init", "--workspace"])
            .arg(&workspace)
            .output()
            .expect("failed to initialize workspace");
        assert_success(&output);
        for (path, content) in fixtures {
            assert_eq!(fs::read(workspace.join(path)).unwrap(), content.as_bytes());
        }
        assert_eq!(fs::read_dir(workspace.join("cases")).unwrap().count(), 1);
    }
}

#[test]
fn init_workspace_rejects_case_roots_without_creating_resources() {
    for marker in ["orev.toml", "CASE.md"] {
        let temp = tempfile::tempdir().expect("failed to create temporary directory");
        let workspace = temp.path();
        fs::write(workspace.join(marker), b"existing case\n").unwrap();
        let output = support::orev()
            .args(["init", "--workspace"])
            .arg(workspace)
            .output()
            .expect("failed to initialize workspace");
        assert_eq!(output.status.code(), Some(1));
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains(marker) && stderr.contains("case"));
        assert!(!String::from_utf8_lossy(&output.stdout).contains("Success!"));
        assert_eq!(fs::read_dir(workspace).unwrap().count(), 1);
        assert_eq!(
            fs::read(workspace.join(marker)).unwrap(),
            b"existing case\n"
        );
    }
}

#[test]
fn init_workspace_rejects_path_type_conflicts_without_overwriting() {
    for relative in [
        "cases",
        "kb",
        "kb/techniques",
        "kb/signatures",
        "kb/pitfalls",
        "store",
        "tools",
        "README.md",
        ".gitignore",
    ] {
        let temp = tempfile::tempdir().expect("failed to create temporary directory");
        let workspace = temp.path().join("project");
        let conflict = workspace.join(relative);
        fs::create_dir_all(conflict.parent().unwrap()).unwrap();
        let directory_conflict = matches!(relative, "README.md" | ".gitignore");
        if directory_conflict {
            fs::create_dir(&conflict).unwrap();
        } else {
            fs::write(&conflict, b"preserve this\n").unwrap();
        }
        let output = support::orev()
            .args(["init", "--workspace"])
            .arg(&workspace)
            .output()
            .expect("failed to initialize workspace");
        assert_eq!(output.status.code(), Some(1));
        assert!(String::from_utf8_lossy(&output.stderr).contains(relative));
        assert!(!String::from_utf8_lossy(&output.stdout).contains("Success!"));
        if directory_conflict {
            assert!(conflict.is_dir());
            assert_eq!(fs::read_dir(conflict).unwrap().count(), 0);
        } else {
            assert_eq!(fs::read(conflict).unwrap(), b"preserve this\n");
        }
    }
}

#[test]
fn init_workspace_gitignore_covers_nested_cases_and_shared_assets() {
    let temp = tempfile::tempdir().expect("failed to create temporary directory");
    let workspace = temp.path().join("project");
    let empty_config = temp.path().join("empty-git-config");
    fs::write(&empty_config, b"").unwrap();
    let output = support::orev()
        .args(["init", "--workspace"])
        .arg(&workspace)
        .output()
        .expect("failed to initialize workspace");
    assert_success(&output);
    assert_success(&run_git(
        &workspace,
        &empty_config,
        &["init", "--quiet", "--template="],
    ));

    for case in ["case-a", "case-b"] {
        for path in ["state/env.md", ".orev/cache", ".cache/temp"] {
            assert_ignored(
                &workspace,
                &empty_config,
                &format!("cases/{case}/{path}"),
                true,
            );
        }
        for path in [
            "orev.toml",
            "CASE.md",
            "state/index.yaml",
            "scripts/ghidra/helper.py",
        ] {
            assert_ignored(
                &workspace,
                &empty_config,
                &format!("cases/{case}/{path}"),
                false,
            );
        }
    }
    for path in ["store/data.bin", "tools/ghidra/working/case-a/project.db"] {
        assert_ignored(&workspace, &empty_config, path, true);
    }
    for path in [
        "README.md",
        "kb/techniques/note.md",
        "tools/ghidra/scripts/helper.py",
        "tools/ghidra/extensions/plugin.txt",
    ] {
        assert_ignored(&workspace, &empty_config, path, false);
    }
}

fn assert_invalid_config_has_no_side_effects(command: &str, config_content: &str) -> Output {
    let temp = tempfile::tempdir().expect("failed to create temporary directory");
    let workspace = temp.path().join("case");
    fs::create_dir(&workspace).expect("failed to create case directory");

    let config_path = workspace.join("orev.toml");
    fs::write(&config_path, config_content).expect("failed to write config fixture");

    let sentinel_path = workspace.join("sentinel.txt");
    fs::write(&sentinel_path, b"preserve this\n").expect("failed to write sentinel");

    let output = support::orev()
        .arg(command)
        .current_dir(&workspace)
        .env("PATH", "")
        .env_remove("GHIDRA_INSTALL_DIR")
        .output()
        .expect("failed to run orev");

    assert_eq!(
        output.status.code(),
        Some(1),
        "command: {command}\nstatus: {}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Failed to parse") && stderr.contains("orev.toml"),
        "expected a configuration error from {command}, got:\n{stderr}",
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains("Success!") && !stdout.contains("Doctor check completed."),
        "{command} must not report successful completion:\n{stdout}",
    );

    assert_eq!(
        fs::read(&config_path).expect("failed to read config"),
        config_content.as_bytes(),
        "invalid config must remain unchanged",
    );

    assert_eq!(
        fs::read(&sentinel_path).expect("failed to read sentinel"),
        b"preserve this\n",
        "existing user files must remain unchanged",
    );

    let entries = fs::read_dir(&workspace)
        .expect("failed to inspect case directory")
        .collect::<Result<Vec<_>, _>>()
        .expect("failed to read directory entries");

    assert_eq!(
        entries.len(),
        2,
        "invalid config must not cause additional workspace resources to be created",
    );

    output
}

#[test]
fn init_rejects_invalid_toml_before_creating_workspace_resources() {
    assert_invalid_config_has_no_side_effects("init", "[case\n");
}

#[test]
fn init_rejects_incomplete_config_before_creating_workspace_resources() {
    assert_invalid_config_has_no_side_effects("init", "[case]\n");
}

fn write_case_config(workspace: &Path) {
    fs::write(
        workspace.join("orev.toml"),
        r#"[case]
name = "configured-case"
version = "0.1.0"
investigators = []
"#,
    )
    .expect("failed to write config fixture");
}

#[test]
fn init_case_name_comes_from_existing_config() {
    let temp = tempfile::tempdir().expect("failed to create temporary directory");
    let workspace = temp.path().join("different-directory-name");
    fs::create_dir(&workspace).expect("failed to create case directory");
    write_case_config(&workspace);
    let original_config =
        fs::read(workspace.join("orev.toml")).expect("failed to read config fixture");

    let output = support::orev()
        .arg("init")
        .arg(&workspace)
        .output()
        .expect("failed to run orev init");
    assert_success(&output);

    assert!(
        String::from_utf8_lossy(&output.stdout).contains("Case Name: configured-case"),
        "initialization must display the configured case name",
    );
    let guide = fs::read_to_string(workspace.join("CASE.md")).expect("failed to read case guide");
    assert_eq!(
        guide.lines().next(),
        Some("# Analysis Case: configured-case")
    );
    assert_eq!(
        fs::read(workspace.join("orev.toml")).expect("failed to read config after init"),
        original_config,
    );
}

#[test]
fn status_reports_only_case_information() {
    let workspace = tempfile::tempdir().unwrap();
    write_case_config(workspace.path());
    let output = support::orev()
        .arg("status")
        .current_dir(workspace.path())
        .output()
        .expect("failed to run orev status");
    assert_success(&output);

    let stdout = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<_> = stdout.lines().collect();
    assert_eq!(lines.len(), 4, "{stdout}");
    assert_eq!(lines[0], "=== Orev Analysis Case Workspace Status ===");
    assert_eq!(lines[1], "Case Name:       configured-case");
    assert_eq!(lines[2], "Case Version:    0.1.0");
    let reported = lines[3].strip_prefix("Workspace Root:  ").unwrap();
    assert_eq!(
        fs::canonicalize(reported).unwrap(),
        fs::canonicalize(workspace.path()).unwrap()
    );
}

#[test]
fn status_reports_empty_and_populated_projects() {
    let workspace = tempfile::tempdir().unwrap();
    let init = support::orev()
        .args(["init", "--workspace"])
        .current_dir(workspace.path())
        .output()
        .unwrap();
    assert_success(&init);

    for names in [vec![], vec!["router audit", "case-\u{6848}\u{4ef6}"]] {
        for name in &names {
            let output = support::orev()
                .args(["case", "add", name])
                .current_dir(workspace.path())
                .output()
                .unwrap();
            assert_success(&output);
        }
        let config_path = workspace.path().join("orev.toml");
        let before = fs::read(&config_path).unwrap();
        let output = support::orev()
            .arg("status")
            .current_dir(workspace.path())
            .output()
            .unwrap();
        assert_success(&output);
        assert!(output.stderr.is_empty(), "{output:?}");
        let stdout = String::from_utf8_lossy(&output.stdout);
        let lines: Vec<_> = stdout.lines().collect();
        assert_eq!(lines.len(), 3 + names.len(), "{stdout}");
        assert_eq!(lines[0], "=== Orev Multi-Case Project Workspace Status ===");
        let reported = lines[1].strip_prefix("Workspace Root:  ").unwrap();
        assert_eq!(
            fs::canonicalize(reported).unwrap(),
            fs::canonicalize(workspace.path()).unwrap()
        );
        assert_eq!(lines[2], format!("Registered Cases: {}", names.len()));
        for (line, name) in lines[3..].iter().zip(&names) {
            assert_eq!(*line, format!("  - {name}"));
        }
        assert_eq!(fs::read(config_path).unwrap(), before);
    }
}

#[test]
fn status_and_doctor_reject_malformed_and_unknown_configuration_fields() {
    let workspace = tempfile::tempdir().unwrap();
    for content in [
        "[workspace\n",
        "[workspace]\ncases = \"not a list\"\n",
        "[workspace]\n",
        "[workspace]\ncases = []\nproject_root = \"..\"\n",
        "[workspace]\ncases = []\nunknown = true\n",
        "[workspace]\ncases = []\n[unknown]\n",
        "[workspace]\ncases = []\n[storage]\nunknown = true\n",
        "[workspace]\ncases = []\n[case]\nname = \"mixed\"\nversion = \"0.1.0\"\n",
        "[case]\nname = \"case\"\nversion = \"0.1.0\"\nunknown = true\n",
    ] {
        fs::write(workspace.path().join("orev.toml"), content).unwrap();
        for command in ["status", "doctor"] {
            let output = support::orev()
                .arg(command)
                .current_dir(workspace.path())
                .env("PATH", "")
                .env_remove("GHIDRA_INSTALL_DIR")
                .output()
                .unwrap();
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(
                stderr.contains("Error: Failed to parse"),
                "{command}\n{content}\n{stderr}"
            );
            let stdout = String::from_utf8_lossy(&output.stdout);
            assert!(!stdout.contains("configuration loaded and validated"));
            if command == "status" {
                assert!(output.stdout.is_empty(), "{output:?}");
            } else {
                assert_eq!(output.status.code(), Some(1), "{output:?}");
                assert!(!stdout.contains("Doctor check completed."));
            }
        }
    }
}

#[test]
fn doctor_checks_project_layout_without_requiring_case_directories() {
    let workspace = tempfile::tempdir().unwrap();
    let init = support::orev()
        .args(["init", "--workspace"])
        .current_dir(workspace.path())
        .output()
        .unwrap();
    assert_success(&init);
    for populated in [false, true] {
        if populated {
            let output = support::orev()
                .args(["case", "add", "first"])
                .current_dir(workspace.path())
                .output()
                .unwrap();
            assert_success(&output);
        }
        let before = fs::read(workspace.path().join("orev.toml")).unwrap();
        let output = support::orev()
            .arg("doctor")
            .current_dir(workspace.path())
            .env("PATH", "")
            .env_remove("GHIDRA_INSTALL_DIR")
            .output()
            .unwrap();
        assert_success(&output);
        assert!(output.stderr.is_empty(), "{output:?}");
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            stdout.contains("configuration loaded and validated."),
            "{stdout}"
        );
        assert!(stdout.contains("Doctor check completed."), "{stdout}");
        let directories: Vec<_> = stdout
            .lines()
            .filter(|line| line.starts_with("[\u{2713}] Directory"))
            .collect();
        assert_eq!(directories.len(), 4, "{stdout}");
        let root = fs::canonicalize(workspace.path()).unwrap();
        for name in ["kb", "store", "tools", "cases"] {
            let expected = format!(
                "[\u{2713}] Directory `{}` exists.",
                root.join(name).display()
            );
            assert!(directories.contains(&expected.as_str()), "{stdout}");
        }
        assert!(!stdout.contains("is missing."), "{stdout}");
        for name in ["scripts", "state", ".orev"] {
            assert!(!workspace.path().join(name).exists());
        }
        assert_eq!(
            fs::read(workspace.path().join("orev.toml")).unwrap(),
            before
        );
    }

    fs::remove_dir_all(workspace.path().join("cases/first")).unwrap();
    fs::remove_dir(workspace.path().join("cases")).unwrap();
    fs::remove_dir_all(workspace.path().join("kb")).unwrap();
    fs::write(workspace.path().join("kb"), b"not a directory").unwrap();
    let output = support::orev()
        .arg("doctor")
        .current_dir(workspace.path())
        .env("PATH", "")
        .env_remove("GHIDRA_INSTALL_DIR")
        .output()
        .unwrap();
    assert_success(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("[!] Expected a directory at"), "{stdout}");
    assert!(stdout.contains("is missing."), "{stdout}");
}

#[test]
fn doctor_resolves_and_validates_project_storage_roots() {
    for kind in ["store", "tools"] {
        let temp = tempfile::tempdir().unwrap();
        let workspace = temp.path().join("project");
        let external = temp.path().join(format!("external-{kind}"));
        fs::create_dir(&external).unwrap();
        let init = support::orev()
            .args(["init", "--workspace"])
            .arg(&workspace)
            .output()
            .unwrap();
        assert_success(&init);
        let config_path = workspace.join("orev.toml");
        let mut config: toml::Value =
            toml::from_str(&fs::read_to_string(&config_path).unwrap()).unwrap();
        for (root, valid) in [
            (external.clone(), true),
            (PathBuf::from("relative"), false),
            (temp.path().join("missing"), false),
            (workspace.join("kb"), false),
        ] {
            config["storage"].as_table_mut().unwrap().insert(
                format!("{kind}_root"),
                toml::Value::String(root.to_str().unwrap().to_string()),
            );
            let content = toml::to_string_pretty(&config).unwrap();
            fs::write(&config_path, &content).unwrap();
            let output = support::orev()
                .arg("doctor")
                .current_dir(&workspace)
                .env("PATH", "")
                .env_remove("GHIDRA_INSTALL_DIR")
                .output()
                .unwrap();
            if valid {
                assert_success(&output);
                assert!(output.stderr.is_empty(), "{output:?}");
                let stdout = String::from_utf8_lossy(&output.stdout);
                let resolved = fs::canonicalize(root).unwrap();
                assert!(
                    stdout.contains(&format!("Directory `{}` exists.", resolved.display())),
                    "{stdout}"
                );
            } else {
                assert_eq!(output.status.code(), Some(1), "{output:?}");
                let stderr = String::from_utf8_lossy(&output.stderr);
                assert!(
                    stderr.contains(&format!("Failed to resolve storage.{kind}_root")),
                    "{stderr}"
                );
                assert!(
                    !String::from_utf8_lossy(&output.stdout).contains("Doctor check completed.")
                );
            }
            assert_eq!(fs::read_to_string(&config_path).unwrap(), content);
        }
    }
}

#[test]
fn init_rejects_removed_name_option_without_modifying_existing_case() {
    let temp = tempfile::tempdir().expect("failed to create temporary directory");
    let workspace = temp.path().join("case");
    fs::create_dir(&workspace).expect("failed to create case directory");
    write_case_config(&workspace);
    let original_config =
        fs::read(workspace.join("orev.toml")).expect("failed to read config fixture");
    fs::write(workspace.join("sentinel.txt"), b"preserve this\n")
        .expect("failed to write sentinel");

    let output = support::orev()
        .args(["init", "--name", "conflicting-name"])
        .arg(&workspace)
        .output()
        .expect("failed to run orev init");

    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("unexpected argument '--name'"),
        "removed option must be rejected:\n{stderr}",
    );
    assert!(!String::from_utf8_lossy(&output.stdout).contains("Success!"));
    assert_eq!(
        fs::read(workspace.join("orev.toml")).expect("failed to read config after init"),
        original_config,
    );
    assert_eq!(
        fs::read(workspace.join("sentinel.txt")).expect("failed to read sentinel"),
        b"preserve this\n",
    );
    assert_eq!(
        fs::read_dir(&workspace)
            .expect("failed to inspect case directory")
            .count(),
        2,
        "removed option must not create workspace resources",
    );
}

#[test]
fn init_and_doctor_reject_unknown_configuration_fields() {
    let fixture = tempfile::tempdir().expect("failed to create fixture directory");
    write_case_config(fixture.path());

    let content = fs::read_to_string(fixture.path().join("orev.toml"))
        .expect("failed to read valid config fixture");
    let baseline: toml::Value =
        toml::from_str(&content).expect("failed to parse valid config fixture");

    for (section, field, fragment) in [
        ("", "unexpected", "unexpected = true"),
        ("", "workspace", "[workspace]\nkb_dir = \"notes\""),
        ("case", "nam", "nam = \"typo\""),
        (
            "storage",
            "store_rooot",
            "store_rooot = \"/external/store\"",
        ),
        (
            "storage",
            "tools_rooot",
            "tools_rooot = \"/external/tools\"",
        ),
    ] {
        let mut config = baseline.clone();
        let mut extra: toml::Table =
            toml::from_str(fragment).expect("failed to parse unknown-field fixture");
        let value = extra
            .remove(field)
            .expect("fixture must contain tested field");

        let root = config
            .as_table_mut()
            .expect("config fixture must be a table");

        let destination = if section.is_empty() {
            root
        } else {
            root.entry(section.to_string())
                .or_insert_with(|| toml::Value::Table(toml::Table::new()))
                .as_table_mut()
                .expect("config section must be a table")
        };

        destination.insert(field.to_string(), value);

        let content = toml::to_string_pretty(&config).expect("failed to serialize config fixture");

        for command in ["init", "doctor"] {
            let output = assert_invalid_config_has_no_side_effects(command, &content);
            let stderr = String::from_utf8_lossy(&output.stderr);

            assert!(
                stderr.contains(&format!("unknown field `{field}`")),
                "{command} must identify unknown field {field}:\n{stderr}",
            );
        }
    }
}

#[test]
fn init_generates_only_case_configuration_and_layout() {
    let temp = tempfile::tempdir().expect("failed to create temporary directory");
    let workspace = temp.path();

    let output = support::orev()
        .arg("init")
        .arg(workspace)
        .output()
        .expect("failed to run orev init");
    assert_success(&output);

    let content =
        fs::read_to_string(workspace.join("orev.toml")).expect("failed to read generated config");
    let config: toml::Value =
        toml::from_str(&content).expect("generated config must be valid TOML");

    let fields = config.as_table().expect("config must be a table");
    assert_eq!(fields.len(), 2);
    assert!(fields.contains_key("case"));
    assert!(fields.contains_key("storage"));

    let mut entries = fs::read_dir(workspace)
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect::<Vec<_>>();
    entries.sort();
    let expected: Vec<std::ffi::OsString> = [
        ".gitignore",
        ".orev",
        "CASE.md",
        "kb",
        "orev.toml",
        "scripts",
        "state",
        "store",
        "tools",
    ]
    .into_iter()
    .map(Into::into)
    .collect();
    assert_eq!(entries, expected);

    for directory in [
        "kb/techniques",
        "kb/signatures",
        "kb/pitfalls",
        "store",
        "tools",
        "state",
        ".orev",
        "scripts",
    ] {
        assert!(
            workspace.join(directory).is_dir(),
            "missing fixed-layout directory: {directory}",
        );
    }
    assert_eq!(
        fs::read_dir(workspace.join("scripts"))
            .expect("failed to inspect case scripts directory")
            .count(),
        0,
        "initialization must not generate scripts or tool-specific directories",
    );
}

fn write_config_with_storage_root(workspace: &Path, field: &str, root: &Path) {
    write_case_config(workspace);

    let config_path = workspace.join("orev.toml");
    let content = fs::read_to_string(&config_path).expect("failed to read config fixture");
    let mut config: toml::Value = toml::from_str(&content).expect("failed to parse config fixture");

    let mut storage = toml::Table::new();
    storage.insert(
        field.to_string(),
        toml::Value::String(
            root.to_str()
                .expect("fixture path must be valid UTF-8")
                .to_string(),
        ),
    );

    config
        .as_table_mut()
        .expect("config fixture must be a table")
        .insert("storage".to_string(), toml::Value::Table(storage));

    let content = toml::to_string_pretty(&config).expect("failed to serialize config fixture");
    fs::write(config_path, content).expect("failed to write config fixture");
}

#[test]
fn init_and_doctor_use_existing_external_storage_roots() {
    for kind in ["store", "tools"] {
        let temp = tempfile::tempdir().expect("failed to create temporary directory");
        let workspace = temp.path().join("case");
        let external_root = temp.path().join(format!("external-{kind}"));
        let field = format!("{kind}_root");

        fs::create_dir(&workspace).expect("failed to create case directory");
        fs::create_dir(&external_root).expect("failed to create external root");
        fs::write(external_root.join("sentinel.txt"), b"preserve this\n")
            .expect("failed to write sentinel");

        write_config_with_storage_root(&workspace, &field, &external_root);

        let output = support::orev()
            .arg("init")
            .arg(&workspace)
            .output()
            .expect("failed to run orev init");
        assert_success(&output);

        assert!(
            !workspace.join(kind).exists(),
            "external {kind} must not create a fallback local directory",
        );

        let other = if kind == "store" { "tools" } else { "store" };
        assert!(
            workspace.join(other).is_dir(),
            "unconfigured {other} must retain its default local directory",
        );

        assert_eq!(
            fs::read(external_root.join("sentinel.txt")).expect("failed to read sentinel"),
            b"preserve this\n",
            "existing external data must remain unchanged",
        );

        let output = support::orev()
            .arg("doctor")
            .current_dir(&workspace)
            .env("PATH", "")
            .env_remove("GHIDRA_INSTALL_DIR")
            .output()
            .expect("failed to run orev doctor");
        assert_success(&output);

        let stdout = String::from_utf8_lossy(&output.stdout);
        let resolved_root =
            fs::canonicalize(&external_root).expect("failed to resolve external root");

        assert!(
            stdout.contains(&resolved_root.display().to_string()),
            "doctor must inspect configured {field}:\n{stdout}",
        );

        assert!(
            !stdout.contains("is missing."),
            "doctor must not require unused local directories:\n{stdout}",
        );
    }
}

#[test]
fn init_rejects_invalid_external_storage_roots_before_creating_resources() {
    for kind in ["store", "tools"] {
        let temp = tempfile::tempdir().expect("failed to create temporary directory");
        let workspace = temp.path().join("case");
        let missing = temp.path().join("missing-root");
        let regular_file = temp.path().join("not-a-directory");
        let inside_case = workspace.join("inside");
        let field = format!("{kind}_root");

        fs::create_dir(&workspace).expect("failed to create case directory");
        fs::create_dir(&inside_case).expect("failed to create inside directory");
        fs::write(&regular_file, b"preserve this\n").expect("failed to create file fixture");

        for root in [
            PathBuf::new(),
            PathBuf::from("relative-root"),
            missing.clone(),
            regular_file.clone(),
            workspace.clone(),
            inside_case,
        ] {
            write_config_with_storage_root(&workspace, &field, &root);

            let config_path = workspace.join("orev.toml");
            let original_config = fs::read(&config_path).expect("failed to read config");
            let original_entries = fs::read_dir(&workspace)
                .expect("failed to inspect case directory")
                .collect::<Result<Vec<_>, _>>()
                .expect("failed to read directory entries");

            let output = support::orev()
                .arg("init")
                .arg(&workspace)
                .output()
                .expect("failed to run orev init");

            assert_eq!(
                output.status.code(),
                Some(1),
                "invalid {field} accepted: {root:?}\nstdout:\n{}\nstderr:\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr),
            );

            assert!(
                String::from_utf8_lossy(&output.stderr).contains(&format!("storage.{field}")),
                "error must identify the invalid configuration field",
            );

            assert!(!String::from_utf8_lossy(&output.stdout).contains("Success!"));

            assert_eq!(
                fs::read(&config_path).expect("failed to read config after init"),
                original_config,
            );

            let remaining_entries = fs::read_dir(&workspace)
                .expect("failed to inspect case directory after init")
                .collect::<Result<Vec<_>, _>>()
                .expect("failed to read directory entries");

            assert_eq!(
                remaining_entries.len(),
                original_entries.len(),
                "invalid {field} must not create workspace resources",
            );

            assert!(!missing.exists(), "external roots must not be auto-created");
        }

        assert_eq!(
            fs::read(regular_file).expect("failed to read file fixture"),
            b"preserve this\n",
        );
    }
}

#[test]
fn init_preserves_existing_case_scripts_on_reinit() {
    let temp = tempfile::tempdir().expect("failed to create temporary directory");
    let workspace = temp.path().join("case");
    let script_path = workspace.join("scripts/ghidra/custom.py");
    let script_content = b"# existing case-specific analysis script\n";

    fs::create_dir_all(script_path.parent().expect("script must have a parent"))
        .expect("failed to create scripts fixture");
    fs::write(&script_path, script_content).expect("failed to write script fixture");

    for _ in 0..2 {
        let output = support::orev()
            .arg("init")
            .arg(&workspace)
            .output()
            .expect("failed to run orev init");
        assert_success(&output);

        assert_eq!(
            fs::read(&script_path).expect("failed to read existing script"),
            script_content,
            "initialization must preserve existing case scripts",
        );
    }
}

#[test]
fn init_rejects_a_regular_file_at_case_scripts_directory() {
    let temp = tempfile::tempdir().expect("failed to create temporary directory");
    let workspace = temp.path().join("case");
    let scripts_path = workspace.join("scripts");
    let original_content = b"preserve this file\n";

    fs::create_dir(&workspace).expect("failed to create case directory");
    fs::write(&scripts_path, original_content).expect("failed to create conflict fixture");

    let output = support::orev()
        .arg("init")
        .arg(&workspace)
        .output()
        .expect("failed to run orev init");

    assert_eq!(
        output.status.code(),
        Some(1),
        "status: {}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Failed to ensure directory") && stderr.contains("scripts"),
        "error must identify the directory conflict:\n{stderr}",
    );

    assert!(
        !String::from_utf8_lossy(&output.stdout).contains("Success!"),
        "initialization must not report success",
    );

    assert_eq!(
        fs::read(&scripts_path).expect("failed to read conflict fixture"),
        original_content,
        "initialization must not overwrite the conflicting file",
    );
}

#[test]
fn doctor_reports_case_scripts_directory_presence() {
    let temp = tempfile::tempdir().expect("failed to create temporary directory");
    let workspace = temp.path().join("case");

    let output = support::orev()
        .arg("init")
        .arg(&workspace)
        .output()
        .expect("failed to run orev init");
    assert_success(&output);

    let scripts_path = fs::canonicalize(&workspace)
        .expect("failed to resolve case directory")
        .join("scripts");

    for exists in [true, false] {
        if !exists {
            fs::remove_dir(&scripts_path).expect("failed to remove empty scripts directory");
        }

        let output = support::orev()
            .arg("doctor")
            .current_dir(&workspace)
            .env("PATH", "")
            .env_remove("GHIDRA_INSTALL_DIR")
            .output()
            .expect("failed to run orev doctor");
        assert_success(&output);

        let stdout = String::from_utf8_lossy(&output.stdout);
        let sections: Vec<_> = stdout
            .lines()
            .filter(|line| line.starts_with("--- ") && line.ends_with(" ---"))
            .collect();
        assert_eq!(
            sections,
            [
                "--- System Environment ---",
                "--- Runtime Dependencies ---",
                "--- External Toolchain & Versions ---",
            ]
        );
        let suffix = if exists { "` exists." } else { "` is missing." };
        let reported = stdout
            .lines()
            .find_map(|line| {
                let (_, path) = line.split_once("Directory `")?;
                let path = Path::new(path.strip_suffix(suffix)?);
                (path.file_name() == Some(std::ffi::OsStr::new("scripts"))).then_some(path)
            })
            .unwrap_or_else(|| {
                panic!("doctor must report case scripts directory presence:\n{stdout}")
            });
        assert_eq!(
            fs::canonicalize(reported.parent().unwrap()).unwrap(),
            fs::canonicalize(&workspace).unwrap(),
        );
    }
}

#[test]
fn concurrent_init_publishes_complete_case_files() {
    let temp = tempfile::tempdir().expect("failed to create temporary directory");
    let workspace = temp.path().join("case");
    fs::create_dir(&workspace).expect("failed to create case directory");
    write_case_config(&workspace);
    let original_config =
        fs::read(workspace.join("orev.toml")).expect("failed to read config fixture");

    let children: Vec<_> = (0..4)
        .map(|_| {
            support::orev()
                .arg("init")
                .arg(&workspace)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .expect("failed to start concurrent init")
        })
        .collect();
    let outputs: Vec<_> = children
        .into_iter()
        .map(|child| child.wait_with_output().expect("failed to wait for init"))
        .collect();

    for output in &outputs {
        assert_success(output);
    }
    assert_eq!(
        fs::read(workspace.join("orev.toml")).expect("failed to read config"),
        original_config,
    );
    let guide = fs::read_to_string(workspace.join("CASE.md")).expect("failed to read case guide");
    assert!(guide.starts_with("# Analysis Case: configured-case\n"));
    assert!(guide.ends_with("- Case-specific Scripts: `scripts/<tool>/`\n"));
    let ignores =
        fs::read_to_string(workspace.join(".gitignore")).expect("failed to read ignore rules");
    assert!(ignores.starts_with("# Orev Case Workspace Ignores\n"));
    assert!(ignores.ends_with("*.mem\n"));
    for ledger in ["state/index.yaml", "state/investigation.yaml"] {
        assert_eq!(fs::read(workspace.join(ledger)).unwrap(), b"[]\n");
        let value: serde_yaml_ng::Value =
            serde_yaml_ng::from_str(&fs::read_to_string(workspace.join(ledger)).unwrap()).unwrap();
        assert!(value.as_sequence().unwrap().is_empty());
    }
    for entry in fs::read_dir(&workspace).expect("failed to inspect case directory") {
        assert!(
            !entry
                .expect("failed to read directory entry")
                .file_name()
                .to_string_lossy()
                .starts_with(".orev-init-"),
            "successful initialization must not leave staging files",
        );
    }
}

#[test]
fn concurrent_init_uses_the_published_configuration_name() {
    let temp = tempfile::tempdir().expect("failed to create temporary directory");
    let workspace = temp.path().join("case");
    let children: Vec<_> = (0..4)
        .map(|_| {
            support::orev()
                .arg("init")
                .arg(&workspace)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .expect("failed to start concurrent init")
        })
        .collect();
    let outputs: Vec<_> = children
        .into_iter()
        .map(|child| child.wait_with_output().expect("failed to wait for init"))
        .collect();

    let content =
        fs::read_to_string(workspace.join("orev.toml")).expect("failed to read published config");
    let config: toml::Value =
        toml::from_str(&content).expect("published config must be complete TOML");
    let name = config["case"]["name"]
        .as_str()
        .expect("case name must be a string");

    assert_eq!(name, "case");
    for output in &outputs {
        assert_success(output);
        assert!(String::from_utf8_lossy(&output.stdout).contains("Case Name: case"));
    }
    let guide = fs::read_to_string(workspace.join("CASE.md")).expect("failed to read case guide");
    assert_eq!(
        guide.lines().next(),
        Some(format!("# Analysis Case: {name}").as_str()),
    );
}

#[test]
fn init_new_case_name_defaults_to_directory_with_or_without_a_path() {
    for explicit_path in [false, true] {
        let temp = tempfile::tempdir().expect("failed to create temporary directory");
        let workspace = temp.path().join("directory-name");
        fs::create_dir(&workspace).expect("failed to create case directory");

        let mut command = support::orev();
        command.arg("init");
        if explicit_path {
            command.arg(&workspace);
        } else {
            command.current_dir(&workspace);
        }
        let output = command.output().expect("failed to run orev init");
        assert_success(&output);

        let content =
            fs::read_to_string(workspace.join("orev.toml")).expect("failed to read config");
        let config: toml::Value = toml::from_str(&content).expect("config must be valid TOML");
        assert_eq!(config["case"]["name"].as_str(), Some("directory-name"));
    }
}

#[test]
#[cfg(unix)]
fn init_special_case_names_round_trip_without_rewriting_config() {
    for name in [
        "quotes \" and backslashes \\",
        "line one\nline two",
        "tab\tname",
        "case-\u{6848}\u{4ef6}",
    ] {
        let temp = tempfile::tempdir().expect("failed to create temporary directory");
        let workspace = temp.path().join(name);

        let output = support::orev()
            .arg("init")
            .arg(&workspace)
            .output()
            .expect("failed to run orev init");
        assert_success(&output);

        let original =
            fs::read(workspace.join("orev.toml")).expect("failed to read generated config");
        let config: toml::Value =
            toml::from_str(std::str::from_utf8(&original).expect("config must be UTF-8"))
                .expect("generated config must be valid TOML");
        assert_eq!(config["case"]["name"].as_str(), Some(name));

        let output = support::orev()
            .arg("init")
            .arg(&workspace)
            .output()
            .expect("failed to reinitialize case");
        assert_success(&output);
        assert_eq!(
            fs::read(workspace.join("orev.toml")).expect("failed to read config after reinit"),
            original,
        );
    }
}

#[test]
fn reinit_preserves_case_files() {
    let temp = tempfile::tempdir().expect("failed to create temporary directory");
    let workspace = temp.path().join("case");
    fs::create_dir(&workspace).expect("failed to create case directory");
    write_case_config(&workspace);

    let output = support::orev()
        .arg("init")
        .arg(&workspace)
        .output()
        .expect("failed to initialize case");
    assert_success(&output);

    for (path, content) in [
        ("CASE.md", "# Existing analyst guide\n"),
        (".gitignore", "# Existing rules\ncustom-cache/\n"),
        ("state/index.yaml", "# analyst comment\n- fixture: index\n"),
        (
            "state/investigation.yaml",
            "# analyst comment\n- fixture: investigation\n",
        ),
    ] {
        fs::write(workspace.join(path), content).expect("failed to write existing content");
    }
    let snapshots: Vec<_> = [
        "orev.toml",
        "CASE.md",
        ".gitignore",
        "state/index.yaml",
        "state/investigation.yaml",
    ]
    .into_iter()
    .map(|path| {
        (
            path,
            fs::read(workspace.join(path)).expect("failed to snapshot existing file"),
        )
    })
    .collect();

    for _ in 0..2 {
        let output = support::orev()
            .arg("init")
            .arg(&workspace)
            .output()
            .expect("failed to reinitialize case");
        assert_success(&output);

        for (path, original) in &snapshots {
            assert_eq!(
                fs::read(workspace.join(path)).expect("failed to read file after reinit"),
                *original,
                "reinitialization must preserve {path}",
            );
        }
    }
}

#[test]
fn init_rejects_directories_at_metadata_file_paths() {
    for relative in [
        "orev.toml",
        "CASE.md",
        ".gitignore",
        "state/index.yaml",
        "state/investigation.yaml",
    ] {
        let temp = tempfile::tempdir().expect("failed to create temporary directory");
        let workspace = temp.path().join("case");
        fs::create_dir(&workspace).expect("failed to create case directory");
        if relative != "orev.toml" {
            write_case_config(&workspace);
        }

        let conflict = workspace.join(relative);
        fs::create_dir_all(&conflict).expect("failed to create directory conflict");
        fs::write(conflict.join("sentinel.txt"), b"preserve this\n")
            .expect("failed to write directory sentinel");

        assert_init_path_conflict(&workspace, &conflict);

        assert!(conflict.is_dir());
        assert_eq!(
            fs::read(conflict.join("sentinel.txt")).expect("failed to read directory sentinel"),
            b"preserve this\n",
        );
    }
}

#[test]
fn init_rejects_files_at_required_directory_paths() {
    for relative in [
        "kb",
        "kb/techniques",
        "kb/signatures",
        "kb/pitfalls",
        "state",
        ".orev",
        "store",
        "tools",
    ] {
        let temp = tempfile::tempdir().expect("failed to create temporary directory");
        let workspace = temp.path().join("case");
        fs::create_dir(&workspace).expect("failed to create case directory");
        write_case_config(&workspace);

        let conflict = workspace.join(relative);
        fs::create_dir_all(conflict.parent().expect("conflict must have a parent"))
            .expect("failed to create conflict parent");
        fs::write(&conflict, b"preserve this\n").expect("failed to write file conflict");

        assert_init_path_conflict(&workspace, &conflict);

        assert_eq!(
            fs::read(&conflict).expect("failed to read file conflict"),
            b"preserve this\n",
        );
    }
}

#[cfg(unix)]
#[test]
fn init_preserves_valid_file_symlinks() {
    use std::os::unix::fs::symlink;

    for relative in [
        "orev.toml",
        "CASE.md",
        ".gitignore",
        "state/index.yaml",
        "state/investigation.yaml",
    ] {
        let temp = tempfile::tempdir().expect("failed to create temporary directory");
        let workspace = temp.path().join("case");
        let external = temp.path().join("external-file");
        fs::create_dir(&workspace).expect("failed to create case directory");
        write_case_config(&workspace);

        let link = workspace.join(relative);
        fs::create_dir_all(link.parent().expect("link must have a parent"))
            .expect("failed to create link parent");
        if relative == "orev.toml" {
            fs::rename(&link, &external).expect("failed to move config fixture");
        } else {
            fs::write(&external, b"existing external content\n")
                .expect("failed to create external file");
        }
        let original = fs::read(&external).expect("failed to read external file");
        symlink(&external, &link).expect("failed to create file symlink");

        let output = support::orev()
            .arg("init")
            .arg(&workspace)
            .output()
            .expect("failed to run orev init");
        assert_success(&output);

        assert!(
            fs::symlink_metadata(&link)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert_eq!(fs::read(&external).unwrap(), original);
    }
}

#[cfg(unix)]
#[test]
fn init_preserves_valid_directory_symlinks() {
    use std::os::unix::fs::symlink;

    for relative in ["kb", "scripts", "state", ".orev", "store", "tools"] {
        let temp = tempfile::tempdir().expect("failed to create temporary directory");
        let workspace = temp.path().join("case");
        let external = temp.path().join("external-directory");
        fs::create_dir(&workspace).expect("failed to create case directory");
        fs::create_dir(&external).expect("failed to create external directory");
        fs::write(external.join("sentinel.txt"), b"preserve this\n")
            .expect("failed to write external sentinel");
        write_case_config(&workspace);

        let link = workspace.join(relative);
        symlink(&external, &link).expect("failed to create directory symlink");

        let output = support::orev()
            .arg("init")
            .arg(&workspace)
            .output()
            .expect("failed to run orev init");
        assert_success(&output);

        assert!(
            fs::symlink_metadata(&link)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert_eq!(
            fs::read(external.join("sentinel.txt")).unwrap(),
            b"preserve this\n",
        );
    }
}

#[cfg(unix)]
#[test]
fn init_rejects_dangling_or_wrong_type_file_symlinks() {
    use std::os::unix::fs::symlink;

    for relative in [
        "orev.toml",
        "CASE.md",
        ".gitignore",
        "state/index.yaml",
        "state/investigation.yaml",
    ] {
        for target_directory in [false, true] {
            let temp = tempfile::tempdir().expect("failed to create temporary directory");
            let workspace = temp.path().join("case");
            let external = temp.path().join("external-target");
            fs::create_dir(&workspace).expect("failed to create case directory");
            write_case_config(&workspace);
            if target_directory {
                fs::create_dir(&external).expect("failed to create external directory");
            }

            let link = workspace.join(relative);
            fs::create_dir_all(link.parent().expect("link must have a parent"))
                .expect("failed to create link parent");
            if relative == "orev.toml" {
                fs::remove_file(&link).expect("failed to remove config fixture");
            }
            symlink(&external, &link).expect("failed to create invalid file symlink");

            assert_init_path_conflict(&workspace, &link);

            assert!(
                fs::symlink_metadata(&link)
                    .unwrap()
                    .file_type()
                    .is_symlink()
            );
            assert_eq!(external.try_exists().unwrap(), target_directory);
            if target_directory {
                assert_eq!(fs::read_dir(&external).unwrap().count(), 0);
            }
        }
    }
}

#[cfg(unix)]
#[test]
fn init_rejects_dangling_or_wrong_type_directory_symlinks() {
    use std::os::unix::fs::symlink;

    for relative in ["kb", "scripts", "state", ".orev", "store", "tools"] {
        for target_file in [false, true] {
            let temp = tempfile::tempdir().expect("failed to create temporary directory");
            let workspace = temp.path().join("case");
            let external = temp.path().join("external-target");
            fs::create_dir(&workspace).expect("failed to create case directory");
            write_case_config(&workspace);
            if target_file {
                fs::write(&external, b"preserve this\n").expect("failed to create external file");
            }

            let link = workspace.join(relative);
            symlink(&external, &link).expect("failed to create invalid directory symlink");

            assert_init_path_conflict(&workspace, &link);

            assert!(
                fs::symlink_metadata(&link)
                    .unwrap()
                    .file_type()
                    .is_symlink()
            );
            assert_eq!(external.try_exists().unwrap(), target_file);
            if target_file {
                assert_eq!(fs::read(&external).unwrap(), b"preserve this\n");
            }
        }
    }
}

#[cfg(unix)]
#[test]
fn init_and_doctor_resolve_external_storage_symlinks() {
    use std::os::unix::fs::symlink;

    for kind in ["store", "tools"] {
        let temp = tempfile::tempdir().expect("failed to create temporary directory");
        let workspace = temp.path().join("case");
        let external = temp.path().join("external-directory");
        let alias = temp.path().join("external-link");
        fs::create_dir(&workspace).expect("failed to create case directory");
        fs::create_dir(&external).expect("failed to create external directory");
        fs::write(external.join("sentinel.txt"), b"preserve this\n")
            .expect("failed to write external sentinel");
        symlink(&external, &alias).expect("failed to create external storage symlink");
        write_config_with_storage_root(&workspace, &format!("{kind}_root"), &alias);

        let output = support::orev()
            .arg("init")
            .arg(&workspace)
            .output()
            .expect("failed to run orev init");
        assert_success(&output);
        assert!(!workspace.join(kind).exists());

        let output = support::orev()
            .arg("doctor")
            .current_dir(&workspace)
            .env("PATH", "")
            .env_remove("GHIDRA_INSTALL_DIR")
            .output()
            .expect("failed to run orev doctor");
        assert_success(&output);

        let resolved = fs::canonicalize(&external).expect("failed to resolve external directory");
        assert!(String::from_utf8_lossy(&output.stdout).contains(&resolved.display().to_string()));
        assert_eq!(
            fs::read(external.join("sentinel.txt")).unwrap(),
            b"preserve this\n",
        );
    }
}

#[cfg(unix)]
#[test]
fn init_rejects_external_storage_symlinks_into_the_case() {
    use std::os::unix::fs::symlink;

    for kind in ["store", "tools"] {
        let temp = tempfile::tempdir().expect("failed to create temporary directory");
        let workspace = temp.path().join("case");
        let inside = workspace.join("inside");
        let alias = temp.path().join("external-link");
        fs::create_dir_all(&inside).expect("failed to create inside directory");
        symlink(&inside, &alias).expect("failed to create storage symlink");
        write_config_with_storage_root(&workspace, &format!("{kind}_root"), &alias);
        let original_config =
            fs::read(workspace.join("orev.toml")).expect("failed to read config fixture");

        let output = support::orev()
            .arg("init")
            .arg(&workspace)
            .output()
            .expect("failed to run orev init");

        assert_eq!(output.status.code(), Some(1));
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains(&format!("storage.{kind}_root"))
                && stderr.contains("outside the case directory"),
            "physical destinations inside the case must be rejected:\n{stderr}",
        );
        assert!(!String::from_utf8_lossy(&output.stdout).contains("Success!"));
        assert_eq!(
            fs::read(workspace.join("orev.toml")).unwrap(),
            original_config
        );
        assert_eq!(fs::read_dir(&workspace).unwrap().count(), 2);
        assert_eq!(fs::read_dir(&inside).unwrap().count(), 0);
    }
}
