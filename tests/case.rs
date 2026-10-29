use std::fs;
use std::path::Path;
use std::process::Output;
use std::process::Stdio;

mod support;

fn run(directory: &Path, args: &[&str]) -> Output {
    support::orev()
        .current_dir(directory)
        .args(args)
        .env("PATH", "")
        .env_remove("GHIDRA_INSTALL_DIR")
        .output()
        .expect("failed to run orev")
}

fn success(output: &Output) {
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn config(config_root: &Path) -> toml::Value {
    toml::from_str(&fs::read_to_string(config_root.join("orev.toml")).unwrap()).unwrap()
}

#[test]
fn legacy_case_associations_are_rejected_without_side_effects() {
    for association in [
        "project_root = \"../..\"\n",
        "project_root = \"../..\"\nworkspace_root = \"../..\"\n",
    ] {
        let config_root = tempfile::tempdir().unwrap();
        let content = format!("{association}[case]\nname = \"original\"\nversion = \"0.1.0\"\n");
        let path = config_root.path().join("orev.toml");
        fs::write(&path, &content).unwrap();
        for (args, exit_code) in [
            (&["init"][..], 1),
            (&["doctor"][..], 1),
            (&["status"][..], 0),
            (&["case", "add", "new"][..], 1),
        ] {
            let output = run(config_root.path(), args);
            assert_eq!(output.status.code(), Some(exit_code), "{output:?}");
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(stderr.contains("unknown field `project_root`"), "{stderr}");
            assert_eq!(fs::read_to_string(&path).unwrap(), content);
            assert_eq!(fs::read_dir(config_root.path()).unwrap().count(), 1);
        }
    }
}

fn assert_cases(workspace_root: &Path, names: &[&str]) {
    let workspace_config = config(workspace_root);
    let registered: Vec<_> = workspace_config["workspace"]["cases"]
        .as_array()
        .unwrap()
        .iter()
        .map(|name| name.as_str().unwrap())
        .collect();
    assert_eq!(registered, names);
}

fn assert_workspace_case(workspace_root: &Path, name: &str) {
    let case_root = workspace_root.join("cases").join(name);
    let config = config(&case_root);
    assert_eq!(config["case"]["name"].as_str(), Some(name));
    assert_eq!(config["workspace_root"].as_str(), Some("../.."));
    for kind in ["store", "tools"] {
        let expected = fs::canonicalize(workspace_root.join(kind)).unwrap();
        assert_eq!(
            config["storage"][format!("{kind}_root")].as_str(),
            expected.to_str(),
        );
        assert!(!case_root.join(kind).exists());
    }
    assert!(!case_root.join("kb").exists());
    for file in [
        "orev.toml",
        "CASE.md",
        ".gitignore",
        "state/index.yaml",
        "state/investigation.yaml",
    ] {
        assert!(case_root.join(file).is_file(), "{file} must exist");
    }
    assert!(case_root.join("scripts").is_dir());
    let output = run(&case_root, &["doctor"]);
    success(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.contains("is missing."), "{stdout}");
    assert!(
        stdout.contains(&workspace_root.join("kb").display().to_string()),
        "{stdout}"
    );
    let output = run(&case_root, &["init"]);
    success(&output);
    assert!(!case_root.join("kb").exists());
}

#[test]
fn case_add_creates_named_workspace_cases_and_finds_ancestor_workspace() {
    let workspace_root = tempfile::tempdir().unwrap();
    success(&run(workspace_root.path(), &["init", "--workspace"]));
    assert_cases(workspace_root.path(), &[]);
    success(&run(workspace_root.path(), &["case", "add", "first"]));
    success(&run(
        &workspace_root.path().join("kb/techniques"),
        &["case", "add", "second"],
    ));
    assert_cases(workspace_root.path(), &["first", "second"]);
    assert_workspace_case(workspace_root.path(), "first");
    assert_workspace_case(workspace_root.path(), "second");
    success(&run(
        &workspace_root.path().join("cases/first/state"),
        &["case", "add", "third"],
    ));
    assert_cases(workspace_root.path(), &["first", "second", "third"]);
    success(&run(workspace_root.path(), &["init", "--workspace"]));
    assert_cases(workspace_root.path(), &["first", "second", "third"]);
}

#[test]
fn case_add_converts_standalone_case_and_moves_unknown_analysis_material() {
    let temp = tempfile::tempdir().unwrap();
    let config_root = temp.path().join("original");
    fs::create_dir(&config_root).unwrap();
    success(&run(&config_root, &["init"]));
    for directory in ["attachments/nested", "corpus", ".git", "extra"] {
        fs::create_dir_all(config_root.join(directory)).unwrap();
    }
    let fixtures = [
        ("CASE.md", b"original analyst guide\n".as_slice()),
        (
            "state/index.yaml",
            b"{\"fixture\":\"existing\"}\n".as_slice(),
        ),
        (
            "attachments/nested/report.txt",
            b"analyst attachment\n".as_slice(),
        ),
        ("notes.md", b"unknown notes\n".as_slice()),
        ("extra/attachment", b"unknown attachment\n".as_slice()),
        ("README.md", b"original case notes\n".as_slice()),
    ];
    for (path, bytes) in fixtures {
        fs::write(config_root.join(path), bytes).unwrap();
    }
    fs::write(config_root.join("store/sentinel"), b"cold bytes").unwrap();
    fs::write(config_root.join(".git/config"), b"git fixture").unwrap();
    let original_ignore = fs::read(config_root.join(".gitignore")).unwrap();
    success(&run(&config_root.join("extra"), &["case", "add", "second"]));
    assert_cases(&config_root, &["original", "second"]);
    for (path, bytes) in fixtures {
        assert_eq!(
            fs::read(config_root.join("cases/original").join(path)).unwrap(),
            bytes
        );
    }
    assert_eq!(
        fs::read(config_root.join("cases/original/.gitignore")).unwrap(),
        original_ignore
    );
    assert_eq!(
        fs::read(config_root.join("store/sentinel")).unwrap(),
        b"cold bytes"
    );
    assert_eq!(
        fs::read(config_root.join(".git/config")).unwrap(),
        b"git fixture"
    );
    assert!(config_root.join("corpus").is_dir());
    assert!(!config_root.join("CASE.md").exists());
    assert_workspace_case(&config_root, "original");
    assert_workspace_case(&config_root, "second");
}

#[test]
fn case_add_requires_name_and_does_not_offer_new() {
    let config_root = tempfile::tempdir().unwrap();
    for args in [vec!["case", "add"], vec!["case", "new", "first"]] {
        let output = run(config_root.path(), &args);
        assert_eq!(output.status.code(), Some(2));
        assert_eq!(fs::read_dir(config_root.path()).unwrap().count(), 0);
    }
}

#[test]
fn case_add_preserves_names_with_spaces_and_unicode() {
    let workspace_root = tempfile::tempdir().unwrap();
    success(&run(workspace_root.path(), &["init", "--workspace"]));
    let names = ["router audit", "case-\u{6848}\u{4ef6}"];
    for name in names {
        success(&run(workspace_root.path(), &["case", "add", name]));
        assert_workspace_case(workspace_root.path(), name);
    }
    assert_cases(workspace_root.path(), &names);
}

#[test]
fn case_add_rejects_invalid_names_without_changes() {
    let workspace_root = tempfile::tempdir().unwrap();
    success(&run(workspace_root.path(), &["init", "--workspace"]));
    let before = fs::read(workspace_root.path().join("orev.toml")).unwrap();
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
        "line\nbreak",
    ] {
        let output = run(workspace_root.path(), &["case", "add", name]);
        assert_eq!(output.status.code(), Some(1), "{output:?}");
        assert_eq!(
            fs::read(workspace_root.path().join("orev.toml")).unwrap(),
            before
        );
        assert_eq!(
            fs::read_dir(workspace_root.path().join("cases"))
                .unwrap()
                .count(),
            0
        );
        assert!(!workspace_root.path().join(".orev-case-add.lock").exists());
    }
}

