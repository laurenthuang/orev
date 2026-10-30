mod integrity;
mod redact;
#[cfg(test)]
mod redact_bounded;
mod terminal;

use self::redact::{current_username, redact_value};
use crate::tool::{Tool, resolve};
use chrono::{SecondsFormat, Utc};
use serde::Serialize;
use std::collections::BTreeMap;
use std::env;
use std::ffi::OsString;
use std::fs;
use std::fs::{DirBuilder, File, OpenOptions};
use std::io::{self, Seek, Write};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicI32, Ordering};
use std::time::Instant;

static SIGNAL: AtomicI32 = AtomicI32::new(0);

#[derive(Serialize)]
struct Metadata {
    #[serde(skip)]
    username: Vec<u8>,
    argv: Vec<String>,
    cwd: PathBuf,
    started_at: String,
    finished_at: Option<String>,
    duration_s: Option<f64>,
    status: &'static str,
    exit_code: Option<u32>,
    termination_signal: Option<i32>,
    error: Option<String>,
    backend: &'static str,
    tool: Tool,
    environment: Environment,
}

#[derive(Serialize)]
struct Environment {
    orev_version: &'static str,
    os: &'static str,
    distribution: BTreeMap<String, String>,
    kernel_release: Option<String>,
    machine: Option<String>,
    variables: BTreeMap<String, String>,
    system_timezone: Option<String>,
    diagnostics: Vec<String>,
}

pub fn execute(argv: &[OsString], version: Option<String>, skip_probe: bool) -> io::Result<u32> {
    let username = current_username()?;
    let program = argv
        .first()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "a tool command is required"))?;
    let cwd = env::current_dir()?;
    let name = Path::new(program)
        .file_name()
        .unwrap_or(program.as_os_str())
        .to_string_lossy()
        .into_owned();
    let slug: String = name
        .chars()
        .take(64)
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.') {
                c
            } else {
                '_'
            }
        })
        .collect();
    let started = Utc::now();
    let timer = Instant::now();
    let directory_name = format!(
        "{}-{}",
        if slug.is_empty() { "tool" } else { &slug },
        started.format("%Y%m%dT%H%M%SZ")
    );
    let directory = create_run_directory(&cwd, &directory_name)?;
    let mut metadata_file = create_record(&directory.join("metadata.yaml"))?;
    let log_path = directory.join("output.txt");
    let mut output_file = create_record(&log_path)?;
    let mut metadata = Metadata {
        username,
        argv: argv
            .iter()
            .map(|arg| arg.to_string_lossy().into())
            .collect(),
        cwd,
        started_at: started.to_rfc3339_opts(SecondsFormat::Nanos, true),
        finished_at: None,
        duration_s: None,
        status: "preparing",
        exit_code: None,
        termination_signal: None,
        error: None,
        backend: "portable-pty (PTY, merged stdout/stderr)",
        environment: environment(),
        tool: Tool::new(name),
    };
    if argv.iter().any(|arg| arg.to_str().is_none()) {
        eprintln!(
            "Warning: non-UTF-8 argument bytes are replaced with U+FFFD in metadata; execution preserves the original bytes"
        );
    }
    save(&mut metadata_file, &metadata)?;
    let monitor = integrity::Monitor::start(&directory, &metadata_file, &output_file)?;
    let _signals = Signals::install()?;
    eprintln!("orev: run records: {}", directory.display());
    let mut result = run(
        argv,
        version,
        skip_probe,
        &mut output_file,
        &mut metadata_file,
        &mut metadata,
    )
    .and_then(|code| {
        metadata.exit_code = Some(code);
        output_file.sync_all()?;
        Ok(code)
    });
    let signal = SIGNAL.swap(0, Ordering::Relaxed);
    if signal != 0 {
        metadata.termination_signal = Some(signal);
        metadata.exit_code = Some((128 + signal) as u32);
        result = Ok((128 + signal) as u32);
    }
    metadata.finished_at = Some(Utc::now().to_rfc3339_opts(SecondsFormat::Nanos, true));
    metadata.duration_s = Some(timer.elapsed().as_secs_f64());
    match &result {
        Ok(code) => {
            metadata.exit_code = Some(*code);
            metadata.status = if *code == 0 { "ok" } else { "failed" };
        }
        Err(error) => {
            metadata.status = "error";
            metadata.error = Some(error.to_string());
        }
    }
    let saved = save(&mut metadata_file, &metadata);
    if let Err(integrity_error) = monitor.finish() {
        let mut error = integrity_error.to_string();
        if let Err(run_error) = result {
            error = format!("{run_error}; {error}");
        }
        if let Err(save_error) = saved {
            error.push_str(&format!("; cannot save metadata: {save_error}"));
        }
        metadata.status = "error";
        metadata.error = Some(error.clone());
        if let Err(save_error) = save(&mut metadata_file, &metadata) {
            error.push_str(&format!("; cannot save error metadata: {save_error}"));
        }
        return Err(io::Error::other(error));
    }
    saved?;
    result
}

