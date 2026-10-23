// src/config.rs
use serde::{Deserialize, Deserializer, Serialize};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

const SUPPORTED_STAGES: &[&str] = &[
    "00_inbox",
    "10_unpack",
    "15_patch",
    "20_static",
    "30_decompile",
    "40_dynamic",
    "50_findings",
    "60_diff",
    "90_report",
];

// 配置文件对应的 Rust 结构体

// 💡自动反序列化
// 加上 #[derive(Deserialize)] 后，serde 库会帮我们把纯文本的 TOML 文件
// 自动转换成内存中带类型的 Rust 结构体。Debug 允许我们打印结构体。
/// 代表 `orev.toml` 配置文件根结构
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(deserialize_with = "deserialize_stages")]
    pub stages: Vec<String>,
    pub case: CaseInfo,
    pub targets: TargetsConfig,
    #[serde(default)]
    pub storage: StorageConfig,
}

/// 分析案例的基本元数据
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaseInfo {
    pub name: String,
    pub version: String,
    #[serde(default, alias = "analysts")]
    pub investigators: Vec<String>,
}

/// 样本目标配置
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TargetsConfig {
    pub files: Vec<String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StorageConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub store_root: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools_root: Option<PathBuf>,
}

impl StorageConfig {
    pub fn resolve_store_root(&self, case_root: &Path) -> io::Result<PathBuf> {
        resolve_storage_root(case_root, self.store_root.as_deref(), "store")
    }

    pub fn resolve_tools_root(&self, case_root: &Path) -> io::Result<PathBuf> {
        resolve_storage_root(case_root, self.tools_root.as_deref(), "tools")
    }
}

fn deserialize_stages<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let stages = Vec::<String>::deserialize(deserializer)?;

    for stage in &stages {
        if !SUPPORTED_STAGES.contains(&stage.as_str()) {
            return Err(serde::de::Error::custom(format!(
                "unsupported stage `{stage}`; expected one of: {}",
                SUPPORTED_STAGES.join(", ")
            )));
        }
    }

    Ok(stages)
}

fn resolve_storage_root(
    case_root: &Path,
    configured: Option<&Path>,
    default_dir: &str,
) -> io::Result<PathBuf> {
    let case_root = fs::canonicalize(case_root)?;

    let Some(configured) = configured else {
        return Ok(case_root.join(default_dir));
    };

    if !configured.is_absolute() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("expected an absolute path, got `{}`", configured.display()),
        ));
    }

    let resolved = fs::canonicalize(configured).map_err(|e| {
        io::Error::new(
            e.kind(),
            format!("cannot resolve `{}`: {}", configured.display(), e),
        )
    })?;

    if !fs::metadata(&resolved)?.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("expected an existing directory at `{}`", resolved.display()),
        ));
    }

    if resolved.starts_with(&case_root) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "configured {default_dir} root must be outside the case directory: `{}`",
                resolved.display()
            ),
        ));
    }

    Ok(resolved)
}
