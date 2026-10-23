use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

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
    let output = Command::new(env!("CARGO_BIN_EXE_orev"))
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
        .expect("conflict must be inside the case");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Error:") && stderr.contains(&relative.display().to_string()),
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
fn init_gitignore_excludes_inputs_but_preserves_metadata_and_scripts() {
    let temp = tempfile::tempdir().expect("failed to create temporary directory");
    let workspace = temp.path().join("case");
    let empty_config = temp.path().join("empty-git-config");
    fs::write(&empty_config, b"").expect("failed to create empty Git config");

    let output = Command::new(env!("CARGO_BIN_EXE_orev"))
        .args(["init", "--name", "ignore-test"])
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

    for kind in ["binary", "firmware", "sample"] {
        for filename in ["gnuture", "firmware.bin", "payload.bin", "input file.dat"] {
            let path = format!("{kind}/test_0123456789ab/00_inbox/{filename}");
            assert_ignored(&workspace, &empty_config, &path, true);
        }

        for filename in ["stage.yaml", "provenance.json", "checksums.sha256"] {
            let path = format!("{kind}/test_0123456789ab/00_inbox/{filename}");
            assert_ignored(&workspace, &empty_config, &path, false);
        }
    }

    for path in [
        "scripts/helper.py",
        "scripts/ghidra/helper.py",
        "tools/custom/scripts/helper.py",
        "tools/custom/extensions/plugin.txt",
        "kb/techniques/example.md",
        "state/index.jsonl",
        "binary/test_0123456789ab/20_static/out/headers.json",
        "binary/test_0123456789ab/50_findings/out/findings.jsonl",
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

    let output = Command::new(env!("CARGO_BIN_EXE_orev"))
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
    assert_invalid_config_has_no_side_effects("init", "stages = [\n");
}

#[test]
fn init_rejects_incomplete_config_before_creating_workspace_resources() {
    assert_invalid_config_has_no_side_effects("init", "stages = []\n");
}

fn write_config_with_stages(workspace: &Path, stages: &[&str]) {
    let mut config: toml::Value = toml::from_str(
        r#"stages = []

[case]
name = "stage-test"
version = "0.1.0"
investigators = []

[targets]
files = []
"#,
    )
    .expect("failed to parse config fixture");

    config["stages"] = toml::Value::Array(
        stages
            .iter()
            .map(|stage| toml::Value::String(stage.to_string()))
            .collect(),
    );

    let content = toml::to_string_pretty(&config).expect("failed to serialize config fixture");

    fs::write(workspace.join("orev.toml"), content).expect("failed to write config fixture");
}

#[test]
fn init_case_name_comes_from_existing_config() {
    for explicit_name in [None, Some("stage-test")] {
        let temp = tempfile::tempdir().expect("failed to create temporary directory");
        let workspace = temp.path().join("different-directory-name");
        fs::create_dir(&workspace).expect("failed to create case directory");
        write_config_with_stages(&workspace, &[]);
        let original_config =
            fs::read(workspace.join("orev.toml")).expect("failed to read config fixture");

        let mut command = Command::new(env!("CARGO_BIN_EXE_orev"));
        command.arg("init").arg(&workspace);
        if let Some(name) = explicit_name {
            command.args(["--name", name]);
        }
        let output = command.output().expect("failed to run orev init");
        assert_success(&output);

        assert!(
            String::from_utf8_lossy(&output.stdout).contains("Case Name: stage-test"),
            "initialization must display the configured case name",
        );
        let guide =
            fs::read_to_string(workspace.join("CASE.md")).expect("failed to read case guide");
        assert_eq!(guide.lines().next(), Some("# Analysis Case: stage-test"));
        assert_eq!(
            fs::read(workspace.join("orev.toml")).expect("failed to read config after init"),
            original_config,
        );
    }
}

#[test]
fn init_case_name_conflict_fails_before_creating_resources() {
    let temp = tempfile::tempdir().expect("failed to create temporary directory");
    let workspace = temp.path().join("case");
    fs::create_dir(&workspace).expect("failed to create case directory");
    write_config_with_stages(&workspace, &[]);
    let original_config =
        fs::read(workspace.join("orev.toml")).expect("failed to read config fixture");
    fs::write(workspace.join("sentinel.txt"), b"preserve this\n")
        .expect("failed to write sentinel");

    let output = Command::new(env!("CARGO_BIN_EXE_orev"))
        .args(["init", "--name", "conflicting-name"])
        .arg(&workspace)
        .output()
        .expect("failed to run orev init");

    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("conflicts with configured case name")
            && stderr.contains("conflicting-name")
            && stderr.contains("stage-test"),
        "expected an explicit name conflict:\n{stderr}",
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
        "name conflicts must not create workspace resources",
    );
}

