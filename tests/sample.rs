use serde_yaml_ng::Value;
use sha2::{Digest, Sha256};
use std::ffi::OsStr;
use std::fs;
use std::os::unix::ffi::OsStringExt;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::{Path, PathBuf};
use std::process::Output;

mod support;

const ABC: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

fn scratch() -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix("sample-test-")
        .tempdir_in(Path::new(env!("CARGO_MANIFEST_DIR")).join("target"))
        .unwrap()
}

fn run(directory: &Path, args: &[&OsStr]) -> Output {
    support::orev()
        .current_dir(directory)
        .args(args)
        .env("PATH", "")
        .output()
        .unwrap()
}

fn success(output: &Output) {
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn failure(output: &Output) {
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("Error: Failed to ingest samples:"),
        "{output:?}"
    );
}

fn case(root: &Path) -> PathBuf {
    let path = root.join("case");
    fs::create_dir(&path).unwrap();
    fs::write(
        path.join("orev.toml"),
        "[case]\nname = 'test'\nversion = '0.1.0'\n",
    )
    .unwrap();
    path
}

fn ingest(directory: &Path, paths: &[&Path]) -> Output {
    let mut args = vec![OsStr::new("sample"), OsStr::new("ingest"), OsStr::new("--")];
    args.extend(paths.iter().map(|path| path.as_os_str()));
    run(directory, &args)
}

fn ingest_case(directory: &Path, name: &str, paths: &[&Path]) -> Output {
    let mut args = vec![
        OsStr::new("sample"),
        OsStr::new("ingest"),
        OsStr::new("--case"),
        OsStr::new(name),
        OsStr::new("--"),
    ];
    args.extend(paths.iter().map(|path| path.as_os_str()));
    run(directory, &args)
}

fn workspace(root: &Path, names: &[&str]) {
    success(&run(root, &[OsStr::new("init"), OsStr::new("--workspace")]));
    for name in names {
        success(&run(
            root,
            &[OsStr::new("case"), OsStr::new("add"), OsStr::new(name)],
        ));
    }
}

fn manifest(intake: &Path) -> Vec<Value> {
    serde_yaml_ng::from_slice(&fs::read(intake.join("stage.yaml")).unwrap()).unwrap()
}

fn assert_no_scratch(intake: &Path) {
    for entry in fs::read_dir(intake).unwrap() {
        assert!(
            !entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with(".orev-ingest-"),
            "staging data remains"
        );
    }
}

fn assert_intake_paths(value: &Value, intake: &Path) {
    match value {
        Value::Mapping(mapping) => {
            for (key, value) in mapping {
                if key.as_str().is_some_and(|key| key.ends_with("path")) {
                    let path = value.as_str().unwrap();
                    assert!(!Path::new(path).is_absolute());
                    assert!(path.split('/').all(|part| !matches!(part, "" | "." | "..")));
                    assert!(intake.join(path).exists(), "not intake-relative: {path}");
                }
                assert_intake_paths(value, intake);
            }
        }
        Value::Sequence(values) => {
            for value in values {
                assert_intake_paths(value, intake);
            }
        }
        _ => {}
    }
}