fn create_run_directory(cwd: &Path, name: &str) -> io::Result<PathBuf> {
    let mut suffix = 0u64;
    loop {
        let directory = cwd.join(if suffix == 0 {
            name.to_owned()
        } else {
            format!("{name}-{suffix}")
        });
        let mut builder = DirBuilder::new();
        builder.mode(0o700);
        match builder.create(&directory) {
            Ok(()) => return Ok(directory),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                suffix = suffix
                    .checked_add(1)
                    .ok_or_else(|| io::Error::other("run directory suffix exhausted"))?;
            }
            Err(error) => return Err(error),
        }
    }
}

fn create_record(path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.read(true).write(true).create_new(true);
    options.mode(0o600);
    options.open(path)
}

fn save(file: &mut File, metadata: &Metadata) -> io::Result<()> {
    let mut value = serde_yaml_ng::to_value(metadata).map_err(io::Error::other)?;
    redact_value(&mut value, &metadata.username);
    let content = serde_yaml_ng::to_string(&value).map_err(io::Error::other)?;
    file.rewind()?;
    file.write_all(content.as_bytes())?;
    file.set_len(content.len() as u64)?;
    file.sync_all()
}

fn environment() -> Environment {
    let mut info = Environment {
        orev_version: env!("CARGO_PKG_VERSION"),
        os: env::consts::OS,
        distribution: BTreeMap::new(),
        kernel_release: None,
        machine: None,
        variables: BTreeMap::new(),
        system_timezone: None,
        diagnostics: Vec::new(),
    };
    // uname reports the running kernel/machine, not the Rust compilation target.
    {
        let uts = rustix::system::uname();
        info.kernel_release = Some(uts.release().to_string_lossy().into_owned());
        info.machine = Some(uts.machine().to_string_lossy().into_owned());
        match read_os_release(
            Path::new("/etc/os-release"),
            Path::new("/usr/lib/os-release"),
        ) {
            Ok(values) => info.distribution = values,
            Err(error) => info.diagnostics.push(format!("os-release: {error}")),
        }
    }
    for key in [
        "LANG",
        "LANGUAGE",
        "LC_ALL",
        "LC_CTYPE",
        "LC_NUMERIC",
        "LC_TIME",
        "LC_COLLATE",
        "LC_MONETARY",
        "LC_MESSAGES",
        "LC_PAPER",
        "LC_NAME",
        "LC_ADDRESS",
        "LC_TELEPHONE",
        "LC_MEASUREMENT",
        "LC_IDENTIFICATION",
        "TZ",
        "TERM",
    ] {
        match env::var(key) {
            Ok(value) => {
                info.variables.insert(key.to_owned(), value);
            }
            Err(env::VarError::NotPresent) => {}
            Err(error) => info.diagnostics.push(format!("{key}: {error}")),
        }
    }
    // Record the timezone database entry actually used by libc, not /etc/timezone,
    // which may be stale. A copied /etc/localtime cannot identify its IANA name.
    match fs::canonicalize("/etc/localtime") {
        Ok(path) => {
            if let Ok(zone) = path.strip_prefix("/usr/share/zoneinfo") {
                info.system_timezone = Some(zone.to_string_lossy().into_owned());
            } else {
                info.diagnostics.push(format!(
                    "system timezone name unavailable: /etc/localtime resolves to {}",
                    path.display()
                ));
            }
        }
        Err(error) => info.diagnostics.push(format!("system timezone: {error}")),
    }
    for diagnostic in &info.diagnostics {
        eprintln!("Warning: environment snapshot: {diagnostic}");
    }
    info
}

fn read_os_release(primary: &Path, fallback: &Path) -> io::Result<BTreeMap<String, String>> {
    let content = match fs::read_to_string(primary) {
        Ok(content) => content,
        Err(error) if error.kind() == io::ErrorKind::NotFound => fs::read_to_string(fallback)?,
        Err(error) => return Err(error),
    };
    let mut values = BTreeMap::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, raw)) = line.split_once('=') else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid os-release assignment",
            ));
        };
        if !matches!(
            key,
            "ID" | "VERSION_ID" | "PRETTY_NAME" | "BUILD_ID" | "VARIANT_ID"
        ) {
            continue;
        }
        let value = os_release_value(raw)?;
        values.insert(key.to_owned(), value);
    }
    if !values.contains_key("ID") {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "os-release is missing ID",
        ));
    }
    Ok(values)
}

fn os_release_value(raw: &str) -> io::Result<String> {
    let raw = raw.trim();
    let (body, double_quoted) = if let Some(body) = raw.strip_prefix('"') {
        (
            body.strip_suffix('"').ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidData, "unterminated os-release value")
            })?,
            true,
        )
    } else if let Some(body) = raw.strip_prefix('\'') {
        return body.strip_suffix('\'').map(str::to_owned).ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "unterminated os-release value")
        });
    } else {
        (raw, false)
    };
    let mut value = String::new();
    let mut chars = body.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.peek().copied() {
                Some(next) if !double_quoted || matches!(next, '$' | '`' | '"' | '\\') => {
                    value.push(chars.next().expect("peeked character"));
                }
                None => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "trailing os-release escape",
                    ));
                }
                _ => value.push(c),
            }
        } else {
            value.push(c);
        }
    }
    Ok(value)
}

