use std::fs;
use std::path::Path;
use std::process::Output;
use std::process::Stdio;

mod support;

fn run(root: &Path, args: &[&str]) -> Output {
    support::orev()
        .current_dir(root)
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

fn config(root: &Path) -> toml::Value {
    toml::from_str(&fs::read_to_string(root.join("orev.toml")).unwrap()).unwrap()
}

fn assert_cases(root: &Path, names: &[&str]) {
    let project = config(root);
    let registered: Vec<_> = project["workspace"]["cases"]
        .as_array()
        .unwrap()
        .iter()
        .map(|name| name.as_str().unwrap())
        .collect();
    assert_eq!(registered, names);
}

fn assert_shared_case(root: &Path, name: &str) {
    let case = root.join("cases").join(name);
    let config = config(&case);
    assert_eq!(config["case"]["name"].as_str(), Some(name));
    assert_eq!(config["project_root"].as_str(), Some("../.."));
    for kind in ["store", "tools"] {
        let expected = fs::canonicalize(root.join(kind)).unwrap();
        assert_eq!(
            config["storage"][format!("{kind}_root")].as_str(),
            expected.to_str(),
        );
        assert!(!case.join(kind).exists());
    }
    assert!(!case.join("kb").exists());
    for file in [
        "orev.toml",
        "CASE.md",
        ".gitignore",
        "state/index.yaml",
        "state/investigation.yaml",
    ] {
        assert!(case.join(file).is_file(), "{file} must exist");
    }
    assert!(case.join("scripts").is_dir());
    let output = run(&case, &["doctor"]);
    success(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.contains("is missing."), "{stdout}");
    assert!(
        stdout.contains(&root.join("kb").display().to_string()),
        "{stdout}"
    );
    let output = run(&case, &["init"]);
    success(&output);
    assert!(!case.join("kb").exists());
}

#[test]
fn case_add_creates_named_shared_cases_and_finds_ancestor_project() {
    let root = tempfile::tempdir().unwrap();
    success(&run(root.path(), &["init", "--workspace"]));
    assert_cases(root.path(), &[]);
    success(&run(root.path(), &["case", "add", "first"]));
    success(&run(
        &root.path().join("kb/techniques"),
        &["case", "add", "second"],
    ));
    assert_cases(root.path(), &["first", "second"]);
    assert_shared_case(root.path(), "first");
    assert_shared_case(root.path(), "second");
    success(&run(
        &root.path().join("cases/first/state"),
        &["case", "add", "third"],
    ));
    assert_cases(root.path(), &["first", "second", "third"]);
    success(&run(root.path(), &["init", "--workspace"]));
    assert_cases(root.path(), &["first", "second", "third"]);
}

#[test]
fn case_add_converts_single_case_and_moves_unknown_analysis_material() {
    let root = tempfile::tempdir().unwrap();
    let standalone = root.path().join("original");
    fs::create_dir(&standalone).unwrap();
    success(&run(&standalone, &["init"]));
    for directory in ["attachments/nested", "corpus", ".git", "extra"] {
        fs::create_dir_all(standalone.join(directory)).unwrap();
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
        fs::write(standalone.join(path), bytes).unwrap();
    }
    fs::write(standalone.join("store/sentinel"), b"cold bytes").unwrap();
    fs::write(standalone.join(".git/config"), b"git fixture").unwrap();
    let original_ignore = fs::read(standalone.join(".gitignore")).unwrap();
    success(&run(&standalone.join("extra"), &["case", "add", "second"]));
    assert_cases(&standalone, &["original", "second"]);
    for (path, bytes) in fixtures {
        assert_eq!(
            fs::read(standalone.join("cases/original").join(path)).unwrap(),
            bytes
        );
    }
    assert_eq!(
        fs::read(standalone.join("cases/original/.gitignore")).unwrap(),
        original_ignore
    );
    assert_eq!(
        fs::read(standalone.join("store/sentinel")).unwrap(),
        b"cold bytes"
    );
    assert_eq!(
        fs::read(standalone.join(".git/config")).unwrap(),
        b"git fixture"
    );
    assert!(standalone.join("corpus").is_dir());
    assert!(!standalone.join("CASE.md").exists());
    assert_shared_case(&standalone, "original");
    assert_shared_case(&standalone, "second");
}

#[test]
fn case_add_requires_name_and_does_not_offer_new() {
    let root = tempfile::tempdir().unwrap();
    for args in [vec!["case", "add"], vec!["case", "new", "first"]] {
        let output = run(root.path(), &args);
        assert_eq!(output.status.code(), Some(2));
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 0);
    }
}