#[test]
fn mixed_inputs_copy_bytes_and_record_only_intake_relative_paths() {
    let scratch = scratch();
    let case = case(scratch.path());
    let originals = scratch.path().join("private-source-parent");
    fs::create_dir(&originals).unwrap();
    let file = originals.join("firmware abc.bin");
    fs::write(&file, b"abc").unwrap();
    fs::set_permissions(&file, fs::Permissions::from_mode(0o755)).unwrap();
    let tree = originals.join("tree");
    fs::create_dir_all(tree.join("nested/empty")).unwrap();
    fs::write(tree.join(".hidden"), b"secret").unwrap();
    fs::write(tree.join("nested/file"), [0, 255, 10]).unwrap();
    let empty = originals.join("empty");
    fs::create_dir(&empty).unwrap();
    success(&ingest(&case, &[&file, &tree, &empty]));
    let intake = case.join("00_intake");
    assert_eq!(fs::read(intake.join("firmware abc.bin")).unwrap(), b"abc");
    assert_eq!(fs::read(intake.join("tree/.hidden")).unwrap(), b"secret");
    assert_eq!(
        fs::read(intake.join("tree/nested/file")).unwrap(),
        [0, 255, 10]
    );
    assert!(intake.join("tree/nested/empty").is_dir());
    assert!(intake.join("empty").is_dir());
    assert_eq!(fs::read(&file).unwrap(), b"abc");
    assert_eq!(fs::read(tree.join(".hidden")).unwrap(), b"secret");
    assert_eq!(fs::read(tree.join("nested/file")).unwrap(), [0, 255, 10]);
    assert!(tree.join("nested/empty").is_dir());
    assert_eq!(
        fs::metadata(&file).unwrap().permissions().mode() & 0o777,
        0o755
    );
    for path in ["", "tree", "tree/nested", "tree/nested/empty", "empty"] {
        assert_eq!(
            fs::metadata(intake.join(path))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
    }
    for path in [
        "firmware abc.bin",
        "tree/.hidden",
        "tree/nested/file",
        "stage.yaml",
    ] {
        assert_eq!(
            fs::metadata(intake.join(path))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }
    let records = manifest(&intake);
    assert_eq!(records.len(), 3);
    assert_eq!(records[0]["kind"], "file");
    assert_eq!(records[0]["stored_path"], "firmware abc.bin");
    assert_eq!(records[0]["sha256"], ABC);
    assert_eq!(records[0]["byte_size"], 3);
    assert_eq!(records[1]["kind"], "directory");
    assert_eq!(records[1]["byte_size"], 9);
    assert_eq!(
        records[2]["sha256"],
        "aa6bb224f441e6de667f18b30de3e6d04f3724dc49def8ff903c522b3bc64a91"
    );
    let paths: Vec<_> = records[1]["entries"]
        .as_sequence()
        .unwrap()
        .iter()
        .map(|entry| entry["path"].as_str().unwrap())
        .collect();
    assert_eq!(
        paths,
        [
            "tree/.hidden",
            "tree/nested",
            "tree/nested/empty",
            "tree/nested/file"
        ]
    );
    for record in &records {
        let time =
            chrono::DateTime::parse_from_rfc3339(record["ingested_at"].as_str().unwrap()).unwrap();
        assert_eq!(time.offset().local_minus_utc(), 0);
        assert!(record["source_modified_at"].is_string());
        assert!(record.get("source_path").is_none());
        assert_intake_paths(record, &intake);
    }
    let text = fs::read_to_string(intake.join("stage.yaml")).unwrap();
    assert!(!text.contains(originals.to_str().unwrap()));
    assert!(!text.contains("private-source-parent"));
    assert!(!text.contains(scratch.path().to_str().unwrap()));
    assert_no_scratch(&intake);
}

#[test]
fn directory_digest_has_independent_canonical_vector_and_ignores_root_location() {
    let scratch = scratch();
    let case = case(scratch.path());
    let one = scratch.path().join("first");
    let two = scratch.path().join("second");
    fs::create_dir_all(one.join("z-empty")).unwrap();
    fs::write(one.join("a-file"), b"abc").unwrap();
    fs::create_dir(&two).unwrap();
    fs::write(two.join("a-file"), b"abc").unwrap();
    fs::create_dir(two.join("z-empty")).unwrap();
    fs::set_permissions(two.join("a-file"), fs::Permissions::from_mode(0o700)).unwrap();
    success(&ingest(&case, &[&two, &one]));
    let mut canonical = b"orev-directory-sha256-v1\0F".to_vec();
    canonical.extend_from_slice(&6u64.to_be_bytes());
    canonical.extend_from_slice(b"a-file");
    canonical.extend_from_slice(&3u64.to_be_bytes());
    canonical.extend_from_slice(&[
        0xba, 0x78, 0x16, 0xbf, 0x8f, 0x01, 0xcf, 0xea, 0x41, 0x41, 0x40, 0xde, 0x5d, 0xae, 0x22,
        0x23, 0xb0, 0x03, 0x61, 0xa3, 0x96, 0x17, 0x7a, 0x9c, 0xb4, 0x10, 0xff, 0x61, 0xf2, 0x00,
        0x15, 0xad,
    ]);
    canonical.extend_from_slice(b"D");
    canonical.extend_from_slice(&7u64.to_be_bytes());
    canonical.extend_from_slice(b"z-empty");
    let expected = format!("{:x}", Sha256::digest(&canonical));
    let records = manifest(&case.join("00_intake"));
    assert_eq!(records[0]["sha256"], expected);
    assert_eq!(records[1]["sha256"], expected);
    let third = scratch.path().join("third");
    fs::create_dir(&third).unwrap();
    fs::write(third.join("a-file"), b"abc").unwrap();
    success(&ingest(&case, &[&third]));
    assert_ne!(manifest(&case.join("00_intake"))[2]["sha256"], expected);
}

#[test]
fn repeated_ingest_appends_without_changing_previous_records_or_artifacts() {
    let scratch = scratch();
    let case = case(scratch.path());
    let first = scratch.path().join("first");
    let second = scratch.path().join("second");
    fs::write(&first, b"abc").unwrap();
    fs::write(&second, b"other").unwrap();
    success(&ingest(&case, &[&first]));
    let intake = case.join("00_intake");
    let old = manifest(&intake);
    fs::remove_file(first).unwrap();
    success(&ingest(&case, &[&second]));
    let records = manifest(&intake);
    assert_eq!(records.len(), 2);
    assert_eq!(records[0], old[0]);
    assert_eq!(fs::read(intake.join("first")).unwrap(), b"abc");
    assert_eq!(fs::read(intake.join("second")).unwrap(), b"other");
    assert_no_scratch(&intake);
}

#[test]
fn nearest_workspace_case_and_nested_cwd_use_case_intake() {
    let scratch = scratch();
    let workspace = scratch.path();
    success(&run(
        workspace,
        &[OsStr::new("init"), OsStr::new("--workspace")],
    ));
    success(&run(
        workspace,
        &[
            OsStr::new("case"),
            OsStr::new("add"),
            OsStr::new("firmware"),
        ],
    ));
    let root = workspace.join("cases/firmware");
    fs::create_dir_all(root.join("deep/cwd")).unwrap();
    let source = workspace.join("sample");
    fs::write(&source, b"abc").unwrap();
    success(&ingest(
        &root.join("deep/cwd"),
        &[Path::new("../../../../sample")],
    ));
    assert!(root.join("00_intake/sample").is_file());
    assert!(!workspace.join("00_intake").exists());
    assert!(!root.join("deep/cwd/00_intake").exists());
}

#[test]
fn named_case_from_workspace_and_subdirectory_keeps_sources_cwd_relative() {
    let scratch = scratch();
    let root = scratch.path();
    workspace(root, &["first", "target case"]);
    fs::write(root.join("one.bin"), b"abc").unwrap();
    fs::create_dir_all(root.join("tree/empty")).unwrap();
    success(&ingest_case(
        root,
        "target case",
        &[Path::new("one.bin"), Path::new("tree")],
    ));
    fs::create_dir(root.join("nested")).unwrap();
    fs::write(root.join("two.bin"), b"second").unwrap();
    success(&ingest_case(
        &root.join("nested"),
        "first",
        &[Path::new("../two.bin")],
    ));
    assert_eq!(
        fs::read(root.join("cases/first/00_intake/two.bin")).unwrap(),
        b"second"
    );
    let intake = root.join("cases/target case/00_intake");
    assert_eq!(fs::read(intake.join("one.bin")).unwrap(), b"abc");
    assert!(intake.join("tree/empty").is_dir());
    for record in manifest(&intake) {
        assert_intake_paths(&record, &intake);
    }
    let text = fs::read_to_string(intake.join("stage.yaml")).unwrap();
    assert!(!text.contains(root.to_str().unwrap()));
    assert!(!root.join("00_intake").exists());
    assert!(!root.join("nested/00_intake").exists());
}

#[test]
fn named_case_from_an_associated_case_can_select_a_sibling() {
    let scratch = scratch();
    let root = scratch.path();
    workspace(root, &["first", "second"]);
    fs::write(root.join("one.bin"), b"abc").unwrap();
    fs::write(root.join("two.bin"), b"other").unwrap();
    let cwd = root.join("cases/first/scripts");
    success(&ingest_case(
        &cwd,
        "second",
        &[Path::new("../../../one.bin")],
    ));
    assert!(!root.join("cases/first/00_intake").exists());
    success(&ingest_case(
        &cwd,
        "first",
        &[Path::new("../../../two.bin")],
    ));
    assert_eq!(
        fs::read(root.join("cases/second/00_intake/one.bin")).unwrap(),
        b"abc"
    );
    assert_eq!(
        fs::read(root.join("cases/first/00_intake/two.bin")).unwrap(),
        b"other"
    );
}

#[test]
fn standalone_named_selection_requires_the_configured_name() {
    let scratch = scratch();
    let case = case(scratch.path());
    let source = scratch.path().join("input");
    fs::write(&source, b"abc").unwrap();
    let output = ingest_case(&case, "other", &[&source]);
    failure(&output);
    assert!(String::from_utf8_lossy(&output.stderr).contains("standalone case"));
    assert!(!case.join("00_intake").exists());
    let nested = case.join("nested");
    fs::create_dir(&nested).unwrap();
    success(&ingest_case(&nested, "test", &[&source]));
    assert_eq!(fs::read(case.join("00_intake/input")).unwrap(), b"abc");
    assert!(!case.join("cases").exists());
}

#[test]
fn named_selection_rejects_unknown_unregistered_and_invalid_names_without_writes() {
    let scratch = scratch();
    let root = scratch.path();
    workspace(root, &["target"]);
    fs::write(root.join("input"), b"abc").unwrap();
    fs::create_dir(root.join("cases/unregistered")).unwrap();
    for name in [
        "missing",
        "unregistered",
        "",
        " ",
        ".",
        "..",
        "../target",
        "/target",
        "a\\b",
        "a:b",
        "a\nb",
    ] {
        failure(&ingest_case(root, name, &[Path::new("input")]));
        assert!(!root.join("00_intake").exists());
        assert!(!root.join("cases/target/00_intake").exists());
        assert!(!root.join("cases/unregistered/00_intake").exists());
    }
    assert_eq!(fs::read(root.join("input")).unwrap(), b"abc");
}

#[test]
fn named_selection_validates_target_configuration_and_workspace_association() {
    let scratch = scratch();
    let root = scratch.path();
    workspace(root, &["target"]);
    fs::create_dir(root.join("foreign")).unwrap();
    workspace(&root.join("foreign"), &[]);
    let source = root.join("input");
    fs::write(&source, b"abc").unwrap();
    let target = root.join("cases/target");
    let config = target.join("orev.toml");
    let valid = fs::read(&config).unwrap();
    for text in [
        "not toml",
        "[workspace]\ncases=[]\n",
        "[case]\nname='target'\nversion='0.1.0'\n",
        "workspace_root='../..'\n[case]\nname='other'\nversion='0.1.0'\n",
        "workspace_root='../../foreign'\n[case]\nname='target'\nversion='0.1.0'\n",
        "workspace_root='../..'\n[case]\nname='target'\nversion='0.1.0'\nbogus=true\n",
        "workspace_root='../..'\n[case]\nname='target'\nversion='0.1.0'\n[storage]\nstore_root='missing'\n",
    ] {
        fs::write(&config, text).unwrap();
        failure(&ingest_case(root, "target", &[&source]));
        assert!(!target.join("00_intake").exists());
    }
    fs::remove_file(&config).unwrap();
    let linked = root.join("linked-config");
    fs::write(&linked, &valid).unwrap();
    symlink(&linked, &config).unwrap();
    failure(&ingest_case(root, "target", &[&source]));
    assert!(!target.join("00_intake").exists());
    fs::remove_file(&config).unwrap();
    fs::write(&config, valid).unwrap();
    fs::create_dir(root.join("nested")).unwrap();
    fs::write(root.join("nested/orev.toml"), "invalid").unwrap();
    failure(&ingest_case(&root.join("nested"), "target", &[&source]));
    assert!(!target.join("00_intake").exists());
}

#[test]
fn named_selection_rejects_missing_or_symlinked_case_directories() {
    let scratch = scratch();
    let root = scratch.path();
    workspace(root, &["target"]);
    let source = root.join("input");
    fs::write(&source, b"abc").unwrap();
    let target = root.join("cases/target");
    let moved = root.join("moved-target");
    fs::rename(&target, &moved).unwrap();
    failure(&ingest_case(root, "target", &[&source]));
    symlink(&moved, &target).unwrap();
    failure(&ingest_case(root, "target", &[&source]));
    assert!(!moved.join("00_intake").exists());
    fs::remove_file(&target).unwrap();
    fs::rename(&moved, &target).unwrap();
    fs::rename(root.join("cases"), root.join("moved-cases")).unwrap();
    symlink(root.join("moved-cases"), root.join("cases")).unwrap();
    failure(&ingest_case(root, "target", &[&source]));
    assert!(!root.join("moved-cases/target/00_intake").exists());
}

#[test]
fn named_selection_requires_a_value_and_accepts_the_option_after_paths() {
    let scratch = scratch();
    let root = scratch.path();
    workspace(root, &["target"]);
    fs::write(root.join("input"), b"abc").unwrap();
    let output = run(
        root,
        &[
            OsStr::new("sample"),
            OsStr::new("ingest"),
            OsStr::new("input"),
            OsStr::new("--case"),
        ],
    );
    assert_eq!(output.status.code(), Some(2));
    assert!(!root.join("cases/target/00_intake").exists());
    success(&run(
        root,
        &[
            OsStr::new("sample"),
            OsStr::new("ingest"),
            OsStr::new("input"),
            OsStr::new("--case"),
            OsStr::new("target"),
        ],
    ));
    assert_eq!(
        fs::read(root.join("cases/target/00_intake/input")).unwrap(),
        b"abc"
    );
}

#[test]
fn dot_and_parent_inputs_use_resolved_directory_names() {
    let scratch = scratch();
    let case = case(scratch.path());
    let source = case.join("sources/parent/nested");
    fs::create_dir_all(&source).unwrap();
    fs::write(source.join("sample.bin"), b"abc").unwrap();
    success(&ingest(&source, &[Path::new("."), Path::new("..")]));
    let intake = case.join("00_intake");
    assert_eq!(fs::read(intake.join("nested/sample.bin")).unwrap(), b"abc");
    assert_eq!(
        fs::read(intake.join("parent/nested/sample.bin")).unwrap(),
        b"abc"
    );
    let records = manifest(&intake);
    assert_eq!(records[0]["stored_path"], "nested");
    assert_eq!(records[1]["stored_path"], "parent");
    for record in &records {
        assert_intake_paths(record, &intake);
    }
}

#[test]
fn invalid_case_contexts_fail_before_creating_intake() {
    let scratch = scratch();
    let root = scratch.path();
    let source = root.join("input");
    fs::write(&source, b"abc").unwrap();
    for config in [
        None,
        Some("[workspace]\ncases = []\n"),
        Some("[case]\nname = 'bad'\nversion = '0.1.0'\nunknown = true\n"),
        Some("[case]\nname = 'bad'\nversion = '0.1.0'\n[storage]\nstore_root = 'missing'\n"),
        Some("workspace_root = 'missing'\n[case]\nname = 'bad'\nversion = '0.1.0'\n"),
    ] {
        if let Some(config) = config {
            fs::write(root.join("orev.toml"), config).unwrap();
        }
        failure(&ingest(root, &[&source]));
        failure(&ingest_case(root, "bad", &[&source]));
        assert!(!root.join("00_intake").exists());
    }
    let nested = root.join("nested");
    fs::create_dir(&nested).unwrap();
    fs::write(
        root.join("orev.toml"),
        "[case]\nname = 'valid'\nversion = '0.1.0'\n",
    )
    .unwrap();
    for config in ["not toml", "[workspace]\ncases = []"] {
        fs::write(nested.join("orev.toml"), config).unwrap();
        failure(&ingest(&nested, &[&source]));
        failure(&ingest_case(&nested, "valid", &[&source]));
        assert!(!root.join("00_intake").exists());
        assert!(!nested.join("00_intake").exists());
    }
}

#[test]
fn missing_arguments_fail_and_double_dash_accepts_option_like_names() {
    let scratch = scratch();
    let case = case(scratch.path());
    let output = run(&case, &[OsStr::new("sample"), OsStr::new("ingest")]);
    assert_eq!(output.status.code(), Some(2));
    assert!(!case.join("00_intake").exists());
    fs::write(case.join("-sample with spaces"), b"abc").unwrap();
    success(&ingest(&case, &[Path::new("-sample with spaces")]));
    assert_eq!(
        fs::read(case.join("00_intake/-sample with spaces")).unwrap(),
        b"abc"
    );
}

#[test]
fn preflight_rejects_missing_duplicate_reserved_and_root_inputs_as_a_batch() {
    let scratch = scratch();
    let case = case(scratch.path());
    let good = scratch.path().join("good");
    fs::write(&good, b"abc").unwrap();
    fs::create_dir(scratch.path().join("other")).unwrap();
    let duplicate = scratch.path().join("other/good");
    fs::write(&duplicate, b"other").unwrap();
    for name in ["stage.yaml", ".orev-ingest-private"] {
        fs::write(scratch.path().join(name), b"reserved").unwrap();
    }
    for bad in [
        scratch.path().join("missing"),
        duplicate,
        scratch.path().join("stage.yaml"),
        scratch.path().join(".orev-ingest-private"),
        PathBuf::from("/"),
        PathBuf::from("."),
    ] {
        failure(&ingest(&case, &[&good, &bad]));
        assert!(
            !case.join("00_intake").exists(),
            "created intake for {bad:?}"
        );
    }
}

#[test]
fn existing_destinations_never_merge_or_overwrite_and_preserve_old_manifest_bytes() {
    let scratch = scratch();
    let case = case(scratch.path());
    let original = scratch.path().join("original");
    fs::write(&original, b"abc").unwrap();
    success(&ingest(&case, &[&original]));
    let intake = case.join("00_intake");
    let stage = intake.join("stage.yaml");
    let bytes = fs::read(&stage).unwrap();
    let fresh = scratch.path().join("fresh");
    fs::write(&fresh, b"fresh").unwrap();
    for kind in ["file", "directory", "symlink"] {
        let source = scratch.path().join(kind);
        fs::create_dir(&source).unwrap();
        let destination = intake.join(kind);
        match kind {
            "file" => fs::write(&destination, b"untouched").unwrap(),
            "directory" => fs::create_dir(&destination).unwrap(),
            _ => symlink("missing", &destination).unwrap(),
        }
        failure(&ingest(&case, &[&fresh, &source]));
        assert_eq!(fs::read(&stage).unwrap(), bytes);
        assert!(!intake.join("fresh").exists());
        assert!(fs::symlink_metadata(destination).is_ok());
        assert_no_scratch(&intake);
    }
    failure(&ingest(&case, &[&fresh, &original]));
    assert_eq!(fs::read(stage).unwrap(), bytes);
    assert_eq!(fs::read(intake.join("original")).unwrap(), b"abc");
}

#[test]
fn symlinks_special_files_and_non_utf8_paths_are_rejected_without_partial_ingest() {
    let scratch = scratch();
    let case = case(scratch.path());
    let good = scratch.path().join("good");
    fs::write(&good, b"abc").unwrap();
    let link = scratch.path().join("link");
    symlink(&good, &link).unwrap();
    let directory = scratch.path().join("tree");
    fs::create_dir(&directory).unwrap();
    symlink(&good, directory.join("hidden-link")).unwrap();
    let parent_link = scratch.path().join("parent-link");
    symlink(scratch.path(), &parent_link).unwrap();
    let fifo = scratch.path().join("pipe");
    let c_path = std::ffi::CString::new(fifo.as_os_str().as_encoded_bytes()).unwrap();
    assert_eq!(unsafe { libc::mkfifo(c_path.as_ptr(), 0o600) }, 0);
    let socket_root = tempfile::Builder::new()
        .prefix("orev-socket-")
        .tempdir_in("/tmp")
        .unwrap();
    let socket = socket_root.path().join("socket");
    let _socket = std::os::unix::net::UnixListener::bind(&socket).unwrap();
    let non_utf8 = scratch
        .path()
        .join(std::ffi::OsString::from_vec(vec![b'x', 0xff]));
    fs::write(&non_utf8, b"invalid").unwrap();
    let bad_tree = scratch.path().join("bad-tree");
    fs::create_dir(&bad_tree).unwrap();
    fs::write(
        bad_tree.join(std::ffi::OsString::from_vec(vec![0xff])),
        b"invalid",
    )
    .unwrap();
    for bad in [
        link,
        directory,
        parent_link.join("good"),
        fifo,
        socket,
        non_utf8,
        bad_tree,
    ] {
        failure(&ingest(&case, &[&good, &bad]));
        assert!(!case.join("00_intake").exists());
    }
}

#[test]
fn intake_and_manifest_symlinks_are_never_followed() {
    let scratch = scratch();
    let case = case(scratch.path());
    let source = scratch.path().join("source");
    fs::write(&source, b"abc").unwrap();
    let outside = scratch.path().join("outside");
    fs::create_dir(&outside).unwrap();
    let intake = case.join("00_intake");
    symlink(&outside, &intake).unwrap();
    failure(&ingest(&case, &[&source]));
    assert_eq!(fs::read_dir(&outside).unwrap().count(), 0);
    fs::remove_file(&intake).unwrap();
    fs::create_dir(&intake).unwrap();
    let old = outside.join("stage");
    fs::write(&old, b"[]\n").unwrap();
    symlink(&old, intake.join("stage.yaml")).unwrap();
    failure(&ingest(&case, &[&source]));
    assert_eq!(fs::read(old).unwrap(), b"[]\n");
    assert!(!intake.join("source").exists());
    assert_no_scratch(&intake);
}

#[test]
fn sources_must_not_overlap_intake() {
    let scratch = scratch();
    let case = case(scratch.path());
    let intake = case.join("00_intake");
    fs::create_dir_all(intake.join("tree")).unwrap();
    fs::write(intake.join("sample"), b"abc").unwrap();
    for bad in [
        &intake,
        &intake.join("sample"),
        &intake.join("tree"),
        &case,
        scratch.path(),
    ] {
        failure(&ingest(&case, &[bad]));
        assert!(!intake.join("stage.yaml").exists());
        assert_eq!(fs::read(intake.join("sample")).unwrap(), b"abc");
        assert_no_scratch(&intake);
    }
}

#[test]
fn malformed_manifests_are_rejected_without_modification() {
    let scratch = scratch();
    let case = case(scratch.path());
    let first = scratch.path().join("first");
    let fresh = scratch.path().join("fresh");
    fs::write(&first, b"abc").unwrap();
    fs::write(&fresh, b"new").unwrap();
    success(&ingest(&case, &[&first]));
    let intake = case.join("00_intake");
    let stage = intake.join("stage.yaml");
    let valid = manifest(&intake)[0].clone();
    let mut variants = vec![
        "not: a sequence\n".to_owned(),
        "[] trailing".to_owned(),
        "- kind: file\n".to_owned(),
    ];
    for (field, value) in [
        ("stored_path", Value::from("../outside")),
        ("stored_path", Value::from("/absolute")),
        ("stored_path", Value::from("a/../first")),
        ("stored_path", Value::from("stage.yaml")),
        ("sha256", Value::from("invalid")),
        ("sha256", Value::from("0".repeat(64))),
        ("byte_size", Value::from(4)),
        ("kind", Value::from("directory")),
        ("ingested_at", Value::from("yesterday")),
        ("ingested_at", Value::from("2026-01-01T00:00:00+01:00")),
        ("source_path", Value::from("/private/original")),
        ("unknown", Value::from(true)),
    ] {
        let mut record = valid.clone();
        record[field] = value;
        variants.push(serde_yaml_ng::to_string(&vec![record]).unwrap());
    }
    variants.push(serde_yaml_ng::to_string(&vec![valid.clone(), valid]).unwrap());
    for text in variants {
        fs::write(&stage, &text).unwrap();
        failure(&ingest(&case, &[&fresh]));
        assert_eq!(fs::read_to_string(&stage).unwrap(), text);
        assert!(!intake.join("fresh").exists());
        assert_eq!(fs::read(intake.join("first")).unwrap(), b"abc");
        assert_no_scratch(&intake);
    }
}

#[test]
fn invalid_directory_metadata_and_tampered_old_artifacts_block_append() {
    let scratch = scratch();
    let case = case(scratch.path());
    let tree = scratch.path().join("tree");
    fs::create_dir_all(tree.join("nested/empty")).unwrap();
    fs::write(tree.join("nested/file"), b"abc").unwrap();
    let fresh = scratch.path().join("fresh");
    fs::write(&fresh, b"fresh").unwrap();
    success(&ingest(&case, &[&tree]));
    let intake = case.join("00_intake");
    let stage = intake.join("stage.yaml");
    let valid = manifest(&intake)[0].clone();
    let mut variants = Vec::new();
    for path in [
        "/outside",
        "../outside",
        "nested",
        "other/nested",
        "tree/../outside",
    ] {
        let mut record = valid.clone();
        record["entries"][0]["path"] = Value::from(path);
        variants.push(record);
    }
    let mut unsorted = valid.clone();
    unsorted["entries"].as_sequence_mut().unwrap().reverse();
    variants.push(unsorted);
    let mut duplicate = valid.clone();
    let entry = duplicate["entries"][0].clone();
    duplicate["entries"].as_sequence_mut().unwrap().push(entry);
    variants.push(duplicate);
    let mut missing_parent = valid.clone();
    missing_parent["entries"]
        .as_sequence_mut()
        .unwrap()
        .remove(0);
    variants.push(missing_parent);
    let mut unknown = valid.clone();
    unknown["entries"][0]["unknown"] = Value::from(true);
    variants.push(unknown);
    for record in variants {
        let text = serde_yaml_ng::to_string(&vec![record]).unwrap();
        fs::write(&stage, &text).unwrap();
        failure(&ingest(&case, &[&fresh]));
        assert_eq!(fs::read_to_string(&stage).unwrap(), text);
        assert!(!intake.join("fresh").exists());
    }
    let valid = serde_yaml_ng::to_string(&vec![valid]).unwrap();
    fs::write(&stage, &valid).unwrap();
    fs::write(intake.join("tree/nested/file"), b"xyz").unwrap();
    failure(&ingest(&case, &[&fresh]));
    assert_eq!(fs::read_to_string(stage).unwrap(), valid);
    assert!(!intake.join("fresh").exists());
}

#[test]
fn cooperative_ingests_serialize_and_preserve_both_batches() {
    use std::os::fd::AsRawFd;
    let scratch = scratch();
    let case = case(scratch.path());
    let intake = case.join("00_intake");
    fs::create_dir(&intake).unwrap();
    let directory = fs::File::open(&intake).unwrap();
    assert_eq!(
        unsafe { libc::flock(directory.as_raw_fd(), libc::LOCK_EX) },
        0
    );
    let first = scratch.path().join("first");
    let second = scratch.path().join("second");
    fs::write(&first, b"abc").unwrap();
    fs::write(&second, b"other").unwrap();
    let mut children = Vec::new();
    for source in [&first, &second] {
        children.push(
            support::orev()
                .current_dir(&case)
                .args(["sample", "ingest"])
                .arg(source)
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()
                .unwrap(),
        );
    }
    assert!(!intake.join("stage.yaml").exists());
    assert_eq!(
        unsafe { libc::flock(directory.as_raw_fd(), libc::LOCK_UN) },
        0
    );
    for child in children {
        success(&child.wait_with_output().unwrap());
    }
    let records = manifest(&intake);
    assert_eq!(records.len(), 2);
    let mut names: Vec<_> = records
        .iter()
        .map(|record| record["stored_path"].as_str().unwrap())
        .collect();
    names.sort();
    assert_eq!(names, ["first", "second"]);
    assert_eq!(fs::read(intake.join("first")).unwrap(), b"abc");
    assert_eq!(fs::read(intake.join("second")).unwrap(), b"other");
    assert_no_scratch(&intake);
}

#[test]
fn source_mutation_after_preflight_cleans_staged_batch_and_preserves_old_manifest() {
    use std::os::fd::AsRawFd;
    let scratch = scratch();
    let case = case(scratch.path());
    let old = scratch.path().join("old");
    let first = scratch.path().join("first");
    let changed = scratch.path().join("changed");
    for path in [&old, &first, &changed] {
        fs::write(path, b"abc").unwrap();
    }
    success(&ingest(&case, &[&old]));
    let intake = case.join("00_intake");
    let bytes = fs::read(intake.join("stage.yaml")).unwrap();
    let directory = fs::File::open(&intake).unwrap();
    assert_eq!(
        unsafe { libc::flock(directory.as_raw_fd(), libc::LOCK_EX) },
        0
    );
    let mut child = support::orev()
        .current_dir(&case)
        .args(["sample", "ingest"])
        .args([&first, &changed])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    let waiting = loop {
        assert!(
            child.try_wait().unwrap().is_none(),
            "ingest exited before acquiring lock"
        );
        let channel = fs::read_to_string(format!("/proc/{}/wchan", child.id())).unwrap();
        if channel.trim() == "locks_lock_inode_wait" {
            break true;
        }
        if std::time::Instant::now() >= deadline {
            break false;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    };
    if waiting {
        fs::write(&changed, b"changed after preflight").unwrap();
    }
    assert_eq!(
        unsafe { libc::flock(directory.as_raw_fd(), libc::LOCK_UN) },
        0
    );
    let output = child.wait_with_output().unwrap();
    assert!(waiting, "ingest never reached the intake lock");
    failure(&output);
    assert!(String::from_utf8_lossy(&output.stderr).contains("source changed"));
    assert_eq!(fs::read(intake.join("stage.yaml")).unwrap(), bytes);
    assert_eq!(fs::read(intake.join("old")).unwrap(), b"abc");
    assert!(!intake.join("first").exists());
    assert!(!intake.join("changed").exists());
    assert_no_scratch(&intake);
}

#[test]
fn staged_manifest_write_failure_leaves_old_bytes_and_no_new_samples() {
    use std::os::unix::process::CommandExt;
    let scratch = scratch();
    let case = case(scratch.path());
    let old = scratch.path().join("old");
    let first = scratch.path().join("first");
    let second = scratch.path().join("second");
    for path in [&old, &first, &second] {
        fs::write(path, b"abc").unwrap();
    }
    success(&ingest(&case, &[&old]));
    let intake = case.join("00_intake");
    let bytes = fs::read(intake.join("stage.yaml")).unwrap();
    let mut command = support::orev();
    command
        .current_dir(&case)
        .args(["sample", "ingest"])
        .args([&first, &second]);
    unsafe {
        command.pre_exec(|| {
            if libc::signal(libc::SIGXFSZ, libc::SIG_IGN) == libc::SIG_ERR {
                return Err(std::io::Error::last_os_error());
            }
            let limit = libc::rlimit {
                rlim_cur: 256,
                rlim_max: 256,
            };
            if libc::setrlimit(libc::RLIMIT_FSIZE, &limit) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    failure(&command.output().unwrap());
    assert_eq!(fs::read(intake.join("stage.yaml")).unwrap(), bytes);
    assert_eq!(fs::read(intake.join("old")).unwrap(), b"abc");
    assert!(!intake.join("first").exists());
    assert!(!intake.join("second").exists());
    assert_no_scratch(&intake);
}
