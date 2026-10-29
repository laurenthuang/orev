//! Bounded-exhaustive check of `UsernameRedactor` against an independent spec.
//! See `.agents/skills/verify-redaction/SKILL.md` for scope, bounds, and how to run it.

use super::redact::UsernameRedactor;

/// Boundary bytes as documented for username-path redaction (docs/user/tool-runs.md).
fn spec_boundary(byte: u8) -> bool {
    byte.is_ascii_whitespace() || byte.is_ascii_control() || b"/'\":;,()[]{}<>?#=".contains(&byte)
}

/// Split the stream at boundary bytes. A token equal to the account name becomes
/// `<user>` when it is followed by `/`, or when it is preceded by `/` and followed
/// by any boundary or the end of the stream.
fn spec(input: &[u8], user: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut prev: Option<u8> = None;
    let mut start = 0;
    loop {
        let end = input[start..]
            .iter()
            .position(|b| spec_boundary(*b))
            .map_or(input.len(), |i| start + i);
        let token = &input[start..end];
        let next = input.get(end).copied();
        if token == user && (next == Some(b'/') || prev == Some(b'/')) {
            out.extend_from_slice(b"<user>");
        } else {
            out.extend_from_slice(token);
        }
        match next {
            Some(byte) => {
                out.push(byte);
                prev = Some(byte);
                start = end + 1;
            }
            None => return out,
        }
    }
}

fn streamed(user: &[u8], chunks: &[&[u8]]) -> Vec<u8> {
    let mut redactor = UsernameRedactor::new(user);
    let mut out = Vec::new();
    for chunk in chunks {
        redactor.write(chunk, &mut out).unwrap();
    }
    redactor.finish(&mut out).unwrap();
    out
}

/// Full bounds under `--release`; smaller ones keep plain `cargo test` fast.
fn bound(var: &str, release: usize, debug: usize) -> usize {
    std::env::var(var)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(if cfg!(debug_assertions) {
            debug
        } else {
            release
        })
}

fn for_each_string(alphabet: &[u8], max_len: usize, mut f: impl FnMut(&[u8])) {
    let mut buf = Vec::with_capacity(max_len);
    fn go(alphabet: &[u8], max_len: usize, buf: &mut Vec<u8>, f: &mut impl FnMut(&[u8])) {
        f(buf);
        if buf.len() == max_len {
            return;
        }
        for &byte in alphabet {
            buf.push(byte);
            go(alphabet, max_len, buf, f);
            buf.pop();
        }
    }
    go(alphabet, max_len, &mut buf, &mut f);
}

fn check(user: &[u8], chunks: &[&[u8]], expected: &[u8]) {
    let actual = streamed(user, chunks);
    assert!(
        actual == expected,
        "redaction diverges from spec\n  user:     {:?}\n  chunks:   {:?}\n  expected: {:?}\n  actual:   {:?}",
        String::from_utf8_lossy(user),
        chunks
            .iter()
            .map(|c| String::from_utf8_lossy(c))
            .collect::<Vec<_>>(),
        String::from_utf8_lossy(expected),
        String::from_utf8_lossy(&actual),
    );
}

// Usernames cover distinct letters, a repeated letter, and a single letter.
// The alphabet has the name's letters, `/`, one non-separator boundary, and one
// ordinary byte: every class the redactor branches on.
const USERS: &[&[u8]] = &[b"ab", b"aa", b"a"];
const ALPHABET: &[u8] = b"ab/ x";

/// Every chunking is exponential, so inputs longer than `OREV_REDACT_ALL_CHUNKINGS`
/// get every single split point plus one byte per chunk instead.
#[test]
fn redaction_matches_spec_for_every_bounded_input_and_chunking() {
    let max_len = bound("OREV_REDACT_LEN", 8, 6);
    let all_chunkings = bound("OREV_REDACT_ALL_CHUNKINGS", 6, 5);
    for user in USERS {
        for_each_string(ALPHABET, max_len, |input| {
            let expected = spec(input, user);
            if input.len() <= all_chunkings {
                let gaps = input.len().saturating_sub(1);
                for mask in 0u32..(1 << gaps) {
                    let mut chunks = Vec::new();
                    let mut start = 0;
                    for gap in 0..gaps {
                        if mask & (1 << gap) != 0 {
                            chunks.push(&input[start..=gap]);
                            start = gap + 1;
                        }
                    }
                    chunks.push(&input[start..]);
                    check(user, &chunks, &expected);
                }
            } else {
                for split in 0..=input.len() {
                    check(user, &[&input[..split], &input[split..]], &expected);
                }
                let bytes: Vec<&[u8]> = input.chunks(1).collect();
                check(user, &bytes, &expected);
            }
        });
    }
}

/// Justifies the small `ALPHABET`: every other byte value must act like one of its classes.
#[test]
fn redaction_matches_spec_for_every_byte_value_in_context() {
    let ctx = bound("OREV_REDACT_CTX", 3, 2);
    let user: &[u8] = b"ab";
    let mut contexts = Vec::new();
    for_each_string(ALPHABET, ctx, |s| contexts.push(s.to_vec()));
    for byte in 0..=255u8 {
        for prefix in &contexts {
            for suffix in &contexts {
                let mut input = prefix.clone();
                input.push(byte);
                input.extend_from_slice(suffix);
                let expected = spec(&input, user);
                check(user, &[&input], &expected);
                let at = prefix.len();
                check(user, &[&input[..at], &input[at..]], &expected);
            }
        }
    }
}

#[test]
fn spec_matches_documented_examples() {
    for (input, expected) in [
        (&b"/srv/alice/data"[..], &b"/srv/<user>/data"[..]),
        (b"alice/output", b"<user>/output"),
        (b"/srv/alice-tools", b"/srv/alice-tools"),
        (b"/home/bob", b"/home/bob"),
        (b"alice", b"alice"),
        (b"/srv/alice", b"/srv/<user>"),
    ] {
        assert_eq!(spec(input, b"alice"), expected);
    }
}