#[test]
fn case_add_preserves_names_with_spaces_and_unicode() {
    let root = tempfile::tempdir().unwrap();
    success(&run(root.path(), &["init", "--workspace"]));
    let names = ["router audit", "case-\u{6848}\u{4ef6}"];
    for name in names {
        success(&run(root.path(), &["case", "add", name]));
        assert_shared_case(root.path(), name);
    }
    assert_cases(root.path(), &names);
}

#[test]
fn case_add_rejects_invalid_names_without_changes() {
    let root = tempfile::tempdir().unwrap();
    success(&run(root.path(), &["init", "--workspace"]));
    let before = fs::read(root.path().join("orev.toml")).unwrap();
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
        let output = run(root.path(), &["case", "add", name]);
        assert_eq!(output.status.code(), Some(1), "{output:?}");
        assert_eq!(fs::read(root.path().join("orev.toml")).unwrap(), before);
        assert_eq!(fs::read_dir(root.path().join("cases")).unwrap().count(), 0);
        assert!(!root.path().join(".orev-case-add.lock").exists());
    }
}

#[cfg(unix)]
#[test]
fn case_add_keeps_unix_filename_rules() {
    let root = tempfile::tempdir().unwrap();
    success(&run(root.path(), &["init", "--workspace"]));
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
        success(&run(root.path(), &["case", "add", name]));
        assert_eq!(
            config(&root.path().join("cases").join(name))["case"]["name"].as_str(),
            Some(name)
        );
    }
    assert_cases(root.path(), &names);
}

#[test]
fn case_add_rejects_duplicates_and_existing_unregistered_directories() {
    let root = tempfile::tempdir().unwrap();
    success(&run(root.path(), &["init", "--workspace"]));
    success(&run(root.path(), &["case", "add", "first"]));
    let before = fs::read(root.path().join("orev.toml")).unwrap();
    for name in ["first", "unregistered"] {
        if name == "unregistered" {
            fs::create_dir(root.path().join("cases/unregistered")).unwrap();
            fs::write(root.path().join("cases/unregistered/note"), b"preserve").unwrap();
        }
        let output = run(root.path(), &["case", "add", name]);
        assert_eq!(output.status.code(), Some(1));
        assert!(!String::from_utf8_lossy(&output.stdout).contains("Success!"));
        assert_eq!(fs::read(root.path().join("orev.toml")).unwrap(), before);
    }
    assert_eq!(
        fs::read(root.path().join("cases/unregistered/note")).unwrap(),
        b"preserve"
    );
}

#[test]
fn case_add_stops_at_invalid_nearest_config_and_rejects_no_project() {
    let root = tempfile::tempdir().unwrap();
    assert_eq!(
        run(root.path(), &["case", "add", "first"]).status.code(),
        Some(1)
    );
    success(&run(root.path(), &["init", "--workspace"]));
    fs::create_dir(root.path().join("nested")).unwrap();
    fs::write(root.path().join("nested/orev.toml"), "invalid = [").unwrap();
    assert_eq!(
        run(&root.path().join("nested"), &["case", "add", "first"])
            .status
            .code(),
        Some(1)
    );
    assert_cases(root.path(), &[]);
}

#[test]
fn conversion_preflight_rejects_original_name_or_cases_path_conflicts() {
    for conflict in ["original", "cases"] {
        let root = tempfile::tempdir().unwrap();
        let case = root.path().join("original");
        fs::create_dir(&case).unwrap();
        success(&run(&case, &["init"]));
        let before = fs::read(case.join("orev.toml")).unwrap();
        let name = if conflict == "cases" {
            fs::create_dir(case.join("cases")).unwrap();
            "second"
        } else {
            "original"
        };
        let output = run(&case, &["case", "add", name]);
        assert_eq!(output.status.code(), Some(1));
        assert_eq!(fs::read(case.join("orev.toml")).unwrap(), before);
        assert!(case.join("CASE.md").is_file());
        assert!(!case.join(".orev-case-add.lock").exists());
    }
}

