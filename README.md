# Orev

Orev manages analysis cases for binaries, firmware, and malware.
It creates case directories, checks the local environment, and records tool runs.
Orev is written in Rust.

See the [glossary](docs/glossary.md) for case, case root, workspace, and workspace root.

Orev does not isolate tools or samples.
Use a suitable isolated environment before you run tools on untrusted inputs.

## Commands

| Command | Purpose |
| --- | --- |
| `orev init [PATH]` | Create a standalone case. |
| `orev init --workspace [PATH]` | Create a workspace that can contain multiple cases. |
| `orev case add <NAME>` | Add a case to a workspace, or convert a standalone case first. |
| `orev status` | Show case details or the workspace's case registry. |
| `orev doctor` | Check case or workspace directories, configuration, and external tools. |
| `orev run [OPTIONS] -- <TOOL> [ARGS]...` | Run a tool and record its terminal output and execution details. |

Orev supports Linux only.

## Installation

Install a current stable Rust toolchain, for example with [rustup](https://rustup.rs/).
Install Git before you clone the repository.

1. Get the source:

   ```bash
   git clone https://github.com/laurenthuang/orev.git
   cd orev
   ```

2. Install Orev:

   ```bash
   cargo install --path .
   ```

3. Make sure that Cargo's binary directory is on your `PATH`.

   The default directory is `$HOME/.cargo/bin`.

## Quick start

### Create a standalone case

1. Create the case and enter its case root:

   ```bash
   orev init ./my-case
   cd ./my-case
   ```

2. Read the case status and check the environment:

   ```bash
   orev status
   orev doctor
   ```

Initialization creates directories and guide files.
It does not run analysis tools.
Run `status` and `doctor` from a case root for case details, or a workspace root for shared resources.
See [Diagnostics](docs/user/diagnostics.md) for warnings and exit-status limits.

### Create a workspace with multiple cases

```bash
orev init --workspace ./analysis-workspace
cd ./analysis-workspace
orev case add router-audit
cd ./cases/router-audit
```

Cases inherit shared resources through `workspace_root` without copying storage paths into each case configuration.
See [Workspaces and configuration](docs/user/workspaces.md) before you convert an existing standalone case.

### Record a tool run

If `readelf` is installed and `./program` is your input file, run:

```bash
orev run -- readelf -h ./program
```

Orev creates a separate record directory in the current directory.
It contains `output.txt` and `metadata.yaml`.
No case configuration is required.
See [Tool runs](docs/user/tool-runs.md) for argument handling, record fields, and privacy limits.

## Guides

| Guide | Contents |
| --- | --- |
| [Glossary](docs/glossary.md) | Case, case root, workspace, workspace root, and generic configuration roots. |
| [Workspaces and configuration](docs/user/workspaces.md) | Initialization, case names, shared storage, conversion, and configuration. |
| [Tool runs](docs/user/tool-runs.md) | Terminal behavior, version detection, records, and recording failures. |
| [Diagnostics](docs/user/diagnostics.md) | Status output, doctor checks, warnings, and errors. |

## Development

See the [architecture diagrams](docs/dev/architecture.md) for module dependencies, shared case resources, and tool recording.
Build and test commands are in [AGENTS.md](AGENTS.md#build-test-and-lint).

## License

Orev is dual-licensed under the [MIT License](LICENSE-MIT) and the [Apache License 2.0](LICENSE-APACHE).
You can choose either license.
