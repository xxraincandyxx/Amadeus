// @amadeus-header
// summary: Compatibility registry that composes tool packs, profiles, and unified execution into one catalog.
// layer: tools
// status: active
// feature_flags:
// - concurrency
// provides:
// - module: crate::tools::registry
// - type: crate::tools::registry::ToolRegistry
// uses:
// - module: crate::agent::config::Config
// - module: crate::mcp::adapter
// - module: crate::tools::platform
// - module: crate::tools::tool_trait
// invariants:
// - Provider-visible tool definitions derive from the composed catalog.
// - Legacy tool registration remains available during migration.
// side_effects: none
// tests:
// - tests/tool_approval_test.rs
// @end-amadeus-header

use std::sync::Arc;
use std::sync::RwLock;

use serde_json::Value;

use crate::agent::config::Config;
#[cfg(feature = "concurrency")]
use crate::concurrency::FileLockManager;
use crate::core::id::AgentId;
use crate::error::Result;
use crate::mcp::adapter::create_mcp_tool_pack;
use crate::mcp::McpServerConfig;
use crate::permissions::PermissionMode;
use crate::tools::bash::BashTool;
#[cfg(not(feature = "concurrency"))]
use crate::tools::file::FileLockManager;
use crate::tools::file::{EditFileTool, FileTools, ReadFileTool, WriteFileTool};
use crate::tools::glob::GlobTool;
use crate::tools::grep::GrepTool;
use crate::tools::platform::{
    legacy_registration, ComposedToolCatalog, ToolCatalogView, ToolLevel, ToolPack, ToolProfile,
    ToolRegistration, ToolSource, ToolSpec,
};
use crate::tools::todo::{TodoManager, TodoTool};
use crate::tools::tool_trait::Tool;
use crate::tools::web::WebFetchTool;

