#![cfg(target_os = "linux")]

use std::ffi::OsStr;
use std::fs;
use std::io::{self, Read, Write};
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::time::{Duration, Instant};

fn orev(root: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_orev"));
    command.current_dir(root).env("SHELL", "/not/a/shell");
    command
}

fn fixture(root: &Path, name: &str, body: &str) -> PathBuf {
    let bin = root.join("bin");
    fs::create_dir_all(&bin).unwrap();
    let path = bin.join(name);
    fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    path
}

fn records(root: &Path) -> Vec<PathBuf> {
    fs::read_dir(root)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.join("metadata.yaml").is_file())
        .collect()
}

fn metadata(directory: &Path) -> serde_yaml_ng::Value {
    serde_yaml_ng::from_str(&fs::read_to_string(directory.join("metadata.yaml")).unwrap()).unwrap()
}

fn success(output: &Output) {
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn doctor_and_run_share_version_detection_and_ghidra_does_not_execute() {
    let root = tempfile::tempdir().unwrap();
    success(&orev(root.path()).arg("init").output().unwrap());
    for (name, arg) in [
        ("java", "-version"),
        ("objdump", "--version"),
        ("readelf", "--version"),
        ("strings", "--version"),
    ] {
        fixture(
            root.path(),
            name,
            &format!(
                "test \"$#\" = 0 && exit 0\ntest \"$1\" = '{arg}' || exit 99\nprintf '{name} shared-version\\n' >&2"
            ),
        );
    }
    let installation = root.path().join("ghidra");
    fs::create_dir_all(installation.join("support")).unwrap();
    fs::create_dir_all(installation.join("Ghidra")).unwrap();
    fs::write(
        installation.join("Ghidra/application.properties"),
        "application.version=12.2\n",
    )
    .unwrap();
    let headless = installation.join("support/analyzeHeadless");
    fs::write(&headless, "#!/bin/sh\ntouch should-not-execute\nexit 99\n").unwrap();
    fs::set_permissions(&headless, fs::Permissions::from_mode(0o755)).unwrap();
    let output = orev(root.path())
        .arg("doctor")
        .env("PATH", root.path().join("bin"))
        .env("GHIDRA_INSTALL_DIR", &installation)
        .output()
        .unwrap();
    success(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    for name in ["java", "objdump", "readelf", "strings"] {
        assert!(
            stdout.contains(&format!("{name} shared-version")),
            "{stdout}"
        );
        let output = orev(root.path())
            .arg("run")
            .arg(root.path().join("bin").join(name))
            .output()
            .unwrap();
        success(&output);
    }
    for record in records(root.path()) {
        let info = metadata(&record);
        assert_eq!(
            info["tool"]["version"].as_str(),
            Some(format!("{} shared-version", info["tool"]["name"].as_str().unwrap()).as_str())
        );
    }
    assert!(stdout.contains("[✓] Ghidra: 12.2"), "{stdout}");
    assert!(!root.path().join("should-not-execute").exists());
    std::os::unix::fs::symlink(&headless, root.path().join("bin/analyzeHeadless")).unwrap();
    let output = orev(root.path())
        .arg("doctor")
        .env("PATH", root.path().join("bin"))
        .env_remove("GHIDRA_INSTALL_DIR")
        .output()
        .unwrap();
    success(&output);
    assert!(String::from_utf8_lossy(&output.stdout).contains("[✓] Ghidra: 12.2"));
    assert!(!root.path().join("should-not-execute").exists());
}

#[test]
fn doctor_reports_probe_failures_instead_of_claiming_tools_are_missing() {
    let root = tempfile::tempdir().unwrap();
    success(&orev(root.path()).arg("init").output().unwrap());
    fixture(root.path(), "java", "exec /bin/sleep 30");
    fixture(root.path(), "objdump", "exit 7");
    fixture(root.path(), "readelf", "exit 0");
    let start = Instant::now();
    let output = orev(root.path())
        .arg("doctor")
        .env("PATH", root.path().join("bin"))
        .env_remove("GHIDRA_INSTALL_DIR")
        .output()
        .unwrap();
    success(&output);
    assert!(start.elapsed() < Duration::from_secs(8));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("version probe timed out after 2 seconds"),
        "{stdout}"
    );
    assert!(
        stdout.contains("version probe exited with exit status: 7"),
        "{stdout}"
    );
    assert!(
        stdout.contains("version probe produced no version text"),
        "{stdout}"
    );
    assert!(
        stdout.contains("[!] strings: executable `strings` not found"),
        "{stdout}"
    );
}

fn wait_for_log(directory: &Path, text: &str) -> String {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        let log = fs::read_to_string(directory.join("output.log")).unwrap();
        if log.contains(text) {
            return log;
        }
        assert!(
            Instant::now() < deadline,
            "log did not contain {text:?}: {log:?}"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn metadata_and_logs_redact_current_username_components_without_changing_execution() {
    let root = tempfile::tempdir().unwrap();
    let user = current_username();
    let cwd = root.path().join("nonstandard").join(&user);
    fs::create_dir_all(&cwd).unwrap();
    let tool = fixture(
        &cwd,
        "custom",
        "printf 'CWD:%s\\nARG:%s\\n' \"$PWD\" \"$1\"\nprintf '\\033[31m%s\\033[0m\\n' \"$2\" >&2",
    );
    let output = orev(&cwd)
        .args([
            "run",
            "--tool-version",
            &format!("build /opt/{user}/sdk"),
            "--",
        ])
        .arg(&tool)
        .arg(format!("/media/disk/{user}/input"))
        .arg(format!("/tmp/{user}/file"))
        .env("TZ", format!("/zoneinfo/{user}/zone"))
        .env("USER", "spoofed-account")
        .env("LOGNAME", "spoofed-account")
        .output()
        .unwrap();
    success(&output);
    let terminal = String::from_utf8_lossy(&output.stdout);
    assert!(terminal.contains(&format!("CWD:{}", cwd.display())));
    assert!(terminal.contains(&format!("ARG:/media/disk/{user}/input")));
    assert!(terminal.contains(&format!("\x1b[31m/tmp/{user}/file\x1b[0m")));
    let directory = &records(&cwd)[0];
    let info = metadata(directory);
    let raw = fs::read_to_string(directory.join("metadata.yaml")).unwrap();
    let log = fs::read_to_string(directory.join("output.log")).unwrap();
    assert!(!raw.contains(&format!("/{user}/")), "{raw}");
    assert!(!log.contains(&format!("/{user}/")), "{log}");
    assert!(
        info["cwd"]
            .as_str()
            .unwrap()
            .ends_with("/nonstandard/<user>")
    );
    assert!(
        info["tool"]["executable"]
            .as_str()
            .unwrap()
            .contains("/nonstandard/<user>/bin/custom")
    );
    assert_eq!(
        info["tool"]["version"].as_str(),
        Some("build /opt/<user>/sdk")
    );
    assert_eq!(
        info["environment"]["variables"]["TZ"].as_str(),
        Some("/zoneinfo/<user>/zone")
    );
    assert!(log.contains("ARG:/media/disk/<user>/input"));
    assert!(log.contains("\x1b[31m/tmp/<user>/file\x1b[0m"));
    assert!(!log.contains("COMMAND="));
    assert!(!log.contains("Script started"));
    assert_eq!(
        fs::read_dir(directory).unwrap().count(),
        2,
        "no raw log artifacts"
    );
}

fn current_username() -> String {
    let output = Command::new("/usr/bin/id").arg("-un").output().unwrap();
    success(&output);
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

#[test]
fn redaction_diagnostics_cover_runner_errors() {
    let root = tempfile::tempdir().unwrap();
    let output = orev(root.path())
        .args([
            "run",
            "--no-version-probe",
            "--",
            &format!("/nonexistent/{}/orev-missing-tool", current_username()),
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let info = metadata(&records(root.path())[0]);
    assert!(
        info["error"]
            .as_str()
            .unwrap()
            .contains("/nonexistent/<user>/")
    );
    assert!(
        !fs::read_to_string(records(root.path())[0].join("metadata.yaml"))
            .unwrap()
            .contains(&format!("/{}/", current_username()))
    );
}

#[test]
fn run_preserves_arguments_and_captures_both_streams_without_a_workspace() {
    let root = tempfile::tempdir().unwrap();
    let tool = fixture(
        root.path(),
        "my tool",
        "test -t 0 && test -t 1 && test -t 2 || exit 99\nprintf '<%s>\\n' \"$@\"\nprintf 'SHELL:%s\\n' \"$SHELL\"\nprintf 'stderr marker\\n' >&2",
    );
    let args = [
        "a b",
        "",
        "a'b",
        "$(touch injected)",
        "; touch injected",
        "--help",
        "--version",
        "--",
        "*",
        "$HOME",
    ];
    let output = orev(root.path())
        .args(["run", "--tool-version", "fixture 1"])
        .arg(&tool)
        .args(args)
        .output()
        .unwrap();
    success(&output);
    let terminal = String::from_utf8(output.stdout).unwrap();
    for arg in args {
        assert!(terminal.contains(&format!("<{arg}>\r\n")), "{terminal:?}");
    }
    assert!(terminal.contains("stderr marker\r\n"));
    assert!(terminal.contains("SHELL:/not/a/shell\r\n"));
    assert!(!root.path().join("injected").exists());
    let directories = records(root.path());
    assert_eq!(directories.len(), 1);
    let directory = &directories[0];
    let filename = directory.file_name().unwrap().to_str().unwrap();
    let timestamp = filename.strip_prefix("my_tool-").unwrap();
    assert!(!timestamp.contains('-'));
    assert!(timestamp.ends_with('Z'));
    assert_eq!(timestamp.len(), 16);
    chrono::NaiveDateTime::parse_from_str(timestamp, "%Y%m%dT%H%M%SZ").unwrap();
    let log = fs::read_to_string(directory.join("output.log")).unwrap();
    assert!(log.contains(&terminal));
    let info = metadata(directory);
    assert!(info.get("schema").is_none());
    assert!(info.get("argv_hex").is_none());
    assert!(!directory.join("metadata.toml").exists());
    assert_eq!(info["status"].as_str(), Some("ok"));
    assert_eq!(info["exit_code"].as_i64(), Some(0));
    assert_eq!(info["tool"]["version"].as_str(), Some("fixture 1"));
    assert_eq!(info["tool"]["version_status"].as_str(), Some("provided"));
    assert_eq!(info["cwd"].as_str(), root.path().to_str());
    assert!(info["finished_at"].as_str().unwrap().ends_with('Z'));
    let recorded = info["argv"].as_sequence().unwrap();
    assert_eq!(recorded.len(), args.len() + 1);
    for (recorded, arg) in recorded[1..].iter().zip(args) {
        assert_eq!(recorded.as_str(), Some(arg));
    }
    assert_eq!(
        fs::metadata(directory).unwrap().permissions().mode() & 0o777,
        0o700
    );
    assert_eq!(
        fs::metadata(directory.join("output.log"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
}

#[test]
fn run_propagates_exit_status_and_retains_failed_run() {
    let root = tempfile::tempdir().unwrap();
    let output = orev(root.path())
        .args([
            "run",
            "--no-version-probe",
            "/bin/sh",
            "-c",
            "echo failure >&2; exit 42",
        ])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(42));
    let info = metadata(&records(root.path())[0]);
    assert_eq!(info["status"].as_str(), Some("failed"));
    assert_eq!(info["exit_code"].as_i64(), Some(42));
    assert_eq!(info["tool"]["version_status"].as_str(), Some("skipped"));
}

#[test]
fn environment_snapshot_records_host_and_allowlisted_settings_without_secrets() {
    let root = tempfile::tempdir().unwrap();
    let output = orev(root.path())
        .args(["run", "--no-version-probe", "--", "/bin/true"])
        .env("LANG", "C")
        .env("LC_ALL", "C")
        .env("TZ", "UTC")
        .env("TERM", "xterm-256color")
        .env("OREV_TEST_SECRET", "must-not-be-recorded")
        .env("AWS_SECRET_ACCESS_KEY", "must-not-be-recorded")
        .output()
        .unwrap();
    success(&output);
    let directory = &records(root.path())[0];
    let info = metadata(directory);
    let environment = &info["environment"];
    assert_eq!(
        environment["orev_version"].as_str(),
        Some(env!("CARGO_PKG_VERSION"))
    );
    assert_eq!(environment["os"].as_str(), Some("linux"));
    let kernel = fs::read_to_string("/proc/sys/kernel/osrelease").unwrap();
    assert_eq!(environment["kernel_release"].as_str(), Some(kernel.trim()));
    assert!(!environment["machine"].as_str().unwrap().is_empty());
    assert!(
        !environment["distribution"]["ID"]
            .as_str()
            .unwrap()
            .is_empty()
    );
    for (key, value) in [
        ("LANG", "C"),
        ("LC_ALL", "C"),
        ("TZ", "UTC"),
        ("TERM", "xterm-256color"),
    ] {
        assert_eq!(environment["variables"][key].as_str(), Some(value));
    }
    let raw = fs::read_to_string(directory.join("metadata.yaml")).unwrap();
    assert!(!raw.contains("must-not-be-recorded"));
    assert!(!raw.contains("AWS_SECRET_ACCESS_KEY"));
    assert!(environment["variables"].get("PATH").is_none());
    assert!(environment["variables"].get("HOME").is_none());
}

#[test]
fn runner_options_are_isolated_from_tool_options_with_or_without_separator() {
    for separator in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let tool = fixture(root.path(), "custom", "printf '<%s>\\n' \"$@\"");
        let tool_args = [
            "--tool-version",
            "tool-only",
            "--no-version-probe",
            "--help",
            "-h",
            "--version",
            "--",
            "--tool-version=literal",
            "",
        ];
        let mut command = orev(root.path());
        command.args(["run", "--tool-version", "runner-only"]);
        if separator {
            command.arg("--");
        }
        let output = command.arg(&tool).args(tool_args).output().unwrap();
        success(&output);
        let info = metadata(&records(root.path())[0]);
        assert_eq!(info["tool"]["version"].as_str(), Some("runner-only"));
        assert_eq!(info["tool"]["version_status"].as_str(), Some("provided"));
        let recorded: Vec<_> = info["argv"]
            .as_sequence()
            .unwrap()
            .iter()
            .map(|arg| arg.as_str().unwrap())
            .collect();
        assert_eq!(recorded[0], tool.to_str().unwrap());
        assert_eq!(&recorded[1..], tool_args);
        for arg in tool_args {
            assert!(String::from_utf8_lossy(&output.stdout).contains(&format!("<{arg}>\r\n")));
        }
    }
}

#[test]
fn separator_allows_a_tool_name_that_matches_a_runner_option() {
    let root = tempfile::tempdir().unwrap();
    fixture(root.path(), "--no-version-probe", "printf '<%s>\\n' \"$@\"");
    let output = orev(root.path())
        .args([
            "run",
            "--no-version-probe",
            "--",
            "--no-version-probe",
            "--",
            "--help",
        ])
        .env("PATH", "bin:/usr/bin:/bin")
        .output()
        .unwrap();
    success(&output);
    let info = metadata(&records(root.path())[0]);
    assert_eq!(info["tool"]["name"].as_str(), Some("--no-version-probe"));
    assert_eq!(
        info["argv"]
            .as_sequence()
            .unwrap()
            .iter()
            .map(|arg| arg.as_str().unwrap())
            .collect::<Vec<_>>(),
        ["--no-version-probe", "--", "--help"]
    );
}

#[test]
fn unknown_runner_options_and_missing_commands_after_separator_are_parse_errors() {
    let root = tempfile::tempdir().unwrap();
    for args in [
        vec!["run", "--not-an-orev-option", "/bin/true"],
        vec!["run", "--"],
        vec!["run", "--no-version-probe", "--"],
    ] {
        let output = orev(root.path()).args(args).output().unwrap();
        assert_eq!(output.status.code(), Some(2), "{output:?}");
    }
    let help = orev(root.path()).args(["run", "--help"]).output().unwrap();
    success(&help);
    assert!(String::from_utf8_lossy(&help.stdout).contains("orev run [OPTIONS] -- <TOOL>"));
    assert!(records(root.path()).is_empty());
}

#[test]
fn bare_and_relative_tools_resolve_without_changing_their_working_directory() {
    let root = tempfile::tempdir().unwrap();
    fixture(root.path(), "custom", "printf '%s\\n' \"$PWD\"");
    for tool in ["custom", "bin/custom"] {
        let output = orev(root.path())
            .args(["run", "--", tool])
            .env("PATH", "bin:/usr/bin:/bin")
            .output()
            .unwrap();
        success(&output);
        assert_eq!(
            String::from_utf8_lossy(&output.stdout).trim(),
            root.path().to_str().unwrap()
        );
    }
    assert_eq!(records(root.path()).len(), 2);
}

#[test]
fn large_output_is_streamed_without_truncating_the_transcript() {
    let root = tempfile::tempdir().unwrap();
    let output = orev(root.path())
        .args([
            "run",
            "--no-version-probe",
            "/bin/sh",
            "-c",
            "i=0; while [ \"$i\" -lt 6000 ]; do printf '0123456789abcdefghij\\n'; i=$((i + 1)); done; printf 'END\\n' >&2",
        ])
        .output()
        .unwrap();
    success(&output);
    let terminal = String::from_utf8(output.stdout).unwrap();
    assert_eq!(terminal.matches("0123456789abcdefghij\r\n").count(), 6000);
    assert!(terminal.ends_with("END\r\n"));
    let log = fs::read_to_string(records(root.path())[0].join("output.log")).unwrap();
    assert!(log.contains(&terminal));
}

#[test]
fn missing_tool_is_recorded_and_execution_does_not_require_script_in_path() {
    for existing_tool in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let mut command = orev(root.path());
        command.args(["run", "--no-version-probe"]);
        if existing_tool {
            command.arg("/bin/true").env("PATH", "");
        } else {
            command.arg("orev-test-nonexistent-tool");
        }
        let output = command.output().unwrap();
        if existing_tool {
            success(&output);
            assert_eq!(
                metadata(&records(root.path())[0])["status"].as_str(),
                Some("ok")
            );
            continue;
        }
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty(), "{output:?}");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(
            stderr
                .lines()
                .filter(|line| line.starts_with("Error:"))
                .collect::<Vec<_>>(),
            ["Error: executable `orev-test-nonexistent-tool` not found or not executable"]
        );
        let info = metadata(&records(root.path())[0]);
        assert_eq!(info["status"].as_str(), Some("error"));
        assert!(info["error"].as_str().unwrap().contains("not found"));
        assert!(info["exit_code"].is_null());
    }
}

#[test]
fn known_tools_use_their_own_version_arguments_and_stderr_is_supported() {
    for (name, arg) in [
        ("java", "-version"),
        ("javac", "-version"),
        ("r2", "-v"),
        ("radare2", "-v"),
        ("objdump", "--version"),
        ("readelf", "--version"),
        ("strings", "--version"),
        ("gdb", "--version"),
        ("lldb", "--version"),
        ("binwalk", "--version"),
        ("file", "--version"),
        ("python3.14", "--version"),
        ("qemu-system-aarch64", "--version"),
        ("aarch64-linux-gnu-objdump", "--version"),
    ] {
        let root = tempfile::tempdir().unwrap();
        let tool = fixture(
            root.path(),
            name,
            &format!(
                "if [ \"$1\" = '{arg}' ]; then echo '{name} fixture 2.0' >&2; exit 0; fi\necho actual-run"
            ),
        );
        let output = orev(root.path())
            .arg("run")
            .arg(tool)
            .arg("work")
            .output()
            .unwrap();
        success(&output);
        assert!(String::from_utf8_lossy(&output.stdout).contains("actual-run"));
        assert!(!String::from_utf8_lossy(&output.stdout).contains("fixture 2.0"));
        let directory = &records(root.path())[0];
        let info = metadata(directory);
        assert_eq!(
            info["tool"]["version"].as_str(),
            Some(format!("{name} fixture 2.0").as_str())
        );
        assert_eq!(info["tool"]["version_status"].as_str(), Some("detected"));
        assert!(
            !fs::read_to_string(directory.join("output.log"))
                .unwrap()
                .contains("fixture 2.0")
        );
    }
}

#[test]
fn ghidra_version_is_read_from_installation_without_executing_a_probe() {
    let root = tempfile::tempdir().unwrap();
    let installation = root.path().join("installation");
    fs::create_dir_all(installation.join("support")).unwrap();
    fs::create_dir_all(installation.join("Ghidra")).unwrap();
    fs::write(
        installation.join("Ghidra/application.properties"),
        "application.name=Ghidra\napplication.version=12.1.4\n",
    )
    .unwrap();
    let headless = installation.join("support/analyzeHeadless");
    fs::write(
        &headless,
        "#!/bin/sh\ntest \"$#\" = 0 || exit 99\necho ghidra-run\n",
    )
    .unwrap();
    fs::set_permissions(&headless, fs::Permissions::from_mode(0o755)).unwrap();
    let link = root.path().join("analyzeHeadless");
    std::os::unix::fs::symlink(headless, &link).unwrap();
    success(&orev(root.path()).arg("run").arg(link).output().unwrap());
    let info = metadata(&records(root.path())[0]);
    assert_eq!(info["tool"]["version"].as_str(), Some("12.1.4"));
    assert_eq!(info["tool"]["version_status"].as_str(), Some("detected"));
}

#[test]
fn failed_empty_unsupported_and_timed_out_probes_warn_but_do_not_block_execution() {
    for (name, probe_body) in [
        ("java", "exit 3"),
        ("python", "exit 0"),
        ("unknown-tool", "exit 99"),
        ("file", "sleep 30"),
    ] {
        let root = tempfile::tempdir().unwrap();
        let tool = fixture(
            root.path(),
            name,
            &format!("if [ \"$#\" != 0 ]; then {probe_body}; fi\necho executed"),
        );
        let start = Instant::now();
        let output = orev(root.path()).arg("run").arg(tool).output().unwrap();
        success(&output);
        assert!(start.elapsed() < Duration::from_secs(8));
        assert!(String::from_utf8_lossy(&output.stdout).contains("executed"));
        assert!(String::from_utf8_lossy(&output.stderr).contains("Warning: tool version unknown"));
        let info = metadata(&records(root.path())[0]);
        assert_eq!(info["tool"]["version_status"].as_str(), Some("unknown"));
        assert!(
            !info["tool"]["version_diagnostic"]
                .as_str()
                .unwrap()
                .is_empty()
        );
    }
}

#[test]
fn run_preserves_non_utf8_arguments_and_multiple_runs_never_overwrite() {
    let root = tempfile::tempdir().unwrap();
    let arg = OsStr::from_bytes(b"raw-\xff");
    for _ in 0..2 {
        let output = orev(root.path())
            .args(["run", "--no-version-probe", "/usr/bin/printf", "%s"])
            .arg(arg)
            .output()
            .unwrap();
        success(&output);
        assert_eq!(output.stdout, b"raw-\xff");
        assert!(String::from_utf8_lossy(&output.stderr).contains("non-UTF-8 argument bytes"));
    }
    let directories = records(root.path());
    assert_eq!(directories.len(), 2);
    for directory in directories {
        let info = metadata(&directory);
        assert_eq!(info["argv"][2].as_str(), Some("raw-\u{fffd}"));
        assert!(info.get("argv_hex").is_none());
    }
}

#[test]
fn cli_requires_a_command_and_rejects_conflicting_version_options() {
    let root = tempfile::tempdir().unwrap();
    for args in [
        vec!["run"],
        vec![
            "run",
            "--tool-version",
            "1",
            "--no-version-probe",
            "/bin/true",
        ],
        vec!["run", "--tool-version", "", "/bin/true"],
    ] {
        assert_eq!(
            orev(root.path()).args(args).output().unwrap().status.code(),
            Some(2)
        );
    }
    assert!(records(root.path()).is_empty());
}

#[test]
fn interrupting_a_version_probe_reaps_it_and_never_starts_the_tool() {
    let root = tempfile::tempdir().unwrap();
    let tool = fixture(
        root.path(),
        "file",
        "if [ \"$1\" = '--version' ]; then echo $$ > probe.pid; exec sleep 30; fi\ntouch actual-run",
    );
    let mut child = orev(root.path())
        .arg("run")
        .arg(tool)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(8);
    while !root.path().join("probe.pid").exists() {
        if Instant::now() >= deadline {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("version probe did not start");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let pid: libc::pid_t = fs::read_to_string(root.path().join("probe.pid"))
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    assert_eq!(
        unsafe { libc::kill(child.id() as libc::pid_t, libc::SIGINT) },
        0
    );
    assert_eq!(wait(&mut child).code(), Some(130));
    assert!(!root.path().join("actual-run").exists());
    assert_eq!(unsafe { libc::kill(pid, 0) }, -1);
    assert_eq!(io::Error::last_os_error().raw_os_error(), Some(libc::ESRCH));
    let info = metadata(&records(root.path())[0]);
    assert_eq!(info["exit_code"].as_i64(), Some(130));
    assert_eq!(info["termination_signal"].as_i64(), Some(2));
}

fn interactive(root: &Path, body: &str) -> (Child, fs::File) {
    let mut master = -1;
    let mut slave = -1;
    let size = libc::winsize {
        ws_row: 24,
        ws_col: 80,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };
    assert_eq!(
        unsafe {
            libc::openpty(
                &mut master,
                &mut slave,
                std::ptr::null_mut(),
                std::ptr::null(),
                &size,
            )
        },
        0
    );
    let master = unsafe { fs::File::from_raw_fd(master) };
    let slave = unsafe { fs::File::from_raw_fd(slave) };
    // Keep the master out of the spawned process, so EOF and hangups work normally.
    assert_ne!(
        unsafe { libc::fcntl(master.as_raw_fd(), libc::F_SETFD, libc::FD_CLOEXEC) },
        -1
    );
    let mut command = orev(root);
    command
        .args(["run", "--no-version-probe", "/bin/sh", "-c", body])
        .stdin(slave.try_clone().unwrap())
        .stdout(slave.try_clone().unwrap())
        .stderr(slave);
    unsafe {
        command.pre_exec(|| {
            if libc::setsid() == -1 || libc::ioctl(0, libc::TIOCSCTTY, 0) == -1 {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        });
    }
    (command.spawn().unwrap(), master)
}

fn read_until(master: &mut fs::File, text: &str) -> Vec<u8> {
    let deadline = Instant::now() + Duration::from_secs(8);
    let mut output = Vec::new();
    loop {
        assert!(
            Instant::now() < deadline,
            "timed out waiting for {text:?}: {}",
            String::from_utf8_lossy(&output)
        );
        let mut poll = libc::pollfd {
            fd: master.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        assert_ne!(unsafe { libc::poll(&mut poll, 1, 100) }, -1);
        if poll.revents & libc::POLLIN != 0 {
            let mut buffer = [0; 4096];
            let size = master.read(&mut buffer).unwrap();
            assert_ne!(size, 0);
            output.extend_from_slice(&buffer[..size]);
            if String::from_utf8_lossy(&output).contains(text) {
                return output;
            }
        }
    }
}

fn wait(child: &mut Child) -> std::process::ExitStatus {
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            return status;
        }
        if Instant::now() >= deadline {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("interactive command did not finish");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn record_integrity_failures_warn_while_silent_tools_continue_and_preserve_exit_code() {
    use std::io::BufRead;
    use std::sync::mpsc;

    for changed in [
        "deleted-log",
        "replaced-log",
        "replaced-metadata",
        "replaced-directory",
    ] {
        let root = tempfile::tempdir().unwrap();
        let mut child = orev(root.path())
            .args([
                "run",
                "--no-version-probe",
                "--",
                "/bin/sh",
                "-c",
                "printf 'ready\\n'; sleep 6; printf 'completed\\n'; exit 7",
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let (sender, receiver) = mpsc::channel();
        let stderr = child.stderr.take().unwrap();
        let diagnostics = std::thread::spawn(move || {
            let mut lines = Vec::new();
            for line in io::BufReader::new(stderr).lines() {
                let line = line.unwrap();
                if line.contains("Warning: run record integrity lost") {
                    sender.send(()).unwrap();
                }
                lines.push(line);
            }
            lines.join("\n")
        });
        let deadline = Instant::now() + Duration::from_secs(8);
        let directory = loop {
            if let Some(directory) = records(root.path()).first() {
                break directory.clone();
            }
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(10));
        };
        wait_for_log(&directory, "ready");
        let moved = root.path().join("moved-records");
        match changed {
            "deleted-log" => fs::remove_file(directory.join("output.log")).unwrap(),
            "replaced-log" => {
                fs::rename(directory.join("output.log"), directory.join("original.log")).unwrap();
                fs::write(directory.join("output.log"), b"replacement").unwrap();
            }
            "replaced-metadata" => {
                fs::rename(
                    directory.join("metadata.yaml"),
                    directory.join("original.yaml"),
                )
                .unwrap();
                fs::write(directory.join("metadata.yaml"), b"replacement").unwrap();
            }
            "replaced-directory" => {
                fs::rename(&directory, &moved).unwrap();
                fs::create_dir(&directory).unwrap();
                fs::write(directory.join("metadata.yaml"), b"replacement").unwrap();
            }
            _ => unreachable!(),
        }
        receiver.recv_timeout(Duration::from_secs(3)).unwrap();
        assert!(
            child.try_wait().unwrap().is_none(),
            "{changed}: tool was terminated"
        );
        let output = child.wait_with_output().unwrap();
        assert_eq!(output.status.code(), Some(1), "{changed}");
        assert!(String::from_utf8_lossy(&output.stdout).contains("completed"));
        assert!(diagnostics.join().unwrap().contains("integrity lost"));
        let original_metadata = match changed {
            "replaced-metadata" => directory.join("original.yaml"),
            "replaced-directory" => moved.join("metadata.yaml"),
            _ => directory.join("metadata.yaml"),
        };
        let info: serde_yaml_ng::Value =
            serde_yaml_ng::from_str(&fs::read_to_string(original_metadata).unwrap()).unwrap();
        assert_eq!(info["exit_code"].as_u64(), Some(7));
        assert_eq!(info["status"].as_str(), Some("error"));
        assert!(info["error"].as_str().unwrap().contains("integrity lost"));
        match changed {
            "deleted-log" => assert!(!directory.join("output.log").exists()),
            "replaced-log" => assert_eq!(
                fs::read(directory.join("output.log")).unwrap(),
                b"replacement"
            ),
            _ => assert_eq!(
                fs::read(directory.join("metadata.yaml")).unwrap(),
                b"replacement"
            ),
        }
    }
}

#[test]
fn pty_streams_before_exit_accepts_input_and_tracks_terminal_size() {
    let root = tempfile::tempdir().unwrap();
    let (mut child, mut master) = interactive(
        root.path(),
        "test -t 0 && test -t 1 && test -t 2 || exit 99; stty size; printf 'READY>'; read -r answer; stty size; printf 'ANSWER:%s\\n' \"$answer\"",
    );
    let initial = read_until(&mut master, "READY>");
    assert!(String::from_utf8_lossy(&initial).contains("24 80"));
    assert!(child.try_wait().unwrap().is_none());
    let directory = &records(root.path())[0];
    assert_eq!(metadata(directory)["status"].as_str(), Some("running"));
    wait_for_log(directory, "READY>");
    let size = libc::winsize {
        ws_row: 40,
        ws_col: 120,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };
    assert_eq!(
        unsafe { libc::ioctl(master.as_raw_fd(), libc::TIOCSWINSZ, &size) },
        0
    );
    std::thread::sleep(Duration::from_millis(100));
    master.write_all(b"hello world\n").unwrap();
    let output = read_until(&mut master, "ANSWER:hello world");
    assert!(String::from_utf8_lossy(&output).contains("40 120"));
    assert!(wait(&mut child).success());
    assert_eq!(metadata(directory)["status"].as_str(), Some("ok"));
}

#[test]
fn pty_ctrl_c_reaches_tool_and_finalizes_metadata() {
    let root = tempfile::tempdir().unwrap();
    let (mut child, mut master) = interactive(
        root.path(),
        &format!("printf '/tmp/{}/READY>'; exec sleep 30", current_username()),
    );
    read_until(&mut master, "READY>");
    master.write_all(&[3]).unwrap();
    assert_eq!(wait(&mut child).code(), Some(130));
    let info = metadata(&records(root.path())[0]);
    assert_eq!(info["exit_code"].as_i64(), Some(130));
    assert_eq!(info["status"].as_str(), Some("failed"));
    let log = fs::read_to_string(records(root.path())[0].join("output.log")).unwrap();
    assert!(log.contains("/tmp/<user>/READY>"));
    assert!(!log.contains(&format!("/{}/", current_username())));
}

#[test]
fn pty_restores_outer_terminal_modes_after_tool_exit() {
    let root = tempfile::tempdir().unwrap();
    let (mut child, mut master) =
        interactive(root.path(), "printf 'READY>'; read -r answer; exit 0");
    read_until(&mut master, "READY>");
    master.write_all(b"done\n").unwrap();
    assert!(wait(&mut child).success());
    let mut modes: libc::termios = unsafe { std::mem::zeroed() };
    assert_eq!(
        unsafe { libc::tcgetattr(master.as_raw_fd(), &mut modes) },
        0
    );
    assert_ne!(modes.c_lflag & libc::ICANON, 0);
    assert_ne!(modes.c_lflag & libc::ECHO, 0);
}

#[test]
fn piped_stdin_is_forwarded_without_requiring_an_outer_terminal() {
    let root = tempfile::tempdir().unwrap();
    let mut child = orev(root.path())
        .args([
            "run",
            "--no-version-probe",
            "--",
            "/bin/sh",
            "-c",
            "read -r value; printf 'INPUT:%s\\n' \"$value\"",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"hello from pipe\n")
        .unwrap();
    let output = child.wait_with_output().unwrap();
    success(&output);
    assert!(String::from_utf8_lossy(&output.stdout).contains("INPUT:hello from pipe"));
}

#[test]
fn partial_paths_are_redacted_before_live_logs_are_written() {
    let root = tempfile::tempdir().unwrap();
    let tool = fixture(
        root.path(),
        "stream",
        &format!(
            "printf '/tm'; sleep 0.05; printf 'p/{}'; sleep 0.05; printf '/READY>'; read -r answer; printf '%s\\n' \"$answer\"",
            current_username()
        ),
    );
    let (mut child, mut master) = interactive(root.path(), &format!("exec '{}'", tool.display()));
    let terminal = read_until(&mut master, "READY>");
    assert!(
        String::from_utf8_lossy(&terminal).contains(&format!("/tmp/{}/READY>", current_username()))
    );
    let directory = &records(root.path())[0];
    let log = wait_for_log(directory, "/tmp/<user>/READY>");
    assert!(!log.contains(&format!("/{}/", current_username())));
    assert!(child.try_wait().unwrap().is_none());
    master.write_all(b"done\n").unwrap();
    read_until(&mut master, "done");
    assert!(wait(&mut child).success());
}

#[test]
fn signal_sent_to_orev_is_forwarded_and_recorded() {
    let root = tempfile::tempdir().unwrap();
    let (mut child, mut master) = interactive(root.path(), "printf 'READY>'; exec sleep 30");
    read_until(&mut master, "READY>");
    assert_eq!(
        unsafe { libc::kill(child.id() as libc::pid_t, libc::SIGTERM) },
        0
    );
    assert_eq!(wait(&mut child).code(), Some(143));
    let info = metadata(&records(root.path())[0]);
    assert_eq!(info["exit_code"].as_i64(), Some(143));
    assert_eq!(info["termination_signal"].as_i64(), Some(15));
    assert_eq!(info["status"].as_str(), Some("failed"));
}
