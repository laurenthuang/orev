# Diagnostics

Run `status` and `doctor` from a case root or project root.
Neither command searches parent directories for a configuration.
For case creation and configuration, see [Workspaces and configuration](workspaces.md).

## Case and project status

```bash
orev status
```

Orev reads `orev.toml` in the current directory.
For a case configuration, it shows:

- Case name.
- Case version.
- Case root directory.

For a project configuration with `[workspace]`, it shows:

- Project root directory.
- Registered case count.
- Registered case names, in registry order.

An empty project shows a registered case count of 0.
Status reads the registry; it does not inspect case directories or validate their contents.
Unknown fields and malformed configurations produce errors for both configuration types.

**Exit-status limit:** The current `status` command can print a configuration error and still return exit code 0.
Do not use its exit code as proof that a configuration is valid.

## Environment and workspace checks

```bash
orev doctor
```

Doctor reports the operating system and CPU architecture.
It reads and validates the case or project configuration.
It resolves storage roots and the shared knowledge-base path.
For a case, it checks these directories:

- The case's local or shared `kb/`, `store/`, and `tools/` roots.
- Case-local `scripts/`, `state/`, and `.orev/`.

For a project, it checks `kb/`, `cases/`, and the configured or default `store/` and `tools/` roots.
It does not require case-local directories at the project root.
Project checks do not inspect registered cases.
Run doctor from each case root to check its directories and configuration.

Doctor checks Java, `objdump`, `readelf`, `strings`, and Ghidra.
It distinguishes missing executables from version-detection failures.
It accepts version output on standard output or standard error.
See [Version detection](tool-runs.md#version-detection) for the shared detection rules.

### Ghidra discovery

If `GHIDRA_INSTALL_DIR` is set, doctor uses `GHIDRA_INSTALL_DIR/support/analyzeHeadless`.

Otherwise, doctor searches `PATH` for `analyzeHeadless`.
An invalid explicit installation path produces a diagnostic.
Orev does not silently select a different installation.

Doctor reads the version from `Ghidra/application.properties`.
It does not start Ghidra analysis or execute its launcher.

### Warnings and errors

Missing expected directories, incorrect directory types, and optional tool problems produce warnings.
These warnings alone do not cause a nonzero exit status.
Configuration read or parse failures, invalid storage roots, and filesystem inspection failures return errors.
A shared knowledge-base path that cannot resolve to a directory also returns an error.

A completed doctor command does not prove that every tool is available or that an analysis will succeed.
Read its diagnostics, not only its exit code.
