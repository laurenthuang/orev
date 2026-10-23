use crate::config::Config;
use std::env;
use std::fs;
use std::io;
use std::path::Path;
use std::process::Command;

// 【Rust 基础概念：返回类型 Option<String>】
// 代表这个函数可能成功拿到版本字符串（Some），也可能因为工具没安装而返回空（None）。
// 辅助函数：获取外部工具版本
fn get_tool_version(tool_name: &str, version_arg: &str) -> Option<String> {
    let output = Command::new(tool_name).arg(version_arg).output().ok()?;
    // 【Rust 基础概念：? 操作符】
    // .ok() 把 Result 转成 Option。? 的意思是：
    // 如果前面成功拿到 Some，就解包继续；如果是 None（出错/没找到），立刻短路并让 get_tool_version 返回 None。

    if output.status.success() {
        // 【Rust 基础概念：字节转字符串】
        // 命令输出的 stdout 是底层字节流 (Vec<u8>)，用 from_utf8_lossy 安全转成字符串。
        let stdout = String::from_utf8_lossy(&output.stdout);

        // lines().next()? 取出输出的第一行，拿不到则返回 None
        let first_line = stdout.lines().next()?.trim().to_string();
        Some(first_line)
    } else {
        None
    }

    // 注意：这里没有显式的 return None，因为 if-else 已经覆盖了所有情况。
}

// 辅助函数：检测 Java 版本（Ghidra 核心依赖）
fn check_java_version() -> Option<String> {
    // 注意：java 的版本信息通常输出在标准错误 (stderr) 中，而不是 stdout
    let output = Command::new("java").arg("-version").output().ok()?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    let first_line = stderr.lines().next()?.trim().to_string();
    Some(first_line)
}

// 辅助函数：检测 Ghidra
fn check_ghidra_info() -> Option<String> {
    // 【Rust 基础概念：env::var 读取环境变量】
    // 读取环境变量会返回 Result，用 if let Ok(...) 来安全地匹配成功的分支。
    if let Ok(install_dir) = env::var("GHIDRA_INSTALL_DIR") {
        let path = Path::new(&install_dir);
        let headless_path = path.join("support/analyzeHeadless");
        if headless_path.exists() {
            return Some(format!("Found via GHIDRA_INSTALL_DIR ({})", install_dir));
        }
    }

    if Command::new("analyzeHeadless").arg("-h").output().is_ok() {
        return Some("Found in PATH".to_string());
    }
    None
}

/// 核心业务函数：执行 doctor 体检
pub fn run_doctor() -> io::Result<()> {
    println!("=== Orev Doctor: Environment & Workspace Diagnostic ===\n");
    let current_dir = match env::current_dir() {
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
    let config_path = current_dir.join("orev.toml");
    let config_content = match fs::read_to_string(&config_path) {
        Ok(content) => content,
        Err(e) => {
            eprintln!("Error: Failed to read `{}`: {}", config_path.display(), e);
            return Err(e);
        }
    };

    let config: Config = match toml::from_str(&config_content) {
        Ok(config) => config,
        Err(e) => {
            eprintln!("Error: Failed to parse `{}`: {}", config_path.display(), e);
            return Err(io::Error::new(io::ErrorKind::InvalidData, e));
        }
    };

    println!("[✓] `orev.toml` configuration loaded and validated.");

    let store_root = match config.storage.resolve_store_root(&current_dir) {
        Ok(path) => path,
        Err(e) => {
            eprintln!("Error: Failed to resolve storage.store_root: {}", e);
            return Err(e);
        }
    };
    let tools_root = match config.storage.resolve_tools_root(&current_dir) {
        Ok(path) => path,
        Err(e) => {
            eprintln!("Error: Failed to resolve storage.tools_root: {}", e);
            return Err(e);
        }
    };

    // 2. 检查核心工作区目录
    let mut dirs = vec![
        current_dir.join("kb"),
        store_root,
        tools_root,
        current_dir.join("scripts"),
        current_dir.join("state"),
        current_dir.join(".orev"),
    ];

    if config.stages.iter().any(|stage| stage == "60_diff") {
        dirs.push(current_dir.join("60_diff"));
    }

    if config.stages.iter().any(|stage| stage == "90_report") {
        dirs.push(current_dir.join("90_report"));
    }

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

    // 3. 校验已注册目标的物理存在性
    println!("\n--- Target Binaries Integrity ---");
    if config.targets.files.is_empty() {
        println!("[!] No targets registered in `orev.toml`.");
    } else {
        for file_path in &config.targets.files {
            let target_abs = current_dir.join(file_path);
            if target_abs.exists() {
                println!("[✓] Target file active on disk: `{}`", file_path);
            } else {
                println!("[X] Target file missing on disk: `{}`", file_path);
            }
        }
    }

    // 4. 探查运行时依赖版本
    println!("\n--- Runtime Dependencies ---");
    match check_java_version() {
        Some(ver) => println!("[✓] Java: {}", ver),
        None => println!("[!] Java not found (Required for Ghidra)."),
    }

    // 5. 探查外部逆向分析工具链及其版本
    println!("\n--- External Toolchain & Versions ---");
    let tools = [
        ("objdump", "--version"),
        ("readelf", "--version"),
        ("strings", "--version"),
    ];

    for (tool, arg) in tools {
        match get_tool_version(tool, arg) {
            Some(ver) => println!("[✓] {}: {}", tool, ver),
            None => println!("[!] {}: Not found (optional/recommended)", tool),
        }
    }

    match check_ghid_info_wrapper() {
        Some(info) => println!("[✓] Ghidra: {}", info),
        None => println!("[!] Ghidra: Not found (set GHIDRA_INSTALL_DIR if installed)."),
    }

    println!("\nDoctor check completed.");
    Ok(())
}

fn check_ghid_info_wrapper() -> Option<String> {
    check_ghidra_info()
}
