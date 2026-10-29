// src/config.rs
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

// 配置文件对应的 Rust 结构体

// 💡自动反序列化
// 加上 #[derive(Deserialize)] 后，serde 库会帮我们把纯文本的 TOML 文件
// 自动转换成内存中带类型的 Rust 结构体。Debug 允许我们打印结构体。
/// 案件配置，可通过 `workspace_root` 关联工作区。
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaseConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_root: Option<PathBuf>,
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
pub struct WorkspaceConfig {
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
pub enum RootConfig {
    Case(CaseConfig),
    Workspace(WorkspaceConfig),
}

impl RootConfig {
    pub fn parse(content: &str) -> Result<Self, toml::de::Error> {
        let value: toml::Value = toml::from_str(content)?;
        // Mixed case/workspace configurations must still fail strict schema validation.
        if value.get("workspace").is_some() && value.get("case").is_none() {
            toml::from_str(content).map(Self::Workspace)
        } else {
            toml::from_str(content).map(Self::Case)
        }
    }

    pub fn storage(&self) -> &StorageConfig {
        match self {
            Self::Case(config) => &config.storage,
            Self::Workspace(config) => &config.storage,
        }
    }
}

impl CaseConfig {
    pub fn new_case(name: String) -> Self {
        Self {
            workspace_root: None,
            case: CaseInfo {
                name,
                version: "0.1.0".to_string(),
                investigators: Vec::new(),
            },
            storage: StorageConfig::default(),
        }
    }

    pub fn resolve_kb_root(&self, case_root: &Path) -> io::Result<PathBuf> {
        match &self.workspace_root {
            Some(workspace_root) => {
                let kb = fs::canonicalize(case_root.join(workspace_root).join("kb"))?;
                if !kb.is_dir() {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "workspace knowledge base must be a directory",
                    ));
                }
                Ok(kb)
            }
            None => Ok(case_root.join("kb")),
        }
    }
}

impl StorageConfig {
    pub fn resolve_store_root(&self, config_root: &Path) -> io::Result<PathBuf> {
        resolve_storage_root(config_root, self.store_root.as_deref(), "store")
    }

    pub fn resolve_tools_root(&self, config_root: &Path) -> io::Result<PathBuf> {
        resolve_storage_root(config_root, self.tools_root.as_deref(), "tools")
    }
}

fn resolve_storage_root(
    config_root: &Path,
    configured: Option<&Path>,
    default_dir: &str,
) -> io::Result<PathBuf> {
    let config_root = fs::canonicalize(config_root)?;

    let Some(configured) = configured else {
        return Ok(config_root.join(default_dir));
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

    if resolved.starts_with(&config_root) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "configured {default_dir} root must be outside the configuration root: `{}`",
                resolved.display()
            ),
        ));
    }

    Ok(resolved)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn root_configuration_distinguishes_cases_and_workspaces() {
        let case = toml::to_string_pretty(&CaseConfig::new_case("analysis".into())).unwrap();
        assert!(matches!(
            RootConfig::parse(&case).unwrap(),
            RootConfig::Case(_)
        ));
        let workspace = toml::to_string_pretty(&WorkspaceConfig::default()).unwrap();
        assert!(matches!(
            RootConfig::parse(&workspace).unwrap(),
            RootConfig::Workspace(_)
        ));
    }

    #[test]
    fn workspace_case_round_trips_the_workspace_root_association() {
        let mut case = CaseConfig::new_case("analysis".into());
        case.workspace_root = Some(PathBuf::from("../.."));
        let content = toml::to_string_pretty(&case).unwrap();
        assert!(content.contains("workspace_root = \"../..\""));
        assert!(!content.contains("project_root"));
        let RootConfig::Case(parsed) = RootConfig::parse(&content).unwrap() else {
            panic!("a workspace case must still use the case schema");
        };
        assert_eq!(parsed.workspace_root.as_deref(), Some(Path::new("../..")));
        assert_eq!(parsed.case.name, "analysis");
    }

    #[test]
    fn root_configuration_rejects_legacy_associations_and_mixed_schemas() {
        for content in [
            "project_root = \"../..\"\n[case]\nname = \"analysis\"\nversion = \"0.1.0\"\n",
            "project_root = \"../..\"\nworkspace_root = \"../..\"\n[case]\nname = \"analysis\"\nversion = \"0.1.0\"\n",
            "workspace_root = \"..\"\n[workspace]\ncases = []\n",
            "workspace_root = \"../..\"\nworkspace_root = \"../..\"\n[case]\nname = \"analysis\"\nversion = \"0.1.0\"\n",
            "[case]\nname = \"analysis\"\nversion = \"0.1.0\"\n[workspace]\ncases = []\n",
        ] {
            assert!(RootConfig::parse(content).is_err(), "{content}");
        }
    }
}
