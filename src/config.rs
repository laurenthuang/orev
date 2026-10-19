// src/config.rs
use serde::{Deserialize, Serialize};

// ==========================================================
// 配置文件对应的 Rust 结构体
// ==========================================================
// 【Rust 基础概念：Deserialize 自动反序列化】
// 加上 #[derive(Deserialize)] 后，serde 库会帮我们把纯文本的 TOML 文件
// 自动转换成内存中带类型的 Rust 结构体。Debug 允许我们打印结构体。
/// 代表 `orev.toml` 的配置文件根结构
#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    pub case: CaseInfo,
    pub workspace: WorkspaceConfig,
    pub targets: TargetsConfig,
}

/// 分析案的基本元数据
#[derive(Debug, Serialize, Deserialize)]
pub struct CaseInfo {
    pub name: String,
    pub version: String,
    #[serde(default = "default_authors")]
    pub investigators: Vec<String>,
}

fn default_authors() -> Vec<String> {
    Vec::new()
}

/// 工作区目录路径配置
#[derive(Debug, Serialize, Deserialize)]
pub struct WorkspaceConfig {
    pub notes_dir: String,
    pub patches_dir: String,
    pub reports_dir: String,
    pub scripts_dir: String,
}

/// 样本目标配置
#[derive(Debug, Serialize, Deserialize)]
pub struct TargetsConfig {
    pub files: Vec<String>,
}
