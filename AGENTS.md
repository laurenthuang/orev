# Agent instructions

Keep user instructions in [README.md](./README.md) and [docs/user/](./docs/user/).
Link to them. Do not copy their content.

## Layout and architecture

```text
Cargo.toml          # Rust package and dependencies
src/
  main.rs           # Command dispatch
  cli.rs            # Clap command definitions
  commands/         # Command adapters and output
  ops/              # Workspace and tool-run operations
    run/            # PTY, redaction, and record integrity
  config.rs         # TOML schemas and resource roots
  tool.rs           # Shared discovery and version probes
tests/              # CLI integration tests and support/
docs/user/          # User guides
```

- Keep command adapters limited to calls and output. Put filesystem and execution logic in `src/ops/`.
- `WorkspaceConfig` separates `[case]` from `[workspace]`. Project cases use `project_root` and share project resources.
- `src/ops/case.rs` reuses `src/ops/init.rs` for new cases. Keep conversion locks, symbolic link checks, and rollback.
- `doctor` and `run` share `src/tool.rs`. Keep probe time and output limits. Read Ghidra versions from installation properties. Do not execute its launcher for version detection.

## Build, test, and lint

Run from the repository root:

```bash
cargo build
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test

# One integration-test target:
cargo test --test case

# One exact test:
cargo test --test case case_add_creates_named_shared_cases_and_finds_ancestor_project -- --exact
```

- Integration tests use `CARGO_BIN_EXE_orev` and [tests/support/mod.rs](./tests/support/mod.rs). Use temporary directories and executable shell fixtures.
- Initialization tests need Git. Tool tests set `PATH` and Ghidra paths. They do not need installed analysis tools.
- Do not add generated `Cargo.lock` to version control.

## Commit messages

Use [Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/) titles: `<type>[(scope)][!]: <description>`.

- Types: `feat`, `fix`, `docs`, `test`, `refactor`, `perf`, `build`, `ci`, `chore`, `style`, `revert`, and `agent` for skills and agent instructions (`AGENTS.md`, `.agents/`).
- Scopes are optional and name a command or module, such as `run`, `case`, or `doctor`.
- Write the description in the imperative mood and in lowercase, with no trailing period. Keep the title under 72 characters, aiming for 50.
- Mark breaking CLI or configuration changes with `!` or a `BREAKING CHANGE:` footer.

## Key conventions

- For command changes, use the [workspace](./docs/user/workspaces.md), [tool-run](./docs/user/tool-runs.md), and [diagnostic](./docs/user/diagnostics.md) guides. Update the related guide. Do not use `docs/dev/archive/` as current contracts.
- Reject unknown TOML fields. Use YAML for structured results and lists for state ledgers. Do not convert old formats automatically.
- Check configuration before initialization. Keep valid files and links. Sync temporary files in the destination directory. Install them with an operation that cannot replace another entry.
- `case add` stops at the nearest configuration, even if invalid. `status` and `doctor` read only the current directory.
- Resolve symbolic links before storage boundary checks. Require absolute storage directories that exist outside their case or project. Do not rewrite saved case storage roots.
- Preserve OS-string argument bytes. Do not insert a shell. Allow lossy conversion only in metadata.
- Use directory mode `0700` and record-file mode `0600`. Do not overwrite records or recreate deleted paths. Keep original open handles after record replacement or deletion.
- Use the effective user identifier (UID), not `USER`, for account lookup. Replace complete username path components with `<user>` before record writes. Keep redaction state across chunks. Leave live output and execution unchanged.
- Keep metadata completion steps for handled failures and signals. Return tool exit codes, or `128 + signal` for signal termination. Return 1 for record failures, even if the tool succeeds.
- Keep optional-tool warnings separate from `doctor` configuration failures. The `status` exit code 0 does not prove configuration validity.