#[derive(Clone)]
pub struct ToolRegistry {
    packs: Arc<Vec<ToolPack>>,
    catalog: Arc<ComposedToolCatalog>,
    profile: ToolProfile,
    file_lock_manager: Option<Arc<FileLockManager>>,
    agent_id: Option<AgentId>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        let profile = ToolProfile::default_root();
        Self {
            packs: Arc::new(Vec::new()),
            catalog: Arc::new(ComposedToolCatalog::empty(profile.clone())),
            profile,
            file_lock_manager: None,
            agent_id: None,
        }
    }

    /// Create a new ToolRegistry with file locking enabled.
    pub fn new_with_file_locks(file_lock_manager: Arc<FileLockManager>, agent_id: AgentId) -> Self {
        Self {
            file_lock_manager: Some(file_lock_manager),
            agent_id: Some(agent_id),
            ..Self::new()
        }
    }

    pub fn with_defaults(config: &Config) -> Self {
        Self::with_defaults_and_todo(config, Arc::new(RwLock::new(TodoManager::new())))
    }

    pub fn with_defaults_and_todo(config: &Config, todo_manager: Arc<RwLock<TodoManager>>) -> Self {
        Self::with_defaults_and_todo_with_locks(config, todo_manager, None, None)
    }

    pub fn with_defaults_and_todo_with_locks(
        config: &Config,
        todo_manager: Arc<RwLock<TodoManager>>,
        file_lock_manager: Option<Arc<FileLockManager>>,
        agent_id: Option<AgentId>,
    ) -> Self {
        let file_tools =
            FileTools::from_config_with_locks(config, file_lock_manager.clone(), agent_id);
        let profile = tool_profile_from_config(config, false);

        Self {
            packs: Arc::new(default_builtin_packs(config, file_tools, todo_manager)),
            catalog: Arc::new(ComposedToolCatalog::empty(profile.clone())),
            profile,
            file_lock_manager,
            agent_id,
        }
        .recompose()
    }

    pub fn with_file_locks(mut self, manager: Arc<FileLockManager>, agent_id: AgentId) -> Self {
        self.file_lock_manager = Some(manager);
        self.agent_id = Some(agent_id);
        self
    }

    pub fn file_lock_manager(&self) -> Option<&Arc<FileLockManager>> {
        self.file_lock_manager.as_ref()
    }

    pub fn agent_id(&self) -> Option<&AgentId> {
        self.agent_id.as_ref()
    }

    pub fn with_sub_agent_child_defaults(config: &Config) -> Self {
        Self::with_sub_agent_child_defaults_recursive(config, None)
    }

    pub fn with_sub_agent_child_defaults_recursive(
        config: &Config,
        subagent_tool: Option<Arc<dyn Tool>>,
    ) -> Self {
        let file_tools = FileTools::from_config(config);
        let todo_manager = Arc::new(RwLock::new(TodoManager::new()));
        let mut registry = Self {
            packs: Arc::new(default_builtin_packs(config, file_tools, todo_manager)),
            catalog: Arc::new(ComposedToolCatalog::empty(tool_profile_from_config(
                config, true,
            ))),
            profile: tool_profile_from_config(config, true),
            file_lock_manager: None,
            agent_id: None,
        };

        if let Some(tool) = subagent_tool {
            registry = registry.register_arc(tool);
        }

        registry.recompose()
    }

    pub fn with_profile(mut self, profile: ToolProfile) -> Self {
        self.profile = profile;
        self.recompose()
    }

    pub fn profile(&self) -> &ToolProfile {
        &self.profile
    }

    pub fn catalog(&self) -> &ComposedToolCatalog {
        self.catalog.as_ref()
    }

    pub fn register(self, tool: Box<dyn Tool>) -> Self {
        self.register_arc(Arc::from(tool))
    }

    pub fn register_arc(mut self, tool: Arc<dyn Tool>) -> Self {
        let mut packs = (*self.packs).clone();
        packs.push(ToolPack {
            name: "runtime".to_string(),
            tools: vec![compat_registration(tool)],
        });
        self.packs = Arc::new(packs);
        self.recompose()
    }

    pub async fn register_mcp_server(mut self, config: &McpServerConfig) -> Result<Self> {
        let mut packs = (*self.packs).clone();
        packs.push(create_mcp_tool_pack(config).await?);
        self.packs = Arc::new(packs);
        Ok(self.recompose())
    }

    pub fn get(&self, name: &str) -> Option<ToolSpec> {
        self.catalog.spec(name).cloned()
    }

    pub fn schemas(&self) -> Vec<Value> {
        self.catalog.provider_definitions()
    }

    pub fn names(&self) -> Vec<String> {
        self.catalog.names()
    }

    pub fn inventory(&self) -> Vec<ToolCatalogView> {
        self.catalog.inventory()
    }

    pub fn len(&self) -> usize {
        self.catalog.len()
    }

    pub fn is_empty(&self) -> bool {
        self.catalog.is_empty()
    }

    pub async fn execute(&self, name: &str, input: Value) -> Result<String> {
        Ok(self.catalog.execute(name, input).await?.output)
    }

    pub async fn execute_structured(
        &self,
        name: &str,
        input: Value,
    ) -> Result<crate::tools::ToolExecutionResult> {
        self.catalog.execute(name, input).await
    }

    pub fn filter_by_name(mut self, allowed: &[String]) -> Self {
        self.catalog = Arc::new(self.catalog.filter_by_name(allowed));
        self
    }

    pub fn packs(&self) -> &[ToolPack] {
        self.packs.as_ref()
    }

    fn recompose(mut self) -> Self {
        self.catalog = Arc::new(ComposedToolCatalog::compose(
            self.packs.as_ref(),
            self.profile.clone(),
        ));
        self
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for ToolRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ToolRegistry")
            .field("profile", &self.profile)
            .field(
                "packs",
                &self.packs.iter().map(|pack| &pack.name).collect::<Vec<_>>(),
            )
            .field("catalog", &self.catalog)
            .finish()
    }
}