#[test]
fn init_and_doctor_reject_unknown_configuration_fields() {
    let fixture = tempfile::tempdir().expect("failed to create fixture directory");
    write_config_with_stages(fixture.path(), &[]);

    let content = fs::read_to_string(fixture.path().join("orev.toml"))
        .expect("failed to read valid config fixture");
    let baseline: toml::Value =
        toml::from_str(&content).expect("failed to parse valid config fixture");

    for (section, field, fragment) in [
        ("", "stage", "stage = []"),
        ("", "workspace", "[workspace]\nkb_dir = \"notes\""),
        ("", "sample", "[sample]\nretention_days = 90"),
        (
            "",
            "diff",
            "[[diff]]\nname = \"old-to-new\"\nbase = \"a\"\ntarget = \"b\"\ntool = \"bindiff@8\"",
        ),
        ("case", "nam", "nam = \"typo\""),
        ("targets", "file", "file = []"),
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
fn init_and_doctor_reject_unknown_stage_names() {
    let fixture = tempfile::tempdir().expect("failed to create fixture directory");

    for invalid_stage in [
        "60_dif",
        "",
        " ",
        " 20_static",
        "20_static ",
        "20_STATIC",
        "70_custom",
        "../60_diff",
        "60_diff/out",
    ] {
        write_config_with_stages(fixture.path(), &["90_report", invalid_stage]);
        let content = fs::read_to_string(fixture.path().join("orev.toml"))
            .expect("failed to read invalid-stage fixture");

        for command in ["init", "doctor"] {
            let output = assert_invalid_config_has_no_side_effects(command, &content);
            let stderr = String::from_utf8_lossy(&output.stderr);

            assert!(
                stderr.contains(&format!("unsupported stage `{invalid_stage}`")),
                "{command} must identify the invalid stage:\n{stderr}",
            );
        }
    }
}

#[test]
fn init_creates_only_enabled_case_stages() {
    let selections: &[&[&str]] = &[
        &[],
        &["00_inbox"],
        &["10_unpack"],
        &["15_patch"],
        &["20_static"],
        &["30_decompile"],
        &["40_dynamic"],
        &["50_findings"],
        &["00_inbox", "20_static", "30_decompile", "50_findings"],
        &["60_diff"],
        &["90_report"],
        &["60_diff", "90_report"],
        &["20_static", "20_static"],
        &[
            "00_inbox",
            "10_unpack",
            "15_patch",
            "20_static",
            "30_decompile",
            "40_dynamic",
            "50_findings",
            "60_diff",
            "90_report",
        ],
    ];

    for stages in selections {
        let temp = tempfile::tempdir().expect("failed to create temporary directory");
        let workspace = temp.path();
        write_config_with_stages(workspace, stages);

        let config_path = workspace.join("orev.toml");
        let original_config = fs::read(&config_path).expect("failed to read config");

        let output = Command::new(env!("CARGO_BIN_EXE_orev"))
            .arg("init")
            .arg(workspace)
            .output()
            .expect("failed to run orev init");
        assert_success(&output);

        assert!(
            workspace.join("scripts").is_dir(),
            "case scripts directory must not depend on enabled stages",
        );

        for stage in ["60_diff", "90_report"] {
            let directory = workspace.join(stage);
            let enabled = stages.contains(&stage);

            assert_eq!(
                directory.exists(),
                enabled,
                "unexpected creation of {stage} with stages={stages:?}",
            );

            if enabled {
                assert!(directory.join("out").is_dir());
                assert!(directory.join("log").is_dir());
                assert!(directory.join("stage.yaml").is_file());

                let entries = fs::read_dir(&directory)
                    .expect("failed to inspect stage directory")
                    .collect::<Result<Vec<_>, _>>()
                    .expect("failed to read stage entries");

                assert_eq!(
                    entries.len(),
                    3,
                    "{stage} must contain only stage.yaml, out and log",
                );

                let content = fs::read_to_string(directory.join("stage.yaml"))
                    .expect("failed to read stage metadata");

                assert!(content.lines().any(|line| line == "status: pending"));
                assert!(content.lines().any(|line| line == "target_id: null"));
            }
        }

        assert_eq!(
            fs::read(&config_path).expect("failed to read config after init"),
            original_config,
            "initialization must not rewrite the configured stage selection",
        );
    }
}

#[test]
fn doctor_does_not_require_disabled_case_stages() {
    let temp = tempfile::tempdir().expect("failed to create temporary directory");
    let workspace = temp.path();

    write_config_with_stages(
        workspace,
        &["00_inbox", "20_static", "30_decompile", "50_findings"],
    );

    let output = Command::new(env!("CARGO_BIN_EXE_orev"))
        .arg("init")
        .arg(workspace)
        .output()
        .expect("failed to run orev init");
    assert_success(&output);

    let output = Command::new(env!("CARGO_BIN_EXE_orev"))
        .arg("doctor")
        .current_dir(workspace)
        .env("PATH", "")
        .env_remove("GHIDRA_INSTALL_DIR")
        .output()
        .expect("failed to run orev doctor");
    assert_success(&output);

    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        stdout.contains("Doctor check completed."),
        "doctor did not complete:\n{stdout}",
    );

    assert!(
        !stdout.contains("is missing."),
        "doctor must not require disabled case-stage directories:\n{stdout}",
    );
}

#[test]
fn init_generates_fixed_layout_without_workspace_directory_config() {
    let temp = tempfile::tempdir().expect("failed to create temporary directory");
    let workspace = temp.path();

    let output = Command::new(env!("CARGO_BIN_EXE_orev"))
        .arg("init")
        .arg(workspace)
        .output()
        .expect("failed to run orev init");
    assert_success(&output);

    let content =
        fs::read_to_string(workspace.join("orev.toml")).expect("failed to read generated config");
    let config: toml::Value =
        toml::from_str(&content).expect("generated config must be valid TOML");

    assert!(
        config.get("workspace").is_none(),
        "generated config must not contain obsolete directory configuration",
    );

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
    write_config_with_stages(workspace, &[]);

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

        let output = Command::new(env!("CARGO_BIN_EXE_orev"))
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

        let output = Command::new(env!("CARGO_BIN_EXE_orev"))
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

            let output = Command::new(env!("CARGO_BIN_EXE_orev"))
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
        let output = Command::new(env!("CARGO_BIN_EXE_orev"))
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

    let output = Command::new(env!("CARGO_BIN_EXE_orev"))
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

    let output = Command::new(env!("CARGO_BIN_EXE_orev"))
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

        let output = Command::new(env!("CARGO_BIN_EXE_orev"))
            .arg("doctor")
            .current_dir(&workspace)
            .env("PATH", "")
            .env_remove("GHIDRA_INSTALL_DIR")
            .output()
            .expect("failed to run orev doctor");
        assert_success(&output);

        let stdout = String::from_utf8_lossy(&output.stdout);
        let expected = if exists {
            format!("Directory `{}` exists.", scripts_path.display())
        } else {
            format!("Directory `{}` is missing.", scripts_path.display())
        };

        assert!(
            stdout.contains(&expected),
            "doctor must report case scripts directory presence:\n{stdout}",
        );
    }
}

