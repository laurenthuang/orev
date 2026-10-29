# Behavior contracts

These constraints guide runtime implementation and regression tests.
See the [architecture](architecture.md) for configuration types, module dependencies, and resource resolution.
Command usage is documented in [Workspaces and configuration](../user/workspaces.md), [Tool runs](../user/tool-runs.md), and [Diagnostics](../user/diagnostics.md).

## Module boundaries

- Keep command dispatch and process exit handling in [src/main.rs](../../src/main.rs). Put filesystem and execution logic in [src/ops/](../../src/ops/).
- Reuse [src/ops/init.rs](../../src/ops/init.rs) when adding cases. Preserve conversion locks, symbolic link checks, and rollback.
- Use [src/tool.rs](../../src/tool.rs) for executable discovery and version probes. Keep probe time and output limits. Read Ghidra versions from `Ghidra/application.properties`, not by executing its launcher.

## Cases and workspaces

- Reject unknown TOML fields. Use YAML for structured results and lists for state ledgers. Do not convert old formats automatically.
- Validate existing configuration before creating case resources. Preserve valid files and symbolic links. Synchronize temporary files in the destination directory. Publish files without replacing existing entries.
- `case add` stops at the nearest configuration, even if invalid. `status` and `doctor` read only the current directory.
- Resolve relative storage overrides from the declaring configuration root. Configured roots must already exist and resolve outside that root after symbolic link resolution. Absolute overrides remain supported. Do not silently rewrite saved case overrides.
- Workspace cases inherit unset storage fields. Do not serialize inherited roots into case configurations.
- Keep optional-tool warnings separate from `doctor` configuration failures. The `status` exit code 0 does not prove configuration validity.

## Tool execution

- Preserve OS-string argument bytes. Do not insert a shell. Allow lossy conversion of arguments only in metadata.
- Use directory mode `0700` and record-file mode `0600`. Do not overwrite records or recreate deleted paths. Keep original open handles after record replacement or deletion.
- Use the effective user identifier (UID), not `USER`, for account lookup. Replace complete username path components with `<user>` before record writes. Keep redaction state across chunks. Leave live output and execution unchanged.
- Keep metadata completion steps for handled failures and signals. Return tool exit codes, or `128 + signal` for signal termination. Return 1 for record failures, even if the tool succeeds.