#[cfg(unix)]
#[test]
fn case_add_keeps_unix_filename_rules() {
    let workspace_root = tempfile::tempdir().unwrap();
    success(&run(workspace_root.path(), &["init", "--workspace"]));
    let names = [
        "CON",
        "con.txt",
        "PRN",
        "AUX",
        "NUL",
        "COM1",
        "COM¹",
        "LPT9",
        "LPT³",
        "angle<name>",
        "quote\"name",
        "pipe|star*question?",
        "trailing.",
        "trailing ",
        " leading space",
    ];
    for name in names {
        success(&run(workspace_root.path(), &["case", "add", name]));
        assert_eq!(
            config(&workspace_root.path().join("cases").join(name))["case"]["name"].as_str(),
            Some(name)
        );
    }
    assert_cases(workspace_root.path(), &names);
}

#[test]
fn case_add_rejects_duplicates_and_existing_unregistered_directories() {
    let workspace_root = tempfile::tempdir().unwrap();
    success(&run(workspace_root.path(), &["init", "--workspace"]));
    success(&run(workspace_root.path(), &["case", "add", "first"]));
    let before = fs::read(workspace_root.path().join("orev.toml")).unwrap();
    for name in ["first", "unregistered"] {
        if name == "unregistered" {
            fs::create_dir(workspace_root.path().join("cases/unregistered")).unwrap();
            fs::write(
                workspace_root.path().join("cases/unregistered/note"),
                b"preserve",
            )
            .unwrap();
        }
        let output = run(workspace_root.path(), &["case", "add", name]);
        assert_eq!(output.status.code(), Some(1));
        assert!(!String::from_utf8_lossy(&output.stdout).contains("Success!"));
        assert_eq!(
            fs::read(workspace_root.path().join("orev.toml")).unwrap(),
            before
        );
    }
    assert_eq!(
        fs::read(workspace_root.path().join("cases/unregistered/note")).unwrap(),
        b"preserve"
    );
}

