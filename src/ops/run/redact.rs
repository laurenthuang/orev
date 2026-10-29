use std::ffi::CStr;
use std::io::{self, Write};

pub(super) fn current_username() -> io::Result<Vec<u8>> {
    let uid = unsafe { libc::geteuid() };
    let mut buffer = vec![0; 1024];
    loop {
        let mut entry: libc::passwd = unsafe { std::mem::zeroed() };
        let mut result = std::ptr::null_mut();
        let error = unsafe {
            libc::getpwuid_r(
                uid,
                &mut entry,
                buffer.as_mut_ptr().cast(),
                buffer.len(),
                &mut result,
            )
        };
        if error == libc::ERANGE && buffer.len() < 1024 * 1024 {
            buffer.resize(buffer.len() * 2, 0);
            continue;
        }
        if error != 0 {
            return Err(io::Error::from_raw_os_error(error));
        }
        if result.is_null() || entry.pw_name.is_null() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "cannot identify the current effective UID account for path redaction",
            ));
        }
        let name = unsafe { CStr::from_ptr(entry.pw_name) }.to_bytes();
        if name.is_empty() || name.iter().any(|byte| is_boundary(*byte)) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "current account name cannot be safely matched as a path component",
            ));
        }
        return Ok(name.to_vec());
    }
}

pub(super) struct UsernameRedactor {
    username: Vec<u8>,
    pending: Vec<u8>,
    component_start: bool,
    path: bool,
}

impl UsernameRedactor {
    pub(super) fn new(username: &[u8]) -> Self {
        assert!(!username.is_empty());
        Self {
            username: username.to_vec(),
            pending: Vec::new(),
            component_start: true,
            path: false,
        }
    }

    pub(super) fn write(&mut self, input: &[u8], output: &mut impl Write) -> io::Result<()> {
        let mut redacted = Vec::with_capacity(input.len());
        for &byte in input {
            if self.component_start {
                if self.pending.len() < self.username.len()
                    && byte == self.username[self.pending.len()]
                {
                    self.pending.push(byte);
                    continue;
                }
                if self.pending.len() == self.username.len()
                    && (is_separator(byte) || (self.path && is_boundary(byte)))
                {
                    redacted.extend_from_slice(b"<user>");
                } else {
                    redacted.extend_from_slice(&self.pending);
                }
                self.pending.clear();
                self.component_start = false;
            }
            redacted.push(byte);
            if is_separator(byte) {
                self.component_start = true;
                self.path = true;
            } else if is_boundary(byte) {
                self.component_start = true;
                self.path = false;
            }
        }
        output.write_all(&redacted)
    }

    pub(super) fn finish(&mut self, output: &mut impl Write) -> io::Result<()> {
        if self.path && self.pending.len() == self.username.len() {
            output.write_all(b"<user>")?;
        } else {
            output.write_all(&self.pending)?;
        }
        self.pending.clear();
        output.flush()
    }
}

fn is_separator(byte: u8) -> bool {
    byte == b'/'
}

fn is_boundary(byte: u8) -> bool {
    byte.is_ascii_whitespace()
        || byte.is_ascii_control()
        || matches!(
            byte,
            b'/' | b'\''
                | b'"'
                | b':'
                | b';'
                | b','
                | b'('
                | b')'
                | b'['
                | b']'
                | b'{'
                | b'}'
                | b'<'
                | b'>'
                | b'?'
                | b'#'
                | b'='
        )
}

pub(super) fn redact_value(value: &mut serde_yaml_ng::Value, username: &[u8]) {
    use serde_yaml_ng::Value;
    match value {
        Value::String(text) => {
            let mut output = Vec::new();
            let mut redactor = UsernameRedactor::new(username);
            // Vec's Write implementation is infallible.
            redactor
                .write(text.as_bytes(), &mut output)
                .expect("write to Vec");
            redactor.finish(&mut output).expect("write to Vec");
            *text = String::from_utf8(output).expect("redaction preserves UTF-8");
        }
        Value::Sequence(items) => items
            .iter_mut()
            .for_each(|item| redact_value(item, username)),
        Value::Mapping(items) => items
            .values_mut()
            .for_each(|item| redact_value(item, username)),
        Value::Tagged(tagged) => redact_value(&mut tagged.value, username),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_current_username_components_at_every_chunk_boundary() {
        let input = b"/home/alice/x /srv/alice /mnt/alice/data alice/output \
            alice alice-tools /srv/alice-tools/x /srv/notalice/x /home/bob/x \
            '/opt/alice' \x1b[31m/tmp/alice/y\x1b[0m /tmp/\xff/x /alice/alice";
        let expected = b"/home/<user>/x /srv/<user> /mnt/<user>/data <user>/output \
            alice alice-tools /srv/alice-tools/x /srv/notalice/x /home/bob/x \
            '/opt/<user>' \x1b[31m/tmp/<user>/y\x1b[0m /tmp/\xff/x /<user>/<user>";
        for split in 0..=input.len() {
            let mut redactor = UsernameRedactor::new(b"alice");
            let mut output = Vec::new();
            redactor.write(&input[..split], &mut output).unwrap();
            redactor.write(&input[split..], &mut output).unwrap();
            redactor.finish(&mut output).unwrap();
            assert_eq!(output, expected, "split {split}");
        }
        let mut redactor = UsernameRedactor::new(b"alice");
        let mut output = Vec::new();
        for byte in input {
            redactor.write(&[*byte], &mut output).unwrap();
        }
        redactor.finish(&mut output).unwrap();
        assert_eq!(output, expected);
    }

    #[test]
    fn path_matching_is_case_sensitive_and_uses_slashes() {
        let input = b"/home/Alice/x /home/alice/x alice\\output /home/alice\\output";
        let expected = b"/home/Alice/x /home/<user>/x alice\\output /home/alice\\output";
        for split in 0..=input.len() {
            let mut redactor = UsernameRedactor::new(b"alice");
            let mut output = Vec::new();
            redactor.write(&input[..split], &mut output).unwrap();
            redactor.write(&input[split..], &mut output).unwrap();
            redactor.finish(&mut output).unwrap();
            assert_eq!(output, expected, "split {split}");
        }
    }

    #[test]
    fn identifies_account_from_effective_uid() {
        let username = current_username().unwrap();
        assert!(!username.is_empty());
        assert!(!username.contains(&b'/'));
    }
}