#[test]
fn case_add_honors_project_lock_without_removing_another_lock() {
    let root = tempfile::tempdir().unwrap();
    success(&run(root.path(), &["init", "--workspace"]));
    fs::create_dir(root.path().join(".orev-case-add.lock")).unwrap();
    let output = run(root.path(), &["case", "add", "first"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(root.path().join(".orev-case-add.lock").is_dir());
    assert_cases(root.path(), &[]);
}

#[test]
fn case_add_uses_existing_external_storage_and_rejects_missing_roots() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("project");
    fs::create_dir(&root).unwrap();
    success(&run(&root, &["init", "--workspace"]));
    let external = temp.path().join("external");
    fs::create_dir(&external).unwrap();
    fs::write(external.join("sentinel"), b"preserve").unwrap();
    let mut project = config(&root);
    project["storage"] = toml::Value::Table(toml::Table::from_iter([
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
        root.join("orev.toml"),
        toml::to_string_pretty(&project).unwrap(),
    )
    .unwrap();
    success(&run(&root, &["case", "add", "external-case"]));
    let case = root.join("cases/external-case");
    assert_eq!(
        config(&case)["storage"]["store_root"].as_str(),
        fs::canonicalize(&external).unwrap().to_str()
    );
    success(&run(&case, &["doctor"]));
    assert_eq!(fs::read(external.join("sentinel")).unwrap(), b"preserve");

    let missing = temp.path().join("missing");
    project = config(&root);
    project["storage"]["tools_root"] = toml::Value::String(missing.to_str().unwrap().into());
    let before = toml::to_string_pretty(&project).unwrap();
    fs::write(root.join("orev.toml"), &before).unwrap();
    let output = run(&root, &["case", "add", "bad"]);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(fs::read_to_string(root.join("orev.toml")).unwrap(), before);
    assert!(!root.join("cases/bad").exists());
    assert!(!missing.exists());
}

#[test]
fn concurrent_case_add_never_loses_registry_entries() {
    let root = tempfile::tempdir().unwrap();
    success(&run(root.path(), &["init", "--workspace"]));
    let children: Vec<_> = ["one", "two", "three", "four"]
        .into_iter()
        .map(|name| {
            let child = support::orev()
                .current_dir(root.path())
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
    let project = config(root.path());
    let names = project["workspace"]["cases"].as_array().unwrap();
    assert!(!names.is_empty());
    for (name, output) in outputs {
        if output.status.success() {
            assert!(names.iter().any(|entry| entry.as_str() == Some(name)));
            assert!(
                root.path()
                    .join("cases")
                    .join(name)
                    .join("orev.toml")
                    .is_file()
            );
        } else {
            assert_eq!(output.status.code(), Some(1));
            assert!(String::from_utf8_lossy(&output.stderr).contains("project lock"));
            assert!(!root.path().join("cases").join(name).exists());
        }
    }
    assert_eq!(
        fs::read_dir(root.path().join("cases")).unwrap().count(),
        names.len()
    );
}

#[cfg(unix)]
#[test]
fn case_add_does_not_follow_symlink_destinations_or_relocate_symlinks() {
    use std::os::unix::fs::symlink;
    let temp = tempfile::tempdir().unwrap();
    let project = temp.path().join("project");
    fs::create_dir(&project).unwrap();
    success(&run(&project, &["init", "--workspace"]));
    symlink(temp.path().join("missing"), project.join("cases/linked")).unwrap();
    assert_eq!(
        run(&project, &["case", "add", "linked"]).status.code(),
        Some(1)
    );
    assert_cases(&project, &[]);
    assert!(
        fs::symlink_metadata(project.join("cases/linked"))
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
        let root = temp.path().join("original");
        fs::create_dir(&root).unwrap();
        success(&run(&root, &["init"]));
        let target = if relocated {
            root.join("notes")
        } else {
            temp.path().join("external")
        };
        if name != "kb" {
            fs::create_dir(root.join(name)).unwrap();
        }
        fs::rename(root.join(name), &target).unwrap();
        fs::write(target.join("evidence"), b"preserve").unwrap();
        directory_link(&target, &root.join(name));
        let original = fs::read(root.join("orev.toml")).unwrap();
        let output = run(&root, &["case", "add", "second"]);
        if relocated {
            assert!(!output.status.success());
            assert!(
                String::from_utf8_lossy(&output.stderr)
                    .contains("points into data being relocated")
            );
            assert_eq!(fs::read(root.join("orev.toml")).unwrap(), original);
            assert!(!root.join("cases").exists());
        } else {
            success(&output);
            assert_cases(&root, &["original", "second"]);
        }
        assert_eq!(fs::read(target.join("evidence")).unwrap(), b"preserve");
        assert_eq!(
            fs::read(root.join(name).join("evidence")).unwrap(),
            b"preserve"
        );
    }
}