#[test]
fn case_add_stops_at_invalid_nearest_config_and_rejects_no_workspace() {
    let config_root = tempfile::tempdir().unwrap();
    assert_eq!(
        run(config_root.path(), &["case", "add", "first"])
            .status
            .code(),
        Some(1)
    );
    success(&run(config_root.path(), &["init", "--workspace"]));
    fs::create_dir(config_root.path().join("nested")).unwrap();
    fs::write(config_root.path().join("nested/orev.toml"), "invalid = [").unwrap();
    assert_eq!(
        run(
            &config_root.path().join("nested"),
            &["case", "add", "first"]
        )
        .status
        .code(),
        Some(1)
    );
    assert_cases(config_root.path(), &[]);
}

#[test]
fn conversion_preflight_rejects_original_name_or_cases_path_conflicts() {
    for conflict in ["original", "cases"] {
        let workspace_root = tempfile::tempdir().unwrap();
        let case_root = workspace_root.path().join("original");
        fs::create_dir(&case_root).unwrap();
        success(&run(&case_root, &["init"]));
        let before = fs::read(case_root.join("orev.toml")).unwrap();
        let name = if conflict == "cases" {
            fs::create_dir(case_root.join("cases")).unwrap();
            "second"
        } else {
            "original"
        };
        let output = run(&case_root, &["case", "add", name]);
        assert_eq!(output.status.code(), Some(1));
        assert_eq!(fs::read(case_root.join("orev.toml")).unwrap(), before);
        assert!(case_root.join("CASE.md").is_file());
        assert!(!case_root.join(".orev-case-add.lock").exists());
    }
}

