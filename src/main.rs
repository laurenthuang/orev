// ==========================================================
// 【Rust 基础概念：外部依赖导入（Crates）】
// 在 Cargo.toml 中配置好依赖后，这里使用 use 关键字把库引入当前作用域。
// ==========================================================
use clap::{Parser, Subcommand};
use serde::Deserialize;
use std::env;
use std::fs;
use std::path::Path;
use std::process::Command;

// ==========================================================
// 1. CLI 命令行参数定义
// ==========================================================

// 【Rust 基础概念：属性宏 #[derive(...)]】
// 宏（Macro）是 Rust 编译时的代码生成器。#[derive(Parser)] 告诉编译器：
// “请自动为这个结构体生成解析命令行参数的底层代码，我不用自己写样板代码。”
#[derive(Parser)]
#[command(
    name = "orev",
    version = "0.1.0",
    about = "A modern binary analysis project manager"
)]
struct Cli {
    // 嵌套子命令
    #[command(subcommand)]
    command: Commands,
}

// 【Rust 基础概念：枚举（Enum）】
// Enum 可以定义一组互斥的变体（Variant），非常适合用来做 CLI 的子命令设计。
#[derive(Subcommand)]
enum Commands {
    /// 初始化一个新的二进制分析工作区
    Init {
        // 【Rust 基础概念：Option<T>】
        // Rust 没有 null。如果某个参数是可选的（用户可能传，也可能不传），
        // 必须用 Option 包裹。它有两个值：Some(T) 代表有值，None 代表没有值。
        name: Option<String>,

        // 目标文件路径列表（Vec<String> 类似于动态数组）
        #[arg(short, long)]
        target: Vec<String>,
    },
    /// 显示当前工作区的状态和配置信息
    Status,
    /// 对工作区完整性、系统环境以及外部逆向工具链进行健康诊断
    Doctor,
}

// ==========================================================
// 2. 配置文件对应的 Rust 结构体
// ==========================================================

// 【Rust 基础概念：Deserialize 自动反序列化】
// 加上 #[derive(Deserialize)] 后，serde 库会帮我们把纯文本的 TOML 文件
// 自动转换成内存中带类型的 Rust 结构体。Debug 允许我们打印结构体。
#[derive(Deserialize, Debug)]
struct Config {
    project: ProjectConfig,
    targets: TargetsConfig,
}

#[derive(Deserialize, Debug)]
struct ProjectConfig {
    name: String,
    version: String,
}

#[derive(Deserialize, Debug)]
struct TargetsConfig {
    files: Vec<String>,
}

// ==========================================================
// 3. 辅助函数：系统环境与工具链探查
// ==========================================================

// 获取常规外部工具的版本信息
// 【Rust 基础概念：返回类型 Option<String>】
// 代表这个函数可能成功拿到版本字符串（Some），也可能因为工具没安装而返回空（None）。
fn get_tool_version(tool_name: &str, version_arg: &str) -> Option<String> {
    // 【Rust 基础概念：Command 进程调用】
    // 使用标准库的 Command 安全地在操作系统后台执行外部命令。
    let output = Command::new(tool_name).arg(version_arg).output().ok()?; // 【Rust 基础概念：? 操作符】
    // .ok() 把 Result 转成 Option。? 的意思是：
    // 如果前面成功拿到 Some，就解包继续；如果是 None（出错/没找到），立刻短路并返回 None。

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
}

// 专门检测 Java 运行环境版本（Ghidra 核心依赖）
fn check_java_version() -> Option<String> {
    // 注意：java 的版本信息通常输出在标准错误 (stderr) 中，而不是 stdout
    let output = Command::new("java").arg("-version").output().ok()?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    let first_line = stderr.lines().next()?.trim().to_string();
    Some(first_line)
}

// 检测 Ghidra 是否就位
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

// ==========================================================
// 4. 主函数与各子命令业务逻辑
// ==========================================================

