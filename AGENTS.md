# Agent instructions

Keep user-facing instructions in [README.md](./README.md) and [docs/user/](./docs/user/).
Link to those instructions instead of copying them here.
For command changes, update the relevant [configuration](./docs/user/workspaces.md), [tool-run](./docs/user/tool-runs.md), or [diagnostic](./docs/user/diagnostics.md) guide.
Treat `docs/dev/archive/` as historical material, not current contracts.

## Layout and architecture

Use the [glossary](./docs/glossary.md) for case and workspace terminology.
Use "repository-local" for repository customizations.
See the [architecture](./docs/dev/architecture.md) for configuration types, module dependencies, and tool recording.

- Keep command dispatch and process exit handling in [src/main.rs](./src/main.rs). Put filesystem and execution logic in [src/ops/](./src/ops/).
- Reuse [src/ops/init.rs](./src/ops/init.rs) when adding cases. Preserve conversion locks, symbolic link checks, and rollback.
- Use [src/tool.rs](./src/tool.rs) for executable discovery and version probes. Keep probe time and output limits. Read Ghidra versions from `Ghidra/application.properties`, not by executing its launcher.

## Build, test, and lint

Run from the repository root:

```bash
cargo build
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

For case-management changes, run `cargo test --test case`.

- Integration tests use `CARGO_BIN_EXE_orev` and [tests/support/mod.rs](./tests/support/mod.rs). Use temporary directories and executable shell fixtures.
- Initialization tests need Git. Tool tests set `PATH` and Ghidra paths. They do not need installed analysis tools.
- Use [CLI verification](./.agents/skills/verify-orev/SKILL.md) for real-terminal checks and [redaction verification](./.agents/skills/verify-redaction/SKILL.md) for bounded-exhaustive checks.
- Do not add generated `Cargo.lock` to version control.

## Commit messages

Use [Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/) titles: `<type>[(scope)][!]: <description>`.

- Types: `feat`, `fix`, `docs`, `test`, `refactor`, `perf`, `build`, `ci`, `chore`, `style`, `revert`, and `agent` for skills and agent instructions (`AGENTS.md`, `.agents/`).
- Scopes are optional and name a command or module, such as `run`, `case`, or `doctor`.
- Write the description in the imperative mood and in lowercase, with no trailing period. Keep the title under 72 characters, aiming for 50.
- Mark breaking CLI or configuration changes with `!` or a `BREAKING CHANGE:` footer.

## Case and workspace changes

- Reject unknown TOML fields. Use YAML for structured results and lists for state ledgers. Do not convert old formats automatically.
- Validate existing configuration before creating case resources. Preserve valid files and symbolic links. Synchronize temporary files in the destination directory. Publish files without replacing existing entries.
- `case add` stops at the nearest configuration, even if invalid. `status` and `doctor` read only the current directory.
- Resolve symbolic links before storage boundary checks. Configured external roots must be absolute, already exist, and resolve outside the configuration root that declares them. Do not rewrite saved case storage roots.
- Keep optional-tool warnings separate from `doctor` configuration failures. The `status` exit code 0 does not prove configuration validity.

## Tool execution changes

- Preserve OS-string argument bytes. Do not insert a shell. Allow lossy conversion of arguments only in metadata.
- Use directory mode `0700` and record-file mode `0600`. Do not overwrite records or recreate deleted paths. Keep original open handles after record replacement or deletion.
- Use the effective user identifier (UID), not `USER`, for account lookup. Replace complete username path components with `<user>` before record writes. Keep redaction state across chunks. Leave live output and execution unchanged.
- Keep metadata completion steps for handled failures and signals. Return tool exit codes, or `128 + signal` for signal termination. Return 1 for record failures, even if the tool succeeds.
