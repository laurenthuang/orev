use std::ffi::OsString;
use std::io;

pub fn execute(
    command: &[OsString],
    tool_version: Option<String>,
    no_version_probe: bool,
) -> io::Result<u32> {
    crate::ops::run::execute(command, tool_version, no_version_probe).inspect_err(|error| {
        eprintln!("Error: {error}");
    })
}
