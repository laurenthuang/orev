use serde::Deserialize;

// ==========================================================
// 配置文件对应的 Rust 结构体
// ==========================================================
// 【Rust 基础概念：Deserialize 自动反序列化】
// 加上 #[derive(Deserialize)] 后，serde 库会帮我们把纯文本的 TOML 文件
// 自动转换成内存中带类型的 Rust 结构体。Debug 允许我们打印结构体。
#[derive(Deserialize, Debug)]
pub struct Config {
    pub project: ProjectConfig,
    pub targets: TargetsConfig,
}

#[derive(Deserialize, Debug)]
pub struct ProjectConfig {
    pub name: String,
    pub version: String,
}

#[derive(Deserialize, Debug)]
pub struct TargetsConfig {
    pub files: Vec<String>,
}
