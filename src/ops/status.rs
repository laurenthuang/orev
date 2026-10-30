// src/ops/status.rs
use crate::config::{RootConfig, find_config_root};
use std::env;
use std::fs;
use std::io;

/// 核心业务函数：展示当前案例或多案件工作区的基本信息
pub fn run_status() {
    let config_root = match env::current_dir().and_then(|cwd| find_config_root(&cwd)) {
        Ok(root) => root,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            println!("Error: `orev.toml` not found in current directory or its ancestors.");
            println!(
                "Please run `orev init` for a standalone case or `orev init --workspace` for a workspace."
            );
            return;
        }
        Err(error) => {
            eprintln!("Error: Failed to locate `orev.toml`: {error}");
            return;
        }
    };
    let config_path = config_root.join("orev.toml");

    // 2. 读取并反序列化配置文件
    let config_content = match fs::read_to_string(&config_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error: Failed to read `{}`: {}", config_path.display(), e);
            return;
        }
    };

    let config = match RootConfig::parse(&config_content) {
        Ok(config) => config,
        Err(e) => {
            eprintln!(
                "Error: Failed to parse `{}` format: {}",
                config_path.display(),
                e
            );
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
