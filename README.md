# Orev

`orev` is a modern and high-performance binary analysis project manager written in Rust, designed to provide standardized workspace management and environment health diagnostics for reverse engineering and binary security researchers.

---

## 🚀 Core Features

- **Workspace Standardization (`orev init`)**: Initialize standard reverse engineering directory structures (`samples/`, `notes/`, `patches/`, `.orev/`) with a single command, and automatically import and manage target binary files.
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

### 1. Initialize a Project
Run the initialization command in your target directory:
```bash
orev init -t /path/to/target_binary
```

### 2. Check Workspace Status
```bash
orev status
```

### 3. Run Environment and Dependency Diagnostics
```bash
orev doctor
```

---

## 📄 License

This project is dual-licensed under:

- [MIT License](LICENSE-MIT)
- [Apache License 2.0](LICENSE-APACHE)

You may choose either license to govern your use of this software.
