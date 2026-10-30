use crate::config::RootConfig;
use crate::tool::{self, Tool};
use std::env;
use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path::PathBuf;

fn report_tool(label: &str, name: &str, program: &OsStr) {
    let executable = match tool::resolve(program) {
        Ok(path) => path,
        Err(error) => {
            println!("[!] {label}: {error}");
            return;
        }
    };
    let mut info = Tool::new(name.to_owned());
    info.detect_version(&executable, || false);
    if let Some(diagnostic) = info.version_diagnostic {
        println!(
            "[!] {label}: Found at `{}`, version unknown: {diagnostic}",
            executable.display()
        );
    } else {
        println!("[✓] {label}: {} ({})", info.version, executable.display());
    }
}

/// 核心业务函数：执行 doctor 体检
pub fn run_doctor() -> io::Result<()> {
    println!("=== Orev Doctor: Environment & Configuration Diagnostic ===\n");
    let config_root = match env::current_dir() {
        Ok(dir) => dir,
        Err(e) => {
            eprintln!("Error: Failed to get current directory: {}", e);
            return Err(e);
        }
    };

    // 💡标准库内置常量
    // std::env::consts::OS 和 ARCH 在编译期就已经被硬编码，零运行时开销。
    println!("--- System Environment ---");
    println!("Operating System: {}", std::env::consts::OS);
    println!("CPU Architecture: {}", std::env::consts::ARCH);

    // 1. 读取并校验配置
    let config_path = config_root.join("orev.toml");
    let config_content = match fs::read_to_string(&config_path) {
        Ok(content) => content,
        Err(e) => {
            eprintln!("Error: Failed to read `{}`: {}", config_path.display(), e);
            return Err(e);
        }
    };

    let config = match RootConfig::parse(&config_content) {
        Ok(config) => config,
        Err(e) => {
            eprintln!("Error: Failed to parse `{}`: {}", config_path.display(), e);
            return Err(io::Error::new(io::ErrorKind::InvalidData, e));
        }
    };

    println!("[✓] `orev.toml` configuration loaded and validated.");

    let resources = config
        .resolve_resources(&config_root)
        .inspect_err(|error| eprintln!("Error: {error}"))?;

    // 2. 检查案件或工作区目录
    let dirs = match &config {
        RootConfig::Case(_) => {
            vec![
                resources.kb.path().to_path_buf(),
                resources.store.path().to_path_buf(),
                resources.tools.path().to_path_buf(),
                config_root.join("scripts"),
                config_root.join("state"),
                config_root.join(".orev"),
            ]
        }
        RootConfig::Workspace(_) => vec![
            resources.kb.path().to_path_buf(),
            resources.store.path().to_path_buf(),
            resources.tools.path().to_path_buf(),
            config_root.join("cases"),
        ],
    };

    for path in dirs {
        match fs::metadata(&path) {
            Ok(metadata) if metadata.is_dir() => {
                println!("[✓] Directory `{}` exists.", path.display());
            }
            Ok(_) => {
                println!("[!] Expected a directory at `{}`.", path.display());
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                println!("[!] Directory `{}` is missing.", path.display());
            }
            Err(e) => {
                eprintln!(
                    "Error: Failed to inspect directory `{}`: {}",
                    path.display(),
                    e
                );
                return Err(e);
            }
        }
    }

    // 3. 探查运行时依赖版本
    println!("\n--- Runtime Dependencies ---");
    report_tool("Java", "java", OsStr::new("java"));

    // 4. 探查外部逆向分析工具链及其版本
    println!("\n--- External Toolchain & Versions ---");
    for name in ["objdump", "readelf", "strings", "file", "magika", "diec"] {
        report_tool(name, name, OsStr::new(name));
    }

    let headless = "analyzeHeadless";
    let ghidra = env::var_os("GHIDRA_INSTALL_DIR")
        .map(|root| PathBuf::from(root).join("support").join(headless))
        .unwrap_or_else(|| PathBuf::from(headless));
    report_tool("Ghidra", "analyzeHeadless", ghidra.as_os_str());

    println!("\nDoctor check completed.");
    Ok(())
}
