// src/config.rs
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

// 配置文件对应的 Rust 结构体

// 💡自动反序列化
// 加上 #[derive(Deserialize)] 后，serde 库会帮我们把纯文本的 TOML 文件
// 自动转换成内存中带类型的 Rust 结构体。Debug 允许我们打印结构体。
/// 代表 `orev.toml` 配置文件根结构
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_root: Option<PathBuf>,
    pub case: CaseInfo,
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

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StorageConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub store_root: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools_root: Option<PathBuf>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectConfig {
    pub workspace: WorkspaceInfo,
    #[serde(default)]
    pub storage: StorageConfig,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceInfo {
    pub cases: Vec<String>,
}

#[derive(Debug)]
pub enum WorkspaceConfig {
    Case(Config),
    Project(ProjectConfig),
}

impl WorkspaceConfig {
    pub fn parse(content: &str) -> Result<Self, toml::de::Error> {
        let value: toml::Value = toml::from_str(content)?;
        // Mixed case/project configurations must still fail strict schema validation.
        if value.get("workspace").is_some() && value.get("case").is_none() {
            toml::from_str(content).map(Self::Project)
        } else {
            toml::from_str(content).map(Self::Case)
        }
    }

    pub fn storage(&self) -> &StorageConfig {
        match self {
            Self::Case(config) => &config.storage,
            Self::Project(config) => &config.storage,
        }
    }
}

impl Config {
    pub fn new_case(name: String) -> Self {
        Self {
            project_root: None,
            case: CaseInfo {
                name,
                version: "0.1.0".to_string(),
                investigators: Vec::new(),
            },
            storage: StorageConfig::default(),
        }
    }

    pub fn resolve_kb_root(&self, case_root: &Path) -> io::Result<PathBuf> {
        match &self.project_root {
            Some(root) => {
                let kb = fs::canonicalize(case_root.join(root).join("kb"))?;
                if !kb.is_dir() {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "project knowledge base must be a directory",
                    ));
                }
                Ok(kb)
            }
            None => Ok(case_root.join("kb")),
        }
    }
}

impl StorageConfig {
    pub fn resolve_store_root(&self, case_root: &Path) -> io::Result<PathBuf> {
        resolve_storage_root(case_root, self.store_root.as_deref(), "store")
    }

    pub fn resolve_tools_root(&self, case_root: &Path) -> io::Result<PathBuf> {
        resolve_storage_root(case_root, self.tools_root.as_deref(), "tools")
    }
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