#[test]
fn case_add_honors_workspace_lock_without_removing_another_lock() {
    let workspace_root = tempfile::tempdir().unwrap();
    success(&run(workspace_root.path(), &["init", "--workspace"]));
    fs::create_dir(workspace_root.path().join(".orev-case-add.lock")).unwrap();
    let output = run(workspace_root.path(), &["case", "add", "first"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(workspace_root.path().join(".orev-case-add.lock").is_dir());
    assert_cases(workspace_root.path(), &[]);
}

#[test]
fn case_add_uses_existing_external_storage_and_rejects_missing_roots() {
    let temp = tempfile::tempdir().unwrap();
    let workspace_root = temp.path().join("workspace");
    fs::create_dir(&workspace_root).unwrap();
    success(&run(&workspace_root, &["init", "--workspace"]));
    let external = temp.path().join("external");
    fs::create_dir(&external).unwrap();
    fs::write(external.join("sentinel"), b"preserve").unwrap();
    let mut workspace_config = config(&workspace_root);
    workspace_config["storage"] = toml::Value::Table(toml::Table::from_iter([
        (
            "store_root".into(),
            toml::Value::String(external.to_str().unwrap().into()),
        ),
        (
            "tools_root".into(),
            toml::Value::String(external.to_str().unwrap().into()),
        ),
    ]));
    fs::write(
        workspace_root.join("orev.toml"),
        toml::to_string_pretty(&workspace_config).unwrap(),
    )
    .unwrap();
    success(&run(&workspace_root, &["case", "add", "external-case"]));
    let case_root = workspace_root.join("cases/external-case");
    assert_eq!(
        config(&case_root)["storage"]["store_root"].as_str(),
        fs::canonicalize(&external).unwrap().to_str()
    );
    success(&run(&case_root, &["doctor"]));
    assert_eq!(fs::read(external.join("sentinel")).unwrap(), b"preserve");

    let missing = temp.path().join("missing");
    workspace_config = config(&workspace_root);
    workspace_config["storage"]["tools_root"] =
        toml::Value::String(missing.to_str().unwrap().into());
    let before = toml::to_string_pretty(&workspace_config).unwrap();
    fs::write(workspace_root.join("orev.toml"), &before).unwrap();
    let output = run(&workspace_root, &["case", "add", "bad"]);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        fs::read_to_string(workspace_root.join("orev.toml")).unwrap(),
        before
    );
    assert!(!workspace_root.join("cases/bad").exists());
    assert!(!missing.exists());
}

#[test]
fn concurrent_case_add_never_loses_registry_entries() {
    let workspace_root = tempfile::tempdir().unwrap();
    success(&run(workspace_root.path(), &["init", "--workspace"]));
    let children: Vec<_> = ["one", "two", "three", "four"]
        .into_iter()
        .map(|name| {
            let child = support::orev()
                .current_dir(workspace_root.path())
                .args(["case", "add", name])
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap();
            (name, child)
        })
        .collect();
    let outputs: Vec<_> = children
        .into_iter()
        .map(|(name, child)| (name, child.wait_with_output().unwrap()))
        .collect();
    let workspace_config = config(workspace_root.path());
    let names = workspace_config["workspace"]["cases"].as_array().unwrap();
    assert!(!names.is_empty());
    for (name, output) in outputs {
        if output.status.success() {
            assert!(names.iter().any(|entry| entry.as_str() == Some(name)));
            assert!(
                workspace_root
                    .path()
                    .join("cases")
                    .join(name)
                    .join("orev.toml")
                    .is_file()
            );
        } else {
            assert_eq!(output.status.code(), Some(1));
            assert!(String::from_utf8_lossy(&output.stderr).contains("workspace lock"));
            assert!(!workspace_root.path().join("cases").join(name).exists());
        }
    }
    assert_eq!(
        fs::read_dir(workspace_root.path().join("cases"))
            .unwrap()
            .count(),
        names.len()
    );
}

#[cfg(unix)]
#[test]
fn case_add_does_not_follow_symlink_destinations_or_relocate_symlinks() {
    use std::os::unix::fs::symlink;
    let temp = tempfile::tempdir().unwrap();
    let workspace_root = temp.path().join("workspace");
    fs::create_dir(&workspace_root).unwrap();
    success(&run(&workspace_root, &["init", "--workspace"]));
    symlink(
        temp.path().join("missing"),
        workspace_root.join("cases/linked"),
    )
    .unwrap();
    assert_eq!(
        run(&workspace_root, &["case", "add", "linked"])
            .status
            .code(),
        Some(1)
    );
    assert_cases(&workspace_root, &[]);
    assert!(
        fs::symlink_metadata(workspace_root.join("cases/linked"))
            .unwrap()
            .file_type()
            .is_symlink()
    );

    let standalone = temp.path().join("original");
    fs::create_dir(&standalone).unwrap();
    success(&run(&standalone, &["init"]));
    symlink("store", standalone.join("analyst-link")).unwrap();
    let original_config = fs::read(standalone.join("orev.toml")).unwrap();
    let output = run(&standalone, &["case", "add", "second"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr).contains("symlink"));
    assert_eq!(
        fs::read(standalone.join("orev.toml")).unwrap(),
        original_config
    );
    assert!(!standalone.join("cases").exists());
}

#[cfg(unix)]
fn directory_link(target: &Path, link: &Path) {
    std::os::unix::fs::symlink(target, link).unwrap();
}

#[test]
fn shared_directory_link_is_allowed_only_when_target_stays_in_place() {
    for (name, relocated) in ["kb", "corpus", ".git"]
        .into_iter()
        .flat_map(|name| [false, true].map(|relocated| (name, relocated)))
    {
        let temp = tempfile::tempdir().unwrap();
        let config_root = temp.path().join("original");
        fs::create_dir(&config_root).unwrap();
        success(&run(&config_root, &["init"]));
        let target = if relocated {
            config_root.join("notes")
        } else {
            temp.path().join("external")
        };
        if name != "kb" {
            fs::create_dir(config_root.join(name)).unwrap();
        }
        fs::rename(config_root.join(name), &target).unwrap();
        fs::write(target.join("evidence"), b"preserve").unwrap();
        directory_link(&target, &config_root.join(name));
        let original = fs::read(config_root.join("orev.toml")).unwrap();
        let output = run(&config_root, &["case", "add", "second"]);
        if relocated {
            assert!(!output.status.success());
            assert!(
                String::from_utf8_lossy(&output.stderr)
                    .contains("points into data being relocated")
            );
            assert_eq!(fs::read(config_root.join("orev.toml")).unwrap(), original);
            assert!(!config_root.join("cases").exists());
        } else {
            success(&output);
            assert_cases(&config_root, &["original", "second"]);
        }
        assert_eq!(fs::read(target.join("evidence")).unwrap(), b"preserve");
        assert_eq!(
            fs::read(config_root.join(name).join("evidence")).unwrap(),
            b"preserve"
        );
    }
}