fn default_builtin_packs(
    config: &Config,
    file_tools: FileTools,
    todo_manager: Arc<RwLock<TodoManager>>,
) -> Vec<ToolPack> {
    let bash = Arc::new(BashTool::from_config(config)) as Arc<dyn Tool>;
    let read_file = Arc::new(ReadFileTool::new(file_tools.clone())) as Arc<dyn Tool>;
    let write_file = Arc::new(WriteFileTool::new(file_tools.clone())) as Arc<dyn Tool>;
    let edit_file = Arc::new(EditFileTool::new(file_tools)) as Arc<dyn Tool>;
    let glob = Arc::new(GlobTool::from_config(config)) as Arc<dyn Tool>;
    let grep = Arc::new(GrepTool::from_config(config)) as Arc<dyn Tool>;
    let todo = Arc::new(TodoTool::new(todo_manager)) as Arc<dyn Tool>;
    let web_fetch = Arc::new(WebFetchTool::from_config(config)) as Arc<dyn Tool>;

    vec![
        ToolPack {
            name: "shell".to_string(),
            tools: vec![legacy_registration(
                bash,
                "shell",
                ToolSource::Builtin,
                ToolLevel::Primitive,
                PermissionMode::WorkspaceWrite,
                vec!["shell".to_string(), "filesystem".to_string()],
                Vec::new(),
            )],
        },
        ToolPack {
            name: "filesystem".to_string(),
            tools: vec![
                legacy_registration(
                    read_file,
                    "filesystem",
                    ToolSource::Builtin,
                    ToolLevel::Primitive,
                    PermissionMode::ReadOnly,
                    vec!["filesystem".to_string(), "read".to_string()],
                    Vec::new(),
                ),
                legacy_registration(
                    write_file,
                    "filesystem",
                    ToolSource::Builtin,
                    ToolLevel::Primitive,
                    PermissionMode::WorkspaceWrite,
                    vec!["filesystem".to_string(), "write".to_string()],
                    Vec::new(),
                ),
                legacy_registration(
                    edit_file,
                    "filesystem",
                    ToolSource::Builtin,
                    ToolLevel::Primitive,
                    PermissionMode::WorkspaceWrite,
                    vec!["filesystem".to_string(), "edit".to_string()],
                    Vec::new(),
                ),
            ],
        },
        ToolPack {
            name: "search".to_string(),
            tools: vec![
                legacy_registration(
                    glob,
                    "search",
                    ToolSource::Builtin,
                    ToolLevel::Primitive,
                    PermissionMode::ReadOnly,
                    vec!["search".to_string(), "files".to_string()],
                    vec!["glob_search".to_string()],
                ),
                legacy_registration(
                    grep,
                    "search",
                    ToolSource::Builtin,
                    ToolLevel::Primitive,
                    PermissionMode::ReadOnly,
                    vec!["search".to_string(), "content".to_string()],
                    vec!["grep_search".to_string()],
                ),
            ],
        },
        ToolPack {
            name: "planning".to_string(),
            tools: vec![legacy_registration(
                todo,
                "planning",
                ToolSource::Builtin,
                ToolLevel::ControlPlane,
                PermissionMode::ReadOnly,
                vec!["planning".to_string(), "tracking".to_string()],
                vec!["todo_write".to_string()],
            )],
        },
        ToolPack {
            name: "web".to_string(),
            tools: vec![legacy_registration(
                web_fetch,
                "web",
                ToolSource::Builtin,
                ToolLevel::Primitive,
                PermissionMode::ReadOnly,
                vec!["web".to_string(), "network".to_string()],
                vec!["web_fetch_text".to_string()],
            )],
        },
    ]
}

fn compat_registration(tool: Arc<dyn Tool>) -> ToolRegistration {
    let permission = match tool.name() {
        "read_file" | "glob" | "grep" | "web_fetch" | "memory" | "rag" | "todo" | "sub_agent" | "call_peer" => {
            PermissionMode::ReadOnly
        }
        "write_file" | "edit_file" | "bash" => PermissionMode::WorkspaceWrite,
        _ => PermissionMode::DangerFullAccess,
    };
    let level = match tool.name() {
        "todo" | "sub_agent" | "call_peer" => ToolLevel::ControlPlane,
        _ => ToolLevel::Primitive,
    };

    legacy_registration(
        tool,
        "runtime",
        ToolSource::Runtime,
        level,
        permission,
        vec!["runtime".to_string()],
        Vec::new(),
    )
}

fn tool_profile_from_config(config: &Config, subagent: bool) -> ToolProfile {
    let profile_name = if subagent {
        config.tools.subagent_profile.clone()
    } else {
        config.tools.default_profile.clone()
    };

    let defaults = if subagent {
        ToolProfile::default_subagent()
    } else {
        ToolProfile::default_root()
    };

    let Some(override_profile) = config.tools.profiles.get(&profile_name) else {
        return defaults;
    };

    ToolProfile {
        name: profile_name,
        enabled_packs: if override_profile.enabled_packs.is_empty() {
            defaults.enabled_packs
        } else {
            override_profile.enabled_packs.clone()
        },
        enabled_tools: override_profile.enabled_tools.clone(),
        disabled_tools: override_profile.disabled_tools.clone(),
        allow_aliases: override_profile.allow_aliases,
        include_mcp: override_profile.include_mcp,
        include_control_plane: override_profile.include_control_plane,
        model_permission_mode: override_profile
            .model_permission_mode
            .unwrap_or(defaults.model_permission_mode),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn config_with_profile() -> Config {
        let mut profiles = HashMap::new();
        profiles.insert(
            "tight".to_string(),
            amadeus_config::ToolProfileConfig {
                enabled_packs: vec!["filesystem".to_string()],
                enabled_tools: Vec::new(),
                disabled_tools: vec!["write_file".to_string()],
                allow_aliases: false,
                include_mcp: false,
                include_control_plane: false,
                model_permission_mode: Some(PermissionMode::ReadOnly),
            },
        );

        Config {
            tools: amadeus_config::ToolSettings {
                default_profile: "tight".to_string(),
                subagent_profile: "subagent".to_string(),
                profiles,
            },
            ..Config::default()
        }
    }

    #[test]
    fn registry_composes_profile_filtered_catalog() {
        let registry = ToolRegistry::with_defaults(&config_with_profile());
        let names = registry.names();
        assert!(names.contains(&"read_file".to_string()));
        assert!(!names.contains(&"write_file".to_string()));
        assert!(!names.contains(&"todo".to_string()));
    }
}
