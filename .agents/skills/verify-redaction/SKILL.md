---
name: verify-redaction
description: Bounded-exhaustive verification that orev's username-path redaction (`UsernameRedactor` in src/ops/run/redact.rs) writes exactly what the documented rule says, however the PTY splits the output into chunks. Use before and after any change to src/ops/run/redact.rs, to the redaction rules in docs/user/tool-runs.md, or to how src/ops/run/terminal.rs feeds bytes to the redactor.
---

# Verify username-path redaction

**Property.** For every byte stream and every way it is cut into `write` calls, `UsernameRedactor` produces exactly the output of the documented rule. The rule splits the stream at boundary bytes. A token equal to the account name becomes `<user>` when it is followed by `/`, or when it is preceded by `/` and followed by a boundary or the end of the stream.

**Harm if broken.** `orev run` records (`output.log`, `metadata.yaml`) would leak the analyst's account name in paths, or would corrupt unrelated text. The PTY splits output at arbitrary points, so a chunk-boundary bug appears only on some runs.

## Run it

```bash
cargo test --release --bin orev redact_bounded
```

It takes about 5 s plus the build. All three tests must say `ok`. Plain `cargo test` also runs them with smaller bounds (about 1 s), so CI catches gross breaks. Use the release command for the full bound. Optional overrides: `OREV_REDACT_LEN`, `OREV_REDACT_ALL_CHUNKINGS`, and `OREV_REDACT_CTX`.

The tool is a bounded-exhaustive Rust test on the real type, in [src/ops/run/redact_bounded.rs](../../../src/ops/run/redact_bounded.rs). Kani (a model checker for Rust) would do the same with symbolic bytes, but it is not installed here and adds a large nightly toolchain. This check needs only `cargo`, so anyone who can build orev can run it.

## What is checked (scope and bounds)

| Test | Checks | Bound (release / debug) |
| --- | --- | --- |
| `redaction_matches_spec_for_every_bounded_input_and_chunking` | Streamed output equals `spec` for usernames `ab`, `aa`, and `a`, over alphabet `a b / ␠ x`. | Every string up to length 8 / 6. Every chunking up to length 6 / 5. Longer strings get every single split point plus one byte per chunk. |
| `redaction_matches_spec_for_every_byte_value_in_context` | Each of the 256 byte values is classified as `spec_boundary` says (`/` separator, other boundary, or ordinary), so the small alphabet above is representative. | Byte 0..=255 between every prefix and suffix up to length 3 / 2, whole and split at the byte. |
| `spec_matches_documented_examples` | `spec` reproduces the table in [docs/user/tool-runs.md](../../../docs/user/tool-runs.md#username-path-redaction). | Fixed examples. |

`spec` and `spec_boundary` are written independently of the redactor's state machine (`pending`, `component_start`, `path`). They are the reference, so keep them simple.

**Not covered:**

- `current_username` (the account lookup).
- That every record write goes through a redactor. That wiring is the `UsernameRedactor` in `src/ops/run/terminal.rs` (`relay` and the `finish` call after it) and `redact_value` in `src/ops/run.rs`. Prove the wiring end to end with the `verify-orev` skill's `run-redact` recipe.
- Inputs longer than the bounds, and usernames longer than 2 bytes. The redactor's state is bounded by the username length and does not backtrack, so short names exercise every transition. That is an argument, not a proof.
- UTF-8 preservation in `redact_value`.

## Does my change touch the verified code?

Covered code:

- `src/ops/run/redact.rs`: `UsernameRedactor::{new, write, finish}`, `is_separator`, `is_boundary`, and the string branch of `redact_value`.
- `src/ops/run/redact_bounded.rs`: the check itself.
- The redaction section of `docs/user/tool-runs.md`, which states the rule.

**Sync point.** Last confirmed that the verification describes the code at commit `a3432da`.

Check for drift with `git diff a3432da -- src/ops/run/redact.rs docs/user/tool-runs.md`. Empty output means the verification still describes the code. Move the sync point forward only after you have confirmed that `spec` still states the intended rule for the new code and the release command passes.

## Reading a failure

A failure prints `user`, `chunks`, `expected`, and `actual`. Read the chunks as consecutive PTY reads. Example: `chunks: ["\0/a", "b"]`, `expected: "\0/<user>"`, `actual: "\0/ab"` means the name was split across two reads and leaked into the log.

Decide which side is wrong:

1. **Real bug.** The `expected` output matches the documented rule (`docs/user/tool-runs.md`). Fix `redact.rs`, not the check.
2. **Intended rule change.** The rule itself is meant to change, for example a new boundary byte. Update the guide first, then `spec`/`spec_boundary`, then the code. Rerun, and confirm the property can still fail (next section).
3. **Modeling error.** `spec` disagrees with the guide. Fix `spec` and confirm `spec_matches_documented_examples` covers the case.

Never lower the bounds, remove a username, or shrink the alphabet to make a failure go away.

## Confirming the check can still fail

After editing the check or the spec, break the real code on purpose. Run the release command, which must fail, then restore the file. Use `cp src/ops/run/redact.rs /tmp/redact.rs.orig` first and `cp /tmp/redact.rs.orig src/ops/run/redact.rs` after. These five bugs were each caught when the check was created:

- In `write`, replace `(is_separator(byte) || (self.path && is_boundary(byte)))` with `is_boundary(byte)`. A standalone name gets redacted.
- Delete `self.path = false;`. Path context leaks across a non-`/` boundary.
- In `finish`, drop `self.path &&`. A standalone name at the end of the stream gets redacted.
- Flush `pending` at the end of each `write`. A name split across reads leaks.
- Remove `| b'='` from `is_boundary`. This is caught only by the per-byte test, which shows that test is needed.

Restore, then confirm with `git diff -- src/ops/run/redact.rs` that only your intended change remains.
