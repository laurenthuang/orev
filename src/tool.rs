use serde::Serialize;
use std::env;
use std::ffi::OsStr;
use std::fs;
use std::io::{self, Read, Seek};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const PROBE_TIMEOUT: Duration = Duration::from_secs(2);
const OUTPUT_LIMIT: u64 = 16 * 1024;

#[derive(Serialize)]
pub struct Tool {
    pub name: String,
    pub executable: Option<PathBuf>,
    pub version: String,
    pub version_source: String,
    pub version_status: &'static str,
    pub version_diagnostic: Option<String>,
}

impl Tool {
    pub fn new(name: String) -> Self {
        Self {
            name,
            executable: None,
            version: "unknown".into(),
            version_source: "none".into(),
            version_status: "unknown",
            version_diagnostic: None,
        }
    }

    pub fn detect_version(&mut self, executable: &Path, cancelled: impl Fn() -> bool) {
        let detected = if is_ghidra(&self.name) {
            self.version_source = "Ghidra/application.properties".into();
            ghidra_version(executable)
        } else if let Some(arg) = version_arg(&self.name) {
            self.version_source = format!("{} {arg}", executable.display());
            probe(executable, arg, cancelled)
        } else {
            Err(io::Error::other(
                "no version detector for this tool; use --tool-version or --no-version-probe",
            ))
        };
        match detected {
            Ok(version) => {
                self.version = version;
                self.version_status = "detected";
                self.version_diagnostic = None;
            }
            Err(error) => {
                self.version = "unknown".into();
                self.version_status = "unknown";
                self.version_diagnostic = Some(error.to_string());
            }
        }
    }
}

pub fn resolve(program: &OsStr) -> io::Result<PathBuf> {
    use std::os::unix::fs::PermissionsExt;

    let candidates = if Path::new(program).is_absolute()
        || Path::new(program).components().count() > 1
        || program.to_string_lossy().contains('/')
    {
        vec![env::current_dir()?.join(program)]
    } else {
        let path = env::var_os("PATH").unwrap_or_else(|| "/bin:/usr/bin".into());
        env::split_paths(&path)
            .map(|directory| directory.join(program))
            .collect()
    };
    for candidate in candidates {
        match fs::metadata(&candidate) {
            Ok(info) if info.is_file() && info.permissions().mode() & 0o111 != 0 => {
                return Ok(env::current_dir()?.join(candidate));
            }
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::NotFound,
        format!(
            "executable `{}` not found or not executable",
            program.to_string_lossy()
        ),
    ))
}

fn is_ghidra(name: &str) -> bool {
    matches!(name, "analyzeHeadless" | "ghidra" | "ghidraRun")
}

fn version_arg(name: &str) -> Option<&'static str> {
    match name {
        "java" | "javac" => Some("-version"),
        "r2" | "radare2" => Some("-v"),
        "objdump" | "readelf" | "strings" | "gdb" | "lldb" | "binwalk" | "file" | "magika"
        | "diec" | "python" | "python2" | "python3" | "qemu" => Some("--version"),
        name if name.starts_with("qemu-") => Some("--version"),
        name if name.starts_with("python3.") || name.starts_with("python2.") => Some("--version"),
        name if ["-objdump", "-readelf", "-strings", "-gdb"]
            .iter()
            .any(|suffix| name.ends_with(suffix)) =>
        {
            Some("--version")
        }
        _ => None,
    }
}

fn ghidra_version(executable: &Path) -> io::Result<String> {
    let resolved = fs::canonicalize(executable)?;
    for parent in resolved.ancestors().skip(1).take(2) {
        let path = parent.join("Ghidra/application.properties");
        let content = match fs::read_to_string(&path) {
            Ok(content) => content,
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error),
        };
        for line in content.lines() {
            if let Some((key, value)) = line.split_once('=')
                && key.trim() == "application.version"
                && !value.trim().is_empty()
            {
                return Ok(value.trim().to_owned());
            }
        }
        return Err(io::Error::other(format!(
            "application.version missing from {}",
            path.display()
        )));
    }
    Err(io::Error::new(
        io::ErrorKind::NotFound,
        "Ghidra/application.properties not found beside executable",
    ))
}

pub fn probe(executable: &Path, arg: &str, cancelled: impl Fn() -> bool) -> io::Result<String> {
    let mut stdout = tempfile::tempfile()?;
    let mut stderr = tempfile::tempfile()?;
    let mut command = Command::new(executable);
    command
        .arg(arg)
        .stdin(Stdio::null())
        .stdout(stdout.try_clone()?)
        .stderr(stderr.try_clone()?);
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    let mut child = command.spawn()?;
    let start = Instant::now();
    let result = loop {
        if cancelled() {
            break Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "version probe interrupted",
            ));
        }
        match child.try_wait() {
            Ok(Some(status)) => break Ok(status),
            Ok(None) if start.elapsed() < PROBE_TIMEOUT => {
                std::thread::sleep(Duration::from_millis(10));
            }
            Ok(None) => {
                break Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "version probe timed out after 2 seconds",
                ));
            }
            Err(error) => break Err(error),
        }
    };
    {
        // Include helpers still holding the capture files after their parent exits.
        if let Err(error) = rustix::process::kill_process_group(
            rustix::process::Pid::from_child(&child),
            rustix::process::Signal::KILL,
        ) && error != rustix::io::Errno::SRCH
        {
            return Err(error.into());
        }
    }
    child.wait()?;
    let status = result?;
    if !status.success() {
        return Err(io::Error::other(format!(
            "version probe exited with {status}"
        )));
    }
    let mut output = Vec::new();
    for file in [&mut stdout, &mut stderr] {
        file.rewind()?;
        file.take(OUTPUT_LIMIT).read_to_end(&mut output)?;
        output.push(b'\n');
    }
    String::from_utf8_lossy(&output)
        .lines()
        .find(|line| !line.trim().is_empty())
        .map(|line| line.trim().to_owned())
        .ok_or_else(|| io::Error::other("version probe produced no version text"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_rules_keep_tool_specific_arguments() {
        for (name, argument) in [
            ("java", "-version"),
            ("javac", "-version"),
            ("r2", "-v"),
            ("radare2", "-v"),
            ("magika", "--version"),
            ("diec", "--version"),
            ("python3.14", "--version"),
            ("qemu-system-aarch64", "--version"),
            ("aarch64-linux-gnu-objdump", "--version"),
        ] {
            assert_eq!(version_arg(name), Some(argument), "{name}");
        }
    }

    #[test]
    fn version_names_are_case_sensitive_and_exact() {
        assert_eq!(version_arg("JAVA"), None);
        assert_eq!(version_arg("python.backup"), None);
        assert!(is_ghidra("analyzeHeadless"));
        assert!(!is_ghidra("ANALYZEHEADLESS"));
        assert!(!is_ghidra("analyzeHeadless.backup"));
    }
}
