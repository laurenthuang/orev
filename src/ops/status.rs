// src/ops/status.rs
use crate::config::Config;
use std::env;
use std::fs;

/// 核心业务函数：展示当前工作区的状态与资产盘点
pub fn run_status() {
    let current_dir = env::current_dir().expect("Failed to get current directory");
    let config_path = current_dir.join("orev.toml");

    // 1. 校验配置文件是否存在
    if !config_path.exists() {
        println!("Error: `orev.toml` not found in current directory.");
        println!("Please run `orev init` first to initialize a workspace.");
        return;
    }

    // 2. 读取并反序列化配置文件
    let config_content = fs::read_to_string(&config_path).expect("Failed to read orev.toml");

    // 【Rust 基础概念：反序列化错误处理】
    // 使用 toml::from_str 将文本安全转换为 Config 结构体，解析失败则直接提示。
    let config: Config = match toml::from_str(&config_content) {
        Ok(cfg) => cfg,
        Err(e) => {
            eprintln!("Error: Failed to parse `orev.toml` format: {}", e);
            return;
        }
    };

    // 3. 打印工作区状态总览
    println!("=== Orev Workspace Status ===");
    println!("Project Name:    {}", config.project.name);
    println!("Project Version: {}", config.project.version);
    println!("Workspace Root:  {}", current_dir.display());

    println!("\nManaged Target Binaries:");
    if config.targets.files.is_empty() {
        println!("  (No targets registered)");
    } else {
        for file_path in &config.targets.files {
            let target_abs = current_dir.join(file_path);
            if target_abs.exists() {
                println!("  [✓] {} (active on disk)", file_path);
            } else {
                println!("  [X] {} (missing on disk!)", file_path);
            }
        }
    }
}
