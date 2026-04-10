// @amadeus-header
// summary: Configurable tool platform contracts for composing model-facing specs, runtime execution, and policy.
// layer: tools
// status: active
// feature_flags: none
// provides:
// - module: crate::tools::platform
// - trait: crate::tools::platform::ToolExecutor
// - type: crate::tools::platform::ToolSpec
// - type: crate::tools::platform::ToolPolicy
// - type: crate::tools::platform::ToolPack
// - type: crate::tools::platform::ToolProfile
// - type: crate::tools::platform::ComposedToolCatalog
// uses:
// - module: crate::error
// - module: crate::permissions
// - module: crate::tools::tool_trait
// - format: JSON values
// invariants:
// - Composed catalogs resolve aliases to canonical names before execution.
// - Provider-visible tool definitions derive from ToolSpec rather than legacy schema blobs.
// side_effects: none
// tests:
// - cmd: cargo test -p core tool_platform --features full
// @end-amadeus-header

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{AgentError, Result};
use crate::permissions::PermissionMode;
use crate::tools::tool_trait::Tool;

/// Origin for a tool exposed to the model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolSource {
    Builtin,
    Mcp,
    Runtime,
    Plugin,
}

impl ToolSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Builtin => "builtin",
            Self::Mcp => "mcp",
            Self::Runtime => "runtime",
            Self::Plugin => "plugin",
        }
    }
}

/// Functional level for a tool.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolLevel {
    Primitive,
    ControlPlane,
}

impl ToolLevel {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Primitive => "primitive",
            Self::ControlPlane => "control_plane",
        }
    }
}

/// Model-facing contract for a tool.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
    pub required_permission: PermissionMode,
    pub source: ToolSource,
    pub level: ToolLevel,
    pub tags: Vec<String>,
    pub aliases: Vec<String>,
    pub pack: String,
    pub prompt_approval: bool,
    pub visible_in_modes: Vec<PermissionMode>,
}

impl ToolSpec {
    /// Convert the tool spec into the provider-facing JSON definition.
    pub fn provider_definition(&self) -> Value {
        serde_json::json!({
            "name": self.name,
            "description": self.description,
            "parameters": self.input_schema,
        })
    }

    /// Clone the spec with an alternate exposed name.
    pub fn with_exposed_name(&self, name: String) -> Self {
        let mut cloned = self.clone();
        cloned.name = name;
        cloned
    }
}

/// Runtime execution result for a tool call.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolExecutionResult {
    pub output: String,
    pub is_error: bool,
    pub metadata: Option<Value>,
}

impl ToolExecutionResult {
    pub fn text_output(&self) -> &str {
        &self.output
    }
}

/// Policy applied during tool catalog composition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolPolicy {
    pub enabled: bool,
    pub visible_to_model: bool,
    pub permission_override: Option<PermissionMode>,
    pub approval_required: Option<bool>,
    pub visible_in_modes: Option<Vec<PermissionMode>>,
    pub max_agent_depth: Option<usize>,
}

impl Default for ToolPolicy {
    fn default() -> Self {
        Self {
            enabled: true,
            visible_to_model: true,
            permission_override: None,
            approval_required: None,
            visible_in_modes: None,
            max_agent_depth: None,
        }
    }
}

/// Runtime execution contract for a composed tool.
#[async_trait]
pub trait ToolExecutor: Send + Sync {
    async fn execute(&self, canonical_name: &str, input: Value) -> Result<ToolExecutionResult>;
}

/// One tool entry inside a pack.
#[derive(Clone)]
pub struct ToolRegistration {
    pub spec: ToolSpec,
    pub policy: ToolPolicy,
    pub executor: Arc<dyn ToolExecutor>,
}

impl std::fmt::Debug for ToolRegistration {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ToolRegistration")
            .field("spec", &self.spec)
            .field("policy", &self.policy)
            .finish()
    }
}

/// Named collection of tools that can be composed into a catalog.
#[derive(Debug, Clone)]
pub struct ToolPack {
    pub name: String,
    pub tools: Vec<ToolRegistration>,
}

/// Profile used to compose the final catalog for an agent session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolProfile {
    pub name: String,
    pub enabled_packs: Vec<String>,
    pub enabled_tools: Vec<String>,
    pub disabled_tools: Vec<String>,
    pub allow_aliases: bool,
    pub include_mcp: bool,
    pub include_control_plane: bool,
    pub model_permission_mode: PermissionMode,
}

