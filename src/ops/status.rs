// src/ops/status.rs
use crate::config::WorkspaceConfig;
use std::env;
use std::fs;

/// 核心业务函数：展示当前案例或多案件项目的基本信息
pub fn run_status() {
    let current_dir = env::current_dir().expect("Failed to get current directory");
    let config_path = current_dir.join("orev.toml");

    // 1. 校验配置文件是否存在
    if !config_path.exists() {
        println!("Error: `orev.toml` not found in current directory.");
        println!(
            "Please run `orev init` for a single case project or `orev init --workspace` for a multi-case project."
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

    let config = match WorkspaceConfig::parse(&config_content) {
        Ok(config) => config,
        Err(e) => {
            eprintln!("Error: Failed to parse `orev.toml` format: {}", e);
            return;
        }
    };

    match config {
        WorkspaceConfig::Project(config) => {
            println!("=== Orev Multi-Case Project Workspace Status ===");
            println!("Workspace Root:  {}", current_dir.display());
            println!("Registered Cases: {}", config.workspace.cases.len());
            for name in &config.workspace.cases {
                println!("  - {}", name);
            }
        }
        WorkspaceConfig::Case(config) => {
            println!("=== Orev Analysis Case Workspace Status ===");
            println!("Case Name:       {}", config.case.name);
            println!("Case Version:    {}", config.case.version);
            println!("Workspace Root:  {}", current_dir.display());
        }
    }
}
