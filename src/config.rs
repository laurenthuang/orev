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
    #[serde(default, skip_serializing_if = "StorageConfig::is_empty")]
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

#[derive(Clone, Copy)]
pub struct WorkspaceContext<'a> {
    pub root: &'a Path,
    pub config: &'a WorkspaceConfig,
}

#[derive(Debug)]
pub enum ResourceRoot {
    Local(PathBuf),
    Existing(PathBuf),
}

impl ResourceRoot {
    pub fn path(&self) -> &Path {
        match self {
            Self::Local(path) | Self::Existing(path) => path,
        }
    }

    fn require_existing(self, field: &str) -> io::Result<Self> {
        existing_directory(self.path())
            .map(Self::Existing)
            .map_err(|error| resolution_error(field, error))
    }
}

pub struct ResolvedResources {
    pub kb: ResourceRoot,
    pub store: ResourceRoot,
    pub tools: ResourceRoot,
}

impl ResolvedResources {
    pub fn require_existing(self) -> io::Result<Self> {
        Ok(Self {
            kb: self.kb.require_existing("workspace knowledge base")?,
            store: self.store.require_existing("storage.store_root")?,
            tools: self.tools.require_existing("storage.tools_root")?,
        })
    }
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

    pub fn resolve_resources(&self, config_root: &Path) -> io::Result<ResolvedResources> {
        match self {
            Self::Case(config) => config.resolve_resources(config_root, None),
            Self::Workspace(config) => config.resolve_resources(config_root),
        }
    }
}

impl WorkspaceConfig {
    pub fn resolve_resources(&self, config_root: &Path) -> io::Result<ResolvedResources> {
        resolve_resources(config_root, &self.storage, None)
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

    pub fn resolve_resources(
        &self,
        case_root: &Path,
        staged_workspace: Option<WorkspaceContext<'_>>,
    ) -> io::Result<ResolvedResources> {
        let Some(workspace_root) = &self.workspace_root else {
            return resolve_resources(case_root, &self.storage, None);
        };
        let workspace_root = existing_directory(&case_root.join(workspace_root))
            .map_err(|error| resolution_error("workspace_root", error))?;
        if let Some(context) = staged_workspace
            && fs::canonicalize(context.root)? == workspace_root
        {
            return resolve_resources(case_root, &self.storage, Some(context));
        }

        let config_path = workspace_root.join("orev.toml");
        let content = fs::read_to_string(&config_path).map_err(|error| {
            io::Error::new(
                error.kind(),
                format!(
                    "Failed to read workspace configuration `{}`: {error}",
                    config_path.display()
                ),
            )
        })?;
        let workspace: WorkspaceConfig = toml::from_str(&content).map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Invalid workspace configuration `{}`: {error}",
                    config_path.display()
                ),
            )
        })?;
        resolve_resources(
            case_root,
            &self.storage,
            Some(WorkspaceContext {
                root: &workspace_root,
                config: &workspace,
            }),
        )
    }
}

impl StorageConfig {
    fn is_empty(&self) -> bool {
        self.store_root.is_none() && self.tools_root.is_none()
    }
}

fn resolve_resources(
    config_root: &Path,
    storage: &StorageConfig,
    workspace: Option<WorkspaceContext<'_>>,
) -> io::Result<ResolvedResources> {
    let config_root = fs::canonicalize(config_root)?;
    let store = resolve_storage_field(
        &config_root,
        storage.store_root.as_deref(),
        workspace.map(|context| (context.root, context.config.storage.store_root.as_deref())),
        "store",
    )?;
    let tools = resolve_storage_field(
        &config_root,
        storage.tools_root.as_deref(),
        workspace.map(|context| (context.root, context.config.storage.tools_root.as_deref())),
        "tools",
    )?;
    let kb = match workspace {
        Some(context) => ResourceRoot::Existing(
            existing_directory(&context.root.join("kb"))
                .map_err(|error| resolution_error("workspace knowledge base", error))?,
        ),
        None => ResourceRoot::Local(config_root.join("kb")),
    };
    Ok(ResolvedResources { kb, store, tools })
}

fn resolve_storage_field(
    config_root: &Path,
    configured: Option<&Path>,
    inherited: Option<(&Path, Option<&Path>)>,
    kind: &str,
) -> io::Result<ResourceRoot> {
    let (root, configured, shared) = match (configured, inherited) {
        (Some(path), _) => (config_root, Some(path), false),
        (None, Some((root, path))) => (root, path, true),
        (None, None) => (config_root, None, false),
    };
    resolve_storage_root(root, configured, kind, shared)
        .map_err(|error| resolution_error(&format!("storage.{kind}_root"), error))
}

fn resolution_error(field: &str, error: io::Error) -> io::Error {
    io::Error::new(error.kind(), format!("Failed to resolve {field}: {error}"))
}

fn existing_directory(path: &Path) -> io::Result<PathBuf> {
    let resolved = fs::canonicalize(path).map_err(|error| {
        io::Error::new(
            error.kind(),
            format!("cannot resolve `{}`: {error}", path.display()),
        )
    })?;
    if !fs::metadata(&resolved)?.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("expected an existing directory at `{}`", resolved.display()),
        ));
    }
    Ok(resolved)
}

fn resolve_storage_root(
    config_root: &Path,
    configured: Option<&Path>,
    default_dir: &str,
    shared: bool,
) -> io::Result<ResourceRoot> {
    let config_root = fs::canonicalize(config_root)?;

    let Some(configured) = configured else {
        let local = config_root.join(default_dir);
        return if shared {
            existing_directory(&local).map(ResourceRoot::Existing)
        } else {
            Ok(ResourceRoot::Local(local))
        };
    };

    let resolved = existing_directory(&config_root.join(configured))?;

    if resolved.starts_with(&config_root) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "configured {default_dir} root must be outside the configuration root: `{}`",
                resolved.display()
            ),
        ));
    }

    Ok(ResourceRoot::Existing(resolved))
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