#[test]
fn concurrent_init_publishes_complete_case_files() {
    let temp = tempfile::tempdir().expect("failed to create temporary directory");
    let workspace = temp.path().join("case");
    fs::create_dir(&workspace).expect("failed to create case directory");
    write_config_with_stages(&workspace, &["60_diff", "90_report"]);
    let original_config =
        fs::read(workspace.join("orev.toml")).expect("failed to read config fixture");

    let children: Vec<_> = (0..4)
        .map(|_| {
            Command::new(env!("CARGO_BIN_EXE_orev"))
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
    for stage in ["60_diff", "90_report"] {
        let content = fs::read_to_string(workspace.join(stage).join("stage.yaml"))
            .expect("failed to read stage metadata");
        assert_eq!(
            content,
            format!(
                "schema: orev/stage@v1.2\nstage: {stage}\ntarget_id: null\n\
status: pending\nstarted_at: null\nfinished_at: null\nduration_s: null\n\
tool: null\ninputs: []\noutputs: []\nnotes: \"\"\n"
            ),
        );
        assert_eq!(fs::read_dir(workspace.join(stage)).unwrap().count(), 3);
    }
    let guide = fs::read_to_string(workspace.join("CASE.md")).expect("failed to read case guide");
    assert!(guide.starts_with("# Analysis Case: stage-test\n"));
    assert!(guide.ends_with("- Case-specific Scripts: `scripts/<tool>/`\n"));
    let ignores =
        fs::read_to_string(workspace.join(".gitignore")).expect("failed to read ignore rules");
    assert!(ignores.starts_with("# Orev Case Workspace Ignores\n"));
    assert!(ignores.ends_with("*.mem\n"));
    for ledger in ["state/index.jsonl", "state/investigation.jsonl"] {
        assert_eq!(fs::read(workspace.join(ledger)).unwrap(), b"");
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
    let children: Vec<_> = ["alpha", "beta", "gamma", "delta"]
        .into_iter()
        .map(|name| {
            let child = Command::new(env!("CARGO_BIN_EXE_orev"))
                .args(["init", "--name", name])
                .arg(&workspace)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .expect("failed to start concurrent init");
            (name, child)
        })
        .collect();
    let outputs: Vec<_> = children
        .into_iter()
        .map(|(name, child)| {
            (
                name,
                child.wait_with_output().expect("failed to wait for init"),
            )
        })
        .collect();

    let content =
        fs::read_to_string(workspace.join("orev.toml")).expect("failed to read published config");
    let config: toml::Value =
        toml::from_str(&content).expect("published config must be complete TOML");
    let name = config["case"]["name"]
        .as_str()
        .expect("case name must be a string");

    assert_eq!(
        outputs
            .iter()
            .filter(|(_, output)| output.status.success())
            .count(),
        1,
        "only the process matching the published name may succeed: {outputs:?}",
    );
    for (requested, output) in &outputs {
        if *requested == name {
            assert_success(output);
        } else {
            assert_eq!(output.status.code(), Some(1));
            assert!(
                String::from_utf8_lossy(&output.stderr)
                    .contains("conflicts with configured case name"),
                "a losing creator must report a name conflict: {output:?}",
            );
        }
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

        let mut command = Command::new(env!("CARGO_BIN_EXE_orev"));
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
fn init_special_case_names_round_trip_without_rewriting_config() {
    for name in [
        "quotes \" and backslashes \\",
        "line one\nline two",
        "tab\tname",
        "case-\u{6848}\u{4ef6}",
    ] {
        let temp = tempfile::tempdir().expect("failed to create temporary directory");
        let workspace = temp.path().join("case");

        let output = Command::new(env!("CARGO_BIN_EXE_orev"))
            .args(["init", "--name", name])
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

        let output = Command::new(env!("CARGO_BIN_EXE_orev"))
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
fn reinit_preserves_case_files_and_disabled_stage_results() {
    let temp = tempfile::tempdir().expect("failed to create temporary directory");
    let workspace = temp.path().join("case");
    fs::create_dir(&workspace).expect("failed to create case directory");
    write_config_with_stages(&workspace, &["60_diff", "90_report"]);

    let output = Command::new(env!("CARGO_BIN_EXE_orev"))
        .arg("init")
        .arg(&workspace)
        .output()
        .expect("failed to initialize case");
    assert_success(&output);

    for (path, content) in [
        ("CASE.md", "# Existing analyst guide\n"),
        (".gitignore", "# Existing rules\ncustom-cache/\n"),
        ("state/index.jsonl", "{\"fixture\":\"index\"}\n"),
        (
            "state/investigation.jsonl",
            "{\"fixture\":\"investigation\"}\n",
        ),
    ] {
        fs::write(workspace.join(path), content).expect("failed to write existing content");
    }
    for stage in ["60_diff", "90_report"] {
        let path = workspace.join(stage).join("stage.yaml");
        let content = fs::read_to_string(&path).expect("failed to read stage metadata");
        fs::write(
            path,
            content
                .replace("status: pending", "status: failed")
                .replace("notes: \"\"", "notes: \"existing analyst notes\""),
        )
        .expect("failed to customize stage metadata");
        fs::write(
            workspace.join(stage).join("out/analysis.txt"),
            b"existing analyst export\n",
        )
        .expect("failed to write output fixture");
        fs::write(
            workspace.join(stage).join("log/analysis.log"),
            b"existing analysis log\n",
        )
        .expect("failed to write log fixture");
    }

    for stages in [&["60_diff", "90_report"][..], &[][..]] {
        write_config_with_stages(&workspace, stages);
        let snapshots: Vec<_> = [
            "orev.toml",
            "CASE.md",
            ".gitignore",
            "state/index.jsonl",
            "state/investigation.jsonl",
            "60_diff/stage.yaml",
            "60_diff/out/analysis.txt",
            "60_diff/log/analysis.log",
            "90_report/stage.yaml",
            "90_report/out/analysis.txt",
            "90_report/log/analysis.log",
        ]
        .into_iter()
        .map(|path| {
            (
                path,
                fs::read(workspace.join(path)).expect("failed to snapshot existing file"),
            )
        })
        .collect();

        let output = Command::new(env!("CARGO_BIN_EXE_orev"))
            .arg("init")
            .arg(&workspace)
            .output()
            .expect("failed to reinitialize case");
        assert_success(&output);

        for (path, original) in snapshots {
            assert_eq!(
                fs::read(workspace.join(path)).expect("failed to read file after reinit"),
                original,
                "reinitialization must preserve {path} with stages={stages:?}",
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
        "state/index.jsonl",
        "state/investigation.jsonl",
        "60_diff/stage.yaml",
        "90_report/stage.yaml",
    ] {
        let temp = tempfile::tempdir().expect("failed to create temporary directory");
        let workspace = temp.path().join("case");
        fs::create_dir(&workspace).expect("failed to create case directory");
        if relative != "orev.toml" {
            write_config_with_stages(&workspace, &["60_diff", "90_report"]);
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
        "60_diff",
        "60_diff/out",
        "60_diff/log",
        "90_report",
        "90_report/out",
        "90_report/log",
    ] {
        let temp = tempfile::tempdir().expect("failed to create temporary directory");
        let workspace = temp.path().join("case");
        fs::create_dir(&workspace).expect("failed to create case directory");
        write_config_with_stages(&workspace, &["60_diff", "90_report"]);

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
        "state/index.jsonl",
        "state/investigation.jsonl",
        "60_diff/stage.yaml",
        "90_report/stage.yaml",
    ] {
        let temp = tempfile::tempdir().expect("failed to create temporary directory");
        let workspace = temp.path().join("case");
        let external = temp.path().join("external-file");
        fs::create_dir(&workspace).expect("failed to create case directory");
        write_config_with_stages(&workspace, &["60_diff", "90_report"]);

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

        let output = Command::new(env!("CARGO_BIN_EXE_orev"))
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

    for relative in [
        "kb",
        "scripts",
        "state",
        ".orev",
        "store",
        "tools",
        "60_diff",
        "90_report",
    ] {
        let temp = tempfile::tempdir().expect("failed to create temporary directory");
        let workspace = temp.path().join("case");
        let external = temp.path().join("external-directory");
        fs::create_dir(&workspace).expect("failed to create case directory");
        fs::create_dir(&external).expect("failed to create external directory");
        fs::write(external.join("sentinel.txt"), b"preserve this\n")
            .expect("failed to write external sentinel");
        write_config_with_stages(&workspace, &["60_diff", "90_report"]);

        let link = workspace.join(relative);
        symlink(&external, &link).expect("failed to create directory symlink");

        let output = Command::new(env!("CARGO_BIN_EXE_orev"))
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
        "state/index.jsonl",
        "state/investigation.jsonl",
        "60_diff/stage.yaml",
        "90_report/stage.yaml",
    ] {
        for target_directory in [false, true] {
            let temp = tempfile::tempdir().expect("failed to create temporary directory");
            let workspace = temp.path().join("case");
            let external = temp.path().join("external-target");
            fs::create_dir(&workspace).expect("failed to create case directory");
            write_config_with_stages(&workspace, &["60_diff", "90_report"]);
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

    for relative in [
        "kb",
        "scripts",
        "state",
        ".orev",
        "store",
        "tools",
        "60_diff",
        "90_report",
    ] {
        for target_file in [false, true] {
            let temp = tempfile::tempdir().expect("failed to create temporary directory");
            let workspace = temp.path().join("case");
            let external = temp.path().join("external-target");
            fs::create_dir(&workspace).expect("failed to create case directory");
            write_config_with_stages(&workspace, &["60_diff", "90_report"]);
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

        let output = Command::new(env!("CARGO_BIN_EXE_orev"))
            .arg("init")
            .arg(&workspace)
            .output()
            .expect("failed to run orev init");
        assert_success(&output);
        assert!(!workspace.join(kind).exists());

        let output = Command::new(env!("CARGO_BIN_EXE_orev"))
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

        let output = Command::new(env!("CARGO_BIN_EXE_orev"))
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