impl ToolProfile {
    pub fn default_root() -> Self {
        Self {
            name: "default".to_string(),
            enabled_packs: vec![
                "shell".to_string(),
                "filesystem".to_string(),
                "search".to_string(),
                "planning".to_string(),
                "web".to_string(),
                "orchestration".to_string(),
                "mcp".to_string(),
                "runtime".to_string(),
            ],
            enabled_tools: Vec::new(),
            disabled_tools: Vec::new(),
            allow_aliases: true,
            include_mcp: true,
            include_control_plane: true,
            model_permission_mode: PermissionMode::DangerFullAccess,
        }
    }

    pub fn default_subagent() -> Self {
        Self {
            name: "subagent".to_string(),
            enabled_packs: vec![
                "shell".to_string(),
                "filesystem".to_string(),
                "search".to_string(),
                "planning".to_string(),
                "web".to_string(),
                "runtime".to_string(),
            ],
            enabled_tools: Vec::new(),
            disabled_tools: Vec::new(),
            allow_aliases: true,
            include_mcp: false,
            include_control_plane: false,
            model_permission_mode: PermissionMode::WorkspaceWrite,
        }
    }
}

#[derive(Clone)]
struct ToolCatalogEntry {
    spec: ToolSpec,
    policy: ToolPolicy,
    executor: Arc<dyn ToolExecutor>,
}

impl std::fmt::Debug for ToolCatalogEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ToolCatalogEntry")
            .field("spec", &self.spec)
            .field("policy", &self.policy)
            .finish()
    }
}

/// View record for inventory/reporting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolCatalogView {
    pub name: String,
    pub pack: String,
    pub source: ToolSource,
    pub level: ToolLevel,
    pub schema: Value,
}

/// Fully composed tool catalog for one agent profile.
#[derive(Clone)]
pub struct ComposedToolCatalog {
    profile: ToolProfile,
    entries: Arc<HashMap<String, ToolCatalogEntry>>,
    aliases: Arc<HashMap<String, String>>,
}

impl std::fmt::Debug for ComposedToolCatalog {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ComposedToolCatalog")
            .field("profile", &self.profile)
            .field("entries", &self.entries.keys().collect::<Vec<_>>())
            .field("aliases", &self.aliases)
            .finish()
    }
}

impl ComposedToolCatalog {
    /// Compose a tool catalog from packs and a profile.
    pub fn compose(packs: &[ToolPack], profile: ToolProfile) -> Self {
        let enabled_packs = if profile.enabled_packs.is_empty() {
            None
        } else {
            Some(
                profile
                    .enabled_packs
                    .iter()
                    .map(|pack| pack.as_str())
                    .collect::<HashSet<_>>(),
            )
        };
        let enabled_tools = profile
            .enabled_tools
            .iter()
            .map(|tool| tool.as_str())
            .collect::<HashSet<_>>();
        let disabled_tools = profile
            .disabled_tools
            .iter()
            .map(|tool| tool.as_str())
            .collect::<HashSet<_>>();

        let mut entries = HashMap::new();
        let mut aliases = HashMap::new();

        for pack in packs {
            if enabled_packs
                .as_ref()
                .is_some_and(|allowed| !allowed.contains(pack.name.as_str()))
            {
                continue;
            }

            for tool in &pack.tools {
                if tool.spec.source == ToolSource::Mcp && !profile.include_mcp {
                    continue;
                }
                if tool.spec.level == ToolLevel::ControlPlane && !profile.include_control_plane {
                    continue;
                }
                if !tool.policy.enabled {
                    continue;
                }
                if disabled_tools.contains(tool.spec.name.as_str()) {
                    continue;
                }
                if !enabled_tools.is_empty() && !enabled_tools.contains(tool.spec.name.as_str()) {
                    continue;
                }

                let visible_modes = tool
                    .policy
                    .visible_in_modes
                    .clone()
                    .unwrap_or_else(|| tool.spec.visible_in_modes.clone());
                if tool.policy.visible_to_model
                    && !visible_modes.contains(&profile.model_permission_mode)
                {
                    continue;
                }

                let mut spec = tool.spec.clone();
                if let Some(permission) = tool.policy.permission_override {
                    spec.required_permission = permission;
                }
                if let Some(approval) = tool.policy.approval_required {
                    spec.prompt_approval = approval;
                }
                spec.visible_in_modes = visible_modes;

                let entry = ToolCatalogEntry {
                    spec: spec.clone(),
                    policy: tool.policy.clone(),
                    executor: Arc::clone(&tool.executor),
                };
                entries.insert(spec.name.clone(), entry);

                if profile.allow_aliases {
                    for alias in &tool.spec.aliases {
                        aliases.insert(alias.clone(), tool.spec.name.clone());
                    }
                }
            }
        }

        Self {
            profile,
            entries: Arc::new(entries),
            aliases: Arc::new(aliases),
        }
    }

