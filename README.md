# Orev

`orev` is a modern and high-performance binary analysis project manager written in Rust, designed to provide standardized workspace management and environment health diagnostics for reverse engineering and binary security researchers.

---

## 🚀 Core Features

- **Workspace Standardization (`orev init`)**: Initialize a standalone analysis case with knowledge-base directories, state ledgers, case-level stage scaffolding, configuration, and a case guide. Target files are imported separately with `orev target add`.
- **Project Status Dashboard (`orev status`)**: Quickly inspect current project configurations and target file lists.
- **Comprehensive Environment Check (`orev doctor`)**:
  - Automatically detect host operating system and CPU architecture.
  - Validate the `orev.toml` configuration file and workspace directory integrity.
  - Verify the physical existence of managed target files on disk.
  - Probe runtime dependencies (such as the Java Runtime Environment) and external toolchain versions (such as `objdump`, `readelf`, `radare2`, and `Ghidra`).

---

## 📦 Installation & Building

Ensure that the **Rust development environment** is installed on your system (recommended via `rustup`).

1. Clone the repository and navigate to the project directory:
   ```bash
   git clone [https://github.com/laurenthuang/orev.git](https://github.com/laurenthuang/orev.git)
   cd orev
   ```

2. Build the project using Cargo:
   ```bash
   cargo build --release
   ```

The compiled binary will be located at `target/release/orev`.

---

## 🕹️ Quick Start

### 1. Initialize a Case

Create a case directory and enter it:

```bash
orev init ./my-case --name my-case
cd ./my-case
```

The path and `--name` are optional. Without a path, initialization uses the current directory.
For a new case, the name defaults to the directory name unless `--name` is supplied.
With an existing configuration, its case name is authoritative for output and newly
generated case guides. A conflicting `--name` fails without changing existing files
or creating other workspace resources; a matching name is accepted.

Initialization creates the workspace scaffolding; it does not import target files.

Initialization creates an empty `scripts/` directory for case-specific scripts.
Organize these under `scripts/<tool>/` as needed; initialization does not generate or execute scripts.

Missing files are fully written and synced in the destination directory before
publication without overwriting existing paths. Concurrent initialization reads
the configuration that was actually published; it never replaces it with a
losing process's candidate. Existing files and valid symlinks are preserved.
Staging-file cleanup is attempted on handled failures; abrupt termination may
leave temporary files. Initialization is not a whole-case transaction and does not
promise rollback or power-loss durability.

#### Configuration Support (Current PoC)

The current PoC rejects unknown configuration fields rather than silently
ignoring them. Unsupported settings such as `[sample]` and `[[diff]]`
are design references, not implemented configuration options.

A minimal standalone case configuration is:

```toml
stages = ["00_inbox", "20_static", "30_decompile", "50_findings"]

[case]
name = "my-case"
version = "0.1.0"
investigators = []

[targets]
files = []
```

Omitting `[storage]` uses case-local `store/` and `tools/` directories.
Optional `storage.store_root` and `storage.tools_root` must each point to
an existing absolute directory outside the case, after resolving symlinks.

`stages` accepts only the nine exact, case-sensitive identifiers in the
[stage contract](docs/dev/binary-analysis-workspace-layout/01-layout/03-阶段目录契约.md#42-阶段编号).
Unknown names, empty strings, and names with surrounding whitespace are rejected.
An empty list (`stages = []`) is allowed. This validation does not check
execution dependencies or stage completion; initialization does not run analysis.

### 2. Import a Target (Optional)

Run target import from the case root:

```bash
orev target add /path/to/target_binary --type binary
```

Supported target types are `binary`, `firmware`, and `sample`. The default is `binary`.

### 3. Check Case Status

Run from the case root:

```bash
orev status
```

### 4. Run Environment and Workspace Diagnostics

Run from the case root:

```bash
orev doctor
```

### Development Checks

```bash
cargo fmt -- --check
cargo test --offline --locked --bin orev --test init
cargo clippy --offline --locked --all-targets -- -D warnings
```

Module-local unit tests verify complete, no-overwrite file publication, including
write failures and racing creators. CLI integration tests cover configuration,
names, concurrent initialization, byte-for-byte preservation on reinitialization,
path conflicts, and symlinks (Unix). Git is required for ignore-rule tests;
analysis tools and target execution are not required.

---

## 📄 License

This project is dual-licensed under:

- [MIT License](LICENSE-MIT)
- [Apache License 2.0](LICENSE-APACHE)

You may choose either license to govern your use of this software.
