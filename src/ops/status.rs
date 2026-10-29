// src/ops/status.rs
use crate::config::RootConfig;
use std::env;
use std::fs;

/// 核心业务函数：展示当前案例或多案件工作区的基本信息
pub fn run_status() {
    let config_root = env::current_dir().expect("Failed to get current directory");
    let config_path = config_root.join("orev.toml");

    // 1. 校验配置文件是否存在
    if !config_path.exists() {
        println!("Error: `orev.toml` not found in current directory.");
        println!(
            "Please run `orev init` for a standalone case or `orev init --workspace` for a workspace."
        );
        return;
    }

    // 2. 读取并反序列化配置文件
    let config_content = match fs::read_to_string(&config_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error: Failed to read `orev.toml`: {}", e);
            return;
        }
    };

    let config = match RootConfig::parse(&config_content) {
        Ok(config) => config,
        Err(e) => {
            eprintln!("Error: Failed to parse `orev.toml` format: {}", e);
            return;
        }
    };

    match config {
        RootConfig::Workspace(config) => {
            println!("=== Orev Workspace Status ===");
            println!("Workspace Root:  {}", config_root.display());
            println!("Registered Cases: {}", config.workspace.cases.len());
            for name in &config.workspace.cases {
                println!("  - {}", name);
            }
        }
        RootConfig::Case(config) => {
            println!("=== Orev Case Status ===");
            println!("Case Name:       {}", config.case.name);
            println!("Case Version:    {}", config.case.version);
            println!("Case Root:       {}", config_root.display());
        }
    }
}