    /// Compose an empty catalog.
    pub fn empty(profile: ToolProfile) -> Self {
        Self::compose(&[], profile)
    }

    pub fn profile(&self) -> &ToolProfile {
        &self.profile
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn names(&self) -> Vec<String> {
        let mut names = self.entries.keys().cloned().collect::<Vec<_>>();
        names.sort();
        names
    }

    pub fn canonical_name(&self, name: &str) -> Option<String> {
        if self.entries.contains_key(name) {
            Some(name.to_string())
        } else {
            self.aliases.get(name).cloned()
        }
    }

    pub fn spec(&self, name: &str) -> Option<&ToolSpec> {
        let canonical = self.canonical_name(name)?;
        self.entries.get(&canonical).map(|entry| &entry.spec)
    }

    pub fn provider_definitions(&self) -> Vec<Value> {
        let mut names = self.names();
        names.sort();
        let mut definitions = Vec::new();
        for name in names {
            if let Some(spec) = self.entries.get(&name).map(|entry| &entry.spec) {
                definitions.push(spec.provider_definition());
                if self.profile.allow_aliases {
                    for alias in &spec.aliases {
                        definitions
                            .push(spec.with_exposed_name(alias.clone()).provider_definition());
                    }
                }
            }
        }
        definitions
    }

    pub fn inventory(&self) -> Vec<ToolCatalogView> {
        let mut records = self
            .entries
            .values()
            .map(|entry| ToolCatalogView {
                name: entry.spec.name.clone(),
                pack: entry.spec.pack.clone(),
                source: entry.spec.source,
                level: entry.spec.level,
                schema: entry.spec.provider_definition(),
            })
            .collect::<Vec<_>>();
        records.sort_by(|a, b| a.pack.cmp(&b.pack).then_with(|| a.name.cmp(&b.name)));
        records
    }

    pub fn filter_by_name(&self, allowed: &[String]) -> Self {
        let allowed = allowed
            .iter()
            .map(|name| name.as_str())
            .collect::<HashSet<_>>();
        let entries = self
            .entries
            .iter()
            .filter(|(name, _)| allowed.contains(name.as_str()))
            .map(|(name, entry)| (name.clone(), entry.clone()))
            .collect::<HashMap<_, _>>();
        let aliases = self
            .aliases
            .iter()
            .filter(|(alias, canonical)| {
                allowed.contains(alias.as_str()) || allowed.contains(canonical.as_str())
            })
            .map(|(alias, canonical)| (alias.clone(), canonical.clone()))
            .collect::<HashMap<_, _>>();

        Self {
            profile: self.profile.clone(),
            entries: Arc::new(entries),
            aliases: Arc::new(aliases),
        }
    }

    pub async fn execute(&self, name: &str, input: Value) -> Result<ToolExecutionResult> {
        let canonical = self
            .canonical_name(name)
            .ok_or_else(|| AgentError::ToolNotFound(name.to_string()))?;
        let entry = self
            .entries
            .get(&canonical)
            .ok_or_else(|| AgentError::ToolNotFound(canonical.clone()))?;
        entry.executor.execute(&canonical, input).await
    }
}

/// Compatibility executor that wraps a legacy Tool trait object.
pub struct LegacyToolExecutor {
    tool: Arc<dyn Tool>,
}

impl LegacyToolExecutor {
    pub fn new(tool: Arc<dyn Tool>) -> Self {
        Self { tool }
    }
}

#[async_trait]
impl ToolExecutor for LegacyToolExecutor {
    async fn execute(&self, _canonical_name: &str, input: Value) -> Result<ToolExecutionResult> {
        let output = self.tool.execute(input).await?;
        Ok(ToolExecutionResult {
            output,
            is_error: false,
            metadata: None,
        })
    }
}

fn default_visible_modes(required_permission: PermissionMode) -> Vec<PermissionMode> {
    [
        PermissionMode::ReadOnly,
        PermissionMode::WorkspaceWrite,
        PermissionMode::DangerFullAccess,
        PermissionMode::Prompt,
        PermissionMode::Allow,
    ]
    .into_iter()
    .filter(|mode| *mode >= required_permission)
    .collect()
}

/// Build a tool spec from a legacy Tool implementation while migrating the platform.
pub fn legacy_tool_spec(
    tool: &Arc<dyn Tool>,
    pack: impl Into<String>,
    source: ToolSource,
    level: ToolLevel,
    required_permission: PermissionMode,
    tags: Vec<String>,
    aliases: Vec<String>,
) -> ToolSpec {
    let schema = tool.schema();
    ToolSpec {
        name: tool.name().to_string(),
        description: schema
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        input_schema: schema.get("parameters").cloned().unwrap_or(Value::Null),
        required_permission,
        source,
        level,
        tags,
        aliases,
        pack: pack.into(),
        prompt_approval: false,
        visible_in_modes: default_visible_modes(required_permission),
    }
}

/// Create a compatibility registration for a legacy Tool implementation.
pub fn legacy_registration(
    tool: Arc<dyn Tool>,
    pack: impl Into<String>,
    source: ToolSource,
    level: ToolLevel,
    required_permission: PermissionMode,
    tags: Vec<String>,
    aliases: Vec<String>,
) -> ToolRegistration {
    ToolRegistration {
        spec: legacy_tool_spec(
            &tool,
            pack,
            source,
            level,
            required_permission,
            tags,
            aliases,
        ),
        policy: ToolPolicy::default(),
        executor: Arc::new(LegacyToolExecutor::new(tool)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct EchoTool;

    #[async_trait]
    impl Tool for EchoTool {
        fn name(&self) -> &'static str {
            "echo"
        }

        fn schema(&self) -> &'static Value {
            Box::leak(Box::new(serde_json::json!({
                "name": "echo",
                "description": "echo",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "value": {"type": "string"}
                    }
                }
            })))
        }

