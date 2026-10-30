# Diagnostics

Run `status` and `doctor` from a case root, workspace root, or any of their subdirectories.
Both commands search the current directory and then its parents for the nearest `orev.toml` entry.
They stop at that entry even if it is malformed, unreadable, or a dangling symbolic link.
They do not skip it for a configuration farther up the directory tree.
Inside a workspace case, both commands use the case configuration rather than selecting the workspace.
For case creation and configuration, see [Workspaces and configuration](workspaces.md).

## Case and workspace status

```bash
orev status
```

Orev reads the nearest `orev.toml`.
The reported root is the directory containing that configuration, not the current working directory.
For a case configuration, it shows:

- Case name.
- Case version.
- Case root directory.

For example:

```text
=== Orev Case Status ===
Case Name:       router-audit
Case Version:    0.1.0
Case Root:       /srv/analysis-workspace/cases/router-audit
```

For a workspace configuration with `[workspace]`, it shows:

- Workspace root directory.
- Registered case count.
- Registered case names, in registry order.

For example:

```text
=== Orev Workspace Status ===
Workspace Root:  /srv/analysis-workspace
Registered Cases: 1
  - router-audit
```

An empty workspace shows a registered case count of 0.
Status reads the registry; it does not inspect case directories or validate their contents.
Unknown fields and malformed configurations produce errors for both configuration types.
If neither the current directory nor its parents contain `orev.toml`, status prints this hint:

```text
Please run `orev init` for a standalone case or `orev init --workspace` for a workspace.
```

The current `status` command can print a configuration error and still return exit code 0.
Do not use its exit code as proof that a configuration is valid.

## Case, workspace, and environment checks

```bash
orev doctor
```

Doctor reports the operating system and CPU architecture.
It reads and validates the case or workspace configuration.
It resolves storage roots and the shared knowledge-base path.
For a workspace case, it also validates the associated workspace configuration schema.
Explicit case storage fields override inherited workspace settings.
Relative paths use the directory of the declaring configuration.
For a case, it checks these directories:

- The case's local or shared `kb/`, `store/`, and `tools/` roots.
- Case-local `scripts/` and `state/`.

For a workspace, it checks `kb/`, `cases/`, and the configured or default `store/` and `tools/` roots.
It does not require case-local directories at the workspace root.
Doctor does not require `.orev/` in the case root, workspace root, or user home directory.
Workspace checks do not inspect registered cases.
Run doctor from a case root or one of its subdirectories to check that case's directories and configuration.

Doctor checks Java, `objdump`, `readelf`, `strings`, `file`, Magika, Detect It Easy, and Ghidra.
It distinguishes missing executables from version-detection failures.
It accepts version output on standard output or standard error.
See [Version detection](tool-runs.md#version-detection) for the shared detection rules.

### Magika discovery

Doctor searches `PATH` for `magika` and probes it with `--version`.
Google's Rust CLI package is named `magika-cli`, but its executable is named `magika`.
Missing executables and failed version probes produce optional-tool warnings.
Doctor does not classify files or download models.

### Detect It Easy discovery

Doctor searches `PATH` for the console executable `diec` and probes it with `--version`.
It does not check `die`, which starts the graphical interface.
Record Detect It Easy scans with `orev run -- diec ...`.

### Ghidra discovery

If `GHIDRA_INSTALL_DIR` is set, doctor uses `GHIDRA_INSTALL_DIR/support/analyzeHeadless`.

Otherwise, doctor searches `PATH` for `analyzeHeadless`.
An invalid explicit installation path produces a diagnostic.
Orev does not silently select a different installation.

Doctor reads the version from `Ghidra/application.properties`.
It does not start Ghidra analysis or execute its launcher.

### Warnings and errors

At local default paths, missing directories and incorrect directory types produce warnings.
Optional tool problems also produce warnings.
These warnings alone do not cause a nonzero exit status.
Configuration read or parse failures, invalid storage roots, and filesystem inspection failures return errors.
A shared knowledge-base or inherited storage root that cannot resolve to a directory also returns an error.

A completed doctor command does not prove that every tool is available or that an analysis will succeed.
Read its diagnostics, not only its exit code.
