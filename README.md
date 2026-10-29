# Orev

Orev manages analysis cases for binaries, firmware, and malware.
It creates case directories, checks the local environment, and records tool runs.
Orev is written in Rust.

Orev does not isolate tools or samples.
Use a suitable isolated environment before you run tools on untrusted inputs.

## Commands

| Command | Purpose |
| --- | --- |
| `orev init [PATH]` | Create a standalone case. |
| `orev init --workspace [PATH]` | Create a project that can contain multiple cases. |
| `orev case add <NAME>` | Add a case to a project, or convert a standalone case first. |
| `orev status` | Show the case name, version, and root directory. |
| `orev doctor` | Check case directories, configuration, and external tools. |
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

1. Create the case and enter its root directory:

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
Run `status` and `doctor` from a case root for case details, or a project root for shared resources.
See [Diagnostics](docs/user/diagnostics.md) for warnings and exit-status limits.

### Create a project with multiple cases

```bash
orev init --workspace ./analysis-project
cd ./analysis-project
orev case add router-audit
cd ./cases/router-audit
```

See [Workspaces and configuration](docs/user/workspaces.md) before you convert an existing standalone case.

### Record a tool run

If `readelf` is installed and `./program` is your input file, run:

```bash
orev run -- readelf -h ./program
```

Orev creates a separate record directory in the current directory.
It contains `output.log` and `metadata.yaml`.
No case configuration is required.
See [Tool runs](docs/user/tool-runs.md) for argument handling, record fields, and privacy limits.

## Guides

| Guide | Contents |
| --- | --- |
| [Workspaces and configuration](docs/user/workspaces.md) | Initialization, case names, shared storage, conversion, and configuration. |
| [Tool runs](docs/user/tool-runs.md) | Terminal behavior, version detection, records, and recording failures. |
| [Diagnostics](docs/user/diagnostics.md) | Status output, doctor checks, warnings, and errors. |

## License

Orev is dual-licensed under the [MIT License](LICENSE-MIT) and the [Apache License 2.0](LICENSE-APACHE).
You can choose either license.