        async fn execute(&self, input: Value) -> Result<String> {
            Ok(input
                .get("value")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string())
        }
    }

    #[tokio::test]
    async fn tool_platform_resolves_aliases_and_executes() {
        let tool = Arc::new(EchoTool) as Arc<dyn Tool>;
        let pack = ToolPack {
            name: "runtime".to_string(),
            tools: vec![legacy_registration(
                Arc::clone(&tool),
                "runtime",
                ToolSource::Runtime,
                ToolLevel::Primitive,
                PermissionMode::ReadOnly,
                vec!["test".to_string()],
                vec!["say".to_string()],
            )],
        };

        let catalog = ComposedToolCatalog::compose(&[pack], ToolProfile::default_root());
        let result = catalog
            .execute("say", serde_json::json!({"value": "hello"}))
            .await
            .expect("alias execution should succeed");

        assert_eq!(result.text_output(), "hello");
        assert!(catalog
            .provider_definitions()
            .iter()
            .any(|definition| { definition.get("name").and_then(Value::as_str) == Some("say") }));
    }

    #[test]
    fn tool_platform_filters_control_plane_tools_for_subagent_profile() {
        let tool = Arc::new(EchoTool) as Arc<dyn Tool>;
        let pack = ToolPack {
            name: "runtime".to_string(),
            tools: vec![ToolRegistration {
                spec: ToolSpec {
                    name: "planner".to_string(),
                    description: "planner".to_string(),
                    input_schema: Value::Null,
                    required_permission: PermissionMode::ReadOnly,
                    source: ToolSource::Runtime,
                    level: ToolLevel::ControlPlane,
                    tags: Vec::new(),
                    aliases: Vec::new(),
                    pack: "runtime".to_string(),
                    prompt_approval: false,
                    visible_in_modes: vec![PermissionMode::WorkspaceWrite],
                },
                policy: ToolPolicy::default(),
                executor: Arc::new(LegacyToolExecutor::new(tool)),
            }],
        };

        let catalog = ComposedToolCatalog::compose(&[pack], ToolProfile::default_subagent());
        assert!(catalog.is_empty());
    }
}