fn main() {
    // 解析命令行输入
    let cli = Cli::parse();

    // 【Rust 基础概念：match 模式匹配】
    // Rust 的 match 非常严格，必须穷尽（Cover）枚举的所有可能情况（分支），
    // 少写一个都会导致编译不通过，从而彻底避免漏掉分支的 Bug。
    match cli.command {
        // --------------------------------------------------
        // 子命令：init
        // --------------------------------------------------
        Commands::Init { name, target } => {
            let current_dir = env::current_dir().expect("Failed to get current directory");

            // 【Rust 基础概念：闭包与 unwrap_or_else】
            // 如果 name 是 None，就执行闭包(|| ...) 里的备用逻辑（获取当前文件夹名）。
            let project_name = name.unwrap_or_else(|| {
                current_dir
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("binary_project")
                    .to_string()
            });

            println!("Initializing binary analysis project: `{}`", project_name);

            // 创建标准逆向工作区目录
            let dirs = ["samples", "notes", "patches", ".orev"];
            for dir in &dirs {
                let path = current_dir.join(dir);
                if !path.exists() {
                    fs::create_dir_all(&path).expect("Failed to create directory");
                    println!("  [created] {}/", dir);
                } else {
                    println!("  [exists]  {}/", dir);
                }
            }

            // 导入并复制目标二进制文件到 samples/ 目录
            let mut copied_targets = Vec::new();
            let samples_dir = current_dir.join("samples");

            // 【Rust 基础概念：& 借用（Borrowing）】
            // &target 表示对 target 数组进行“只读借用”，不抢夺它的所有权，
            // 这样循环结束后 target 变量依然可以继续使用。
            for src_str in &target {
                let src_path = Path::new(src_str);
                if src_path.exists() {
                    if let Some(file_name) = src_path.file_name() {
                        let dest_path = samples_dir.join(file_name);
                        fs::copy(src_path, &dest_path).expect("Failed to copy target binary");

                        let rel_path = format!("samples/{}", file_name.to_string_lossy());
                        println!("  [imported] {} -> {}", src_str, rel_path);
                        copied_targets.push(rel_path);
                    }
                } else {
                    eprintln!("  [warning] Target file not found, skipped: {}", src_str);
                }
            }

            if copied_targets.is_empty() {
                copied_targets.push("samples/target_binary".to_string());
            }

            // 生成默认的 orev.toml 配置文件
            let config_path = current_dir.join("orev.toml");
            if !config_path.exists() {
                let targets_toml_array = copied_targets
                    .iter()
                    .map(|t| format!("    \"{}\",", t))
                    .collect::<Vec<_>>()
                    .join("\n");

                let config_content = format!(
                    r#"[project]
name = "{}"
version = "0.1.0"
authors = []

[workspace]
notes_dir = "notes"
patches_dir = "patches"
scripts_dir = "scripts"

[targets]
files = [
{}
]
"#,
                    project_name, targets_toml_array
                );

                fs::write(&config_path, config_content).expect("Failed to write orev.toml");
                println!("  [created] orev.toml");
            } else {
                println!("  [exists]  orev.toml");
            }

            println!("\nSuccess! Workspace is ready.");
        }

        // --------------------------------------------------
        // 子命令：status
        // --------------------------------------------------
        Commands::Status => {
            let current_dir = env::current_dir().expect("Failed to get current directory");
            let config_path = current_dir.join("orev.toml");

            if !config_path.exists() {
                println!("Error: `orev.toml` not found in current directory.");
                println!("Please run `orev init` first.");
                return;
            }

            // 读取并反序列化配置文件
            let config_content =
                fs::read_to_string(&config_path).expect("Failed to read orev.toml");

            // 使用 toml 库将文本反序列化为 Config 结构体
            let config: Config =
                toml::from_str(&config_content).expect("Failed to parse orev.toml format");

            println!("=== Orev Workspace Status ===");
            println!("Project Name:    {}", config.project.name);
            println!("Project Version: {}", config.project.version);
            println!("Managed Targets:");
            for file in &config.targets.files {
                println!("  - {}", file);
            }
        }

        // --------------------------------------------------
        // 子命令：doctor
        // --------------------------------------------------
        Commands::Doctor => {
            println!("=== Orev Doctor: Environment & Workspace Diagnostic ===\n");
            let current_dir = env::current_dir().expect("Failed to get current directory");

            // 【Rust 基础概念：标准库内置常量】
            // std::env::consts::OS 和 ARCH 在编译期就已经被硬编码，零运行时开销。
            println!("--- System Environment ---");
            println!("Operating System: {}", std::env::consts::OS);
            println!("CPU Architecture: {}", std::env::consts::ARCH);

            // 1. 检查 orev.toml 配置文件
            let config_path = current_dir.join("orev.toml");
            if config_path.exists() {
                println!("[✓] `orev.toml` configuration found.");
            } else {
                println!("[X] `orev.toml` missing! (Run `orev init` to initialize)");
                return;
            }

            // 2. 检查核心工作区目录完整性
            let dirs = ["samples", "notes", "patches", ".orev"];
            for dir in &dirs {
                let path = current_dir.join(dir);
                if path.exists() && path.is_dir() {
                    println!("[✓] Directory `{}/` exists.", dir);
                } else {
                    println!("[!] Directory `{}/` is missing.", dir);
                }
            }

            // 3. 校验托管的目标文件物理存活状态
            let config_content = fs::read_to_string(&config_path).unwrap_or_default();
            if let Ok(config) = toml::from_str::<Config>(&config_content) {
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
                ("r2", "-v"),
            ];

            for (tool, arg) in tools {
                match get_tool_version(tool, arg) {
                    Some(ver) => println!("[✓] {}: {}", tool, ver),
                    None => println!("[!] {}: Not found (optional/recommended)", tool),
                }
            }

            match check_ghidra_info() {
                Some(info) => println!("[✓] Ghidra: {}", info),
                None => println!("[!] Ghidra: Not found (set GHIDRA_INSTALL_DIR if installed)."),
            }

            println!("\nDoctor check completed.");
        }
    }
}
