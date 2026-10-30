# Agent instructions

Keep this file limited to stable, repository-wide working rules.
Put feature-specific constraints and design rationale in [docs/dev/](./docs/dev/).

Keep user-facing instructions in [README.md](./README.md) and [docs/user/](./docs/user/).
Link to those instructions instead of copying them here.
For command changes, update the relevant [configuration](./docs/user/workspaces.md), [tool-run](./docs/user/tool-runs.md), or [diagnostic](./docs/user/diagnostics.md) guide.
Treat `docs/dev/archive/` as historical material, not current contracts.

## Layout and architecture

Use the [glossary](./docs/glossary.md) for case and workspace terminology.
Use "repository-local" for repository customizations.
See the [architecture](./docs/dev/architecture.md) for configuration types, module dependencies, and tool recording.
Before changing runtime code or its tests, read and follow the [behavior contracts](./docs/dev/behavior-contracts.md).

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

## Git workflow

Do not create a branch or open a pull request without the user's explicit confirmation.

## Commit messages

Use [Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/) titles: `<type>[(scope)][!]: <description>`.

- Types: `feat`, `fix`, `docs`, `test`, `refactor`, `perf`, `build`, `ci`, `chore`, `style`, `revert`, and `agent` for skills and agent instructions (`AGENTS.md`, `.agents/`).
- Scopes are optional and name a command or module, such as `run`, `case`, or `doctor`.
- Write the description in the imperative mood and in lowercase, with no trailing period. Keep the title under 72 characters, aiming for 50.
- Mark breaking CLI or configuration changes with `!` or a `BREAKING CHANGE:` footer.