fn run(
    argv: &[OsString],
    version: Option<String>,
    skip_probe: bool,
    log: &mut File,
    file: &mut File,
    metadata: &mut Metadata,
) -> io::Result<u32> {
    let executable = resolve(&argv[0])?;
    metadata.tool.executable = Some(executable.clone());
    if let Some(version) = version {
        metadata.tool.version = version;
        metadata.tool.version_source = "user".into();
        metadata.tool.version_status = "provided";
    } else if skip_probe {
        metadata.tool.version_status = "skipped";
    } else {
        metadata
            .tool
            .detect_version(&executable, || SIGNAL.load(Ordering::Relaxed) != 0);
    }
    if metadata.tool.version_status == "unknown" {
        eprintln!(
            "Warning: tool version unknown: {}",
            metadata
                .tool
                .version_diagnostic
                .as_deref()
                .unwrap_or("not detected")
        );
    }
    save(file, metadata)?;
    if SIGNAL.load(Ordering::Relaxed) != 0 {
        return Err(io::Error::new(
            io::ErrorKind::Interrupted,
            "run interrupted",
        ));
    }
    metadata.status = "running";
    save(file, metadata)?;
    terminal::execute(
        argv,
        &executable,
        &metadata.cwd,
        log,
        &metadata.username,
        &SIGNAL,
    )
}

extern "C" fn record_signal(signal: libc::c_int) {
    SIGNAL.store(signal, Ordering::Relaxed);
}

struct Signals(Vec<(libc::c_int, libc::sigaction)>);

impl Signals {
    fn install() -> io::Result<Self> {
        SIGNAL.store(0, Ordering::Relaxed);
        let mut guard = Self(Vec::new());
        for signal in [libc::SIGINT, libc::SIGQUIT, libc::SIGTERM, libc::SIGHUP] {
            // Zero-initialization and sigemptyset initialize the platform sigaction.
            let mut action: libc::sigaction = unsafe { std::mem::zeroed() };
            let mut old: libc::sigaction = unsafe { std::mem::zeroed() };
            action.sa_sigaction = record_signal as *const () as usize;
            unsafe { libc::sigemptyset(&mut action.sa_mask) };
            if unsafe { libc::sigaction(signal, &action, &mut old) } != 0 {
                return Err(io::Error::last_os_error());
            }
            guard.0.push((signal, old));
        }
        Ok(guard)
    }
}

impl Drop for Signals {
    fn drop(&mut self) {
        for (signal, action) in &self.0 {
            if unsafe { libc::sigaction(*signal, action, std::ptr::null_mut()) } != 0 {
                eprintln!(
                    "Warning: cannot restore signal handler: {}",
                    io::Error::last_os_error()
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn os_release_parser_supports_quotes_escapes_and_missing_primary() {
        let root = tempfile::tempdir().unwrap();
        let primary = root.path().join("os-release");
        let fallback = root.path().join("fallback");
        fs::write(&fallback,
                "# comment\nID=test\nVERSION_ID='1.2'\nPRETTY_NAME=\"Test \\\"OS\\\" \\\\ build\"\nHOME_URL=https://example.invalid\n").unwrap();
        let values = read_os_release(&primary, &fallback).unwrap();
        assert_eq!(values["ID"], "test");
        assert_eq!(values["VERSION_ID"], "1.2");
        assert_eq!(values["PRETTY_NAME"], "Test \"OS\" \\ build");
        assert!(!values.contains_key("HOME_URL"));
        fs::write(&primary, "ID=primary\n").unwrap();
        assert_eq!(
            read_os_release(&primary, &fallback).unwrap()["ID"],
            "primary"
        );
        fs::write(&primary, "ID=\"unterminated\n").unwrap();
        assert!(read_os_release(&primary, &fallback).is_err());
        fs::write(&primary, "VERSION_ID=1\n").unwrap();
        assert!(read_os_release(&primary, &fallback).is_err());
    }

    #[test]
    fn same_second_runs_use_suffixes_without_overwriting_existing_entries() {
        let root = tempfile::tempdir().unwrap();
        let name = "readelf-20261028T030042Z";
        let first = create_run_directory(root.path(), name).unwrap();
        fs::write(first.join("existing.log"), "preserve").unwrap();
        fs::write(root.path().join(format!("{name}-1")), "occupied").unwrap();
        let second = create_run_directory(root.path(), name).unwrap();
        assert_eq!(second, root.path().join(format!("{name}-2")));
        assert_eq!(
            fs::read_to_string(first.join("existing.log")).unwrap(),
            "preserve"
        );
        assert_eq!(
            fs::read_to_string(root.path().join(format!("{name}-1"))).unwrap(),
            "occupied"
        );
        assert_eq!(
            create_run_directory(root.path(), name).unwrap(),
            root.path().join(format!("{name}-3"))
        );
    }
}
