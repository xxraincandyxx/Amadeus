// @amadeus-header
// summary: Module root for the hooks subsystem and its exports.
// layer: infra
// status: active
// feature_flags: none
// provides:
// - module: crate::hooks
// - type: crate::hooks::HookAction
// - type: crate::hooks::HookEvent
// - trait: crate::hooks::Hook
// - type: crate::hooks::HookRegistry
// uses:
// - module: crate::agent::config::Config
// - module: crate::error::Result
// - format: JSON values
// - artifact: filesystem paths and files
// invariants:
// - Module exports stay aligned with child modules and re-exports.
// side_effects:
// - Reads or writes filesystem state.
// tests:
// - tests/mod.rs
// @end-amadeus-header

//! # Hooks System
//!
//! Hooks provide extensibility points for the agent loop.
//!
//! ## Hook Events
//!
//! - `on_tool_start` - Called before a tool is executed
//! - `on_tool_complete` - Called after a tool completes
//!
//! ## Hook Actions
//!
//! Hooks can return different actions:
//! - `Continue` - Proceed normally
//! - `ModifyInput` - Change the tool input before execution
//! - `Block` - Prevent the tool from running
//!
//! ## Usage
//!
//! ```rust,ignore
//! use amadeus::hooks::{Hook, HookAction, HookRegistry};
//!
//! struct LoggingHook;
//!
//! #[async_trait::async_trait]
//! impl Hook for LoggingHook {
//!     fn name(&self) -> &str { "logging" }
//!
//!     async fn on_tool_start(&self, name: &str, input: &Value) -> Result<HookAction> {
//!         println!("Tool {} starting with input: {:?}", name, input);
//!         Ok(HookAction::Continue)
//!     }
//!
//!     async fn on_tool_complete(&self, name: &str, output: &str, duration_ms: u64) -> Result<()> {
//!         println!("Tool {} completed in {}ms", name, duration_ms);
//!         Ok(())
//!     }
//! }
//! ```

pub mod shell;

use async_trait::async_trait;
use serde_json::Value;
use std::path::Path;
use std::sync::Arc;

use crate::agent::config::Config;
use crate::error::Result;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookSource {
    Global,
    Workspace,
    Runtime,
}

/// Actions that a hook can return from `on_tool_start`.
#[derive(Debug, Clone)]
pub enum HookAction {
    /// Continue with normal execution.
    Continue,
    /// Modify the tool input before execution.
    ModifyInput(Value),
    /// Block the tool execution with a reason.
    Block(String),
}

/// Hook events that can trigger hooks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookEvent {
    /// Before a tool is executed.
    PreToolUse,
    /// After a tool completes successfully.
    PostToolUse,
    /// After a tool completes with an error.
    PostToolUseFailure,
}

impl HookEvent {
    pub fn title(&self) -> &'static str {
        match self {
            Self::PreToolUse => "PreToolUse",
            Self::PostToolUse => "PostToolUse",
            Self::PostToolUseFailure => "PostToolUseFailure",
        }
    }

    pub fn summary(&self) -> &'static str {
        match self {
            Self::PreToolUse => "Before tool execution",
            Self::PostToolUse => "After tool execution",
            Self::PostToolUseFailure => "After tool execution fails",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HookDescriptor {
    pub name: String,
    pub event: HookEvent,
    pub command: String,
    pub tools: Vec<String>,
    pub source: HookSource,
}

/// Trait for implementing hooks.
#[async_trait]
pub trait Hook: Send + Sync {
    /// Unique name for the hook.
    fn name(&self) -> &str;

    /// Called before a tool is executed.
    ///
    /// Returns a `HookAction` to control execution flow.
    async fn on_tool_start(&self, name: &str, input: &Value) -> Result<HookAction> {
        let _ = (name, input);
        Ok(HookAction::Continue)
    }

    /// Called after a tool completes.
    ///
    /// Can be used for logging, notifications, or side effects.
    async fn on_tool_complete(
        &self,
        name: &str,
        input: &Value,
        output: &str,
        is_error: bool,
        duration_ms: u64,
    ) -> Result<()> {
        let _ = (name, input, output, is_error, duration_ms);
        Ok(())
    }

    /// Filter to limit which tools this hook applies to.
    ///
    /// Returns `true` if the hook should be invoked for the given tool.
    fn matches_tool(&self, _tool_name: &str) -> bool {
        true // Default: matches all tools
    }
}

/// Registry for managing hooks.
#[derive(Clone, Default)]
pub struct HookRegistry {
    hooks: Vec<Arc<dyn Hook>>,
    descriptors: Vec<HookDescriptor>,
}

impl HookRegistry {
    /// Create a new empty hook registry.
    pub fn new() -> Self {
        Self {
            hooks: Vec::new(),
            descriptors: Vec::new(),
        }
    }

    /// Register a hook.
    pub fn register<H: Hook + 'static>(&mut self, hook: H) {
        self.hooks.push(Arc::new(hook));
    }

    /// Register an Arc hook.
    pub fn register_arc(&mut self, hook: Arc<dyn Hook>) {
        self.hooks.push(hook);
    }

    /// Load hooks from a configuration file.
    ///
    /// The file format is:
    /// ```json
    /// {
    ///   "hooks": [
    ///     {
    ///       "type": "shell",
    ///       "name": "pre-commit",
    ///       "event": "tool_start",
    ///       "command": "echo 'Tool {TOOL_NAME} starting'",
    ///       "tools": ["write_file", "edit_file"]
    ///     }
    ///   ]
    /// }
    /// ```
    pub fn load_from_file(path: &Path) -> Result<Self> {
        Self::load_from_file_with_source(path, HookSource::Runtime)
    }

    fn load_from_file_with_source(path: &Path, source: HookSource) -> Result<Self> {
        let content = std::fs::read_to_string(path).map_err(|e| {
            crate::error::AgentError::Config(format!(
                "Failed to read hooks file {}: {}",
                path.display(),
                e
            ))
        })?;

        let json: serde_json::Value = serde_json::from_str(&content).map_err(|e| {
            crate::error::AgentError::Config(format!(
                "Invalid JSON in hooks file {}: {}",
                path.display(),
                e
            ))
        })?;

        let mut registry = Self::new();

        if let Some(hooks) = json.get("hooks").and_then(|v| v.as_array()) {
            for hook_config in hooks {
                if let Some(hook_type) = hook_config.get("type").and_then(|v| v.as_str()) {
                    match hook_type {
                        "shell" => {
                            if let Ok(shell_hook) = shell::ShellHook::from_config(hook_config) {
                                registry.descriptors.push(HookDescriptor {
                                    name: shell_hook.name.clone(),
                                    event: shell_hook.event,
                                    command: shell_hook.command.clone(),
                                    tools: shell_hook.tools.clone(),
                                    source,
                                });
                                registry.register(shell_hook);
                            }
                        }
                        _ => {
                            tracing::warn!("Unknown hook type: {}", hook_type);
                        }
                    }
                }
            }
        }

        Ok(registry)
    }

    /// Load hooks from the global and workspace `.amadeus` roots.
    pub fn load_for_config(config: &Config) -> Result<Self> {
        let mut registry = Self::new();

        if let Some(global_hooks_path) = Config::global_hooks_path() {
            if global_hooks_path.exists() {
                registry.merge(Self::load_from_file_with_source(
                    &global_hooks_path,
                    HookSource::Global,
                )?);
            }
        }

        let workspace_hooks_path = config.workspace_hooks_path();
        if workspace_hooks_path.exists() {
            registry.merge(Self::load_from_file_with_source(
                &workspace_hooks_path,
                HookSource::Workspace,
            )?);
        }

        Ok(registry)
    }

    /// Merge another registry into this one, preserving registration order.
    pub fn merge(&mut self, other: Self) {
        self.hooks.extend(other.hooks);
        self.descriptors.extend(other.descriptors);
    }

    /// Invoke all hooks for `on_tool_start`.
    ///
    /// Returns the first `Block` action if any hook blocks,
    /// or the last `ModifyInput` action if any hook modifies,
    /// or `Continue` if all hooks allow.
    pub async fn on_tool_start(&self, name: &str, input: &Value) -> Result<HookAction> {
        let mut current_input = input.clone();
        let mut action = HookAction::Continue;

        for hook in &self.hooks {
            if !hook.matches_tool(name) {
                continue;
            }

            match hook.on_tool_start(name, &current_input).await? {
                HookAction::Continue => {}
                HookAction::ModifyInput(new_input) => {
                    current_input = new_input;
                    action = HookAction::ModifyInput(current_input.clone());
                }
                HookAction::Block(reason) => {
                    return Ok(HookAction::Block(reason));
                }
            }
        }

        Ok(action)
    }

    /// Invoke all hooks for `on_tool_complete`.
    pub async fn on_tool_complete(
        &self,
        name: &str,
        input: &Value,
        output: &str,
        is_error: bool,
        duration_ms: u64,
    ) -> Result<()> {
        for hook in &self.hooks {
            if !hook.matches_tool(name) {
                continue;
            }
            hook.on_tool_complete(name, input, output, is_error, duration_ms)
                .await?;
        }
        Ok(())
    }

    /// Check if there are any hooks registered.
    pub fn is_empty(&self) -> bool {
        self.hooks.is_empty()
    }

    /// Get the number of registered hooks.
    pub fn len(&self) -> usize {
        self.hooks.len()
    }

    /// Get hook names.
    pub fn names(&self) -> Vec<&str> {
        self.hooks.iter().map(|h| h.name()).collect()
    }

    pub fn descriptors(&self) -> &[HookDescriptor] {
        &self.descriptors
    }

    pub fn descriptors_for_event(&self, event: HookEvent) -> Vec<HookDescriptor> {
        self.descriptors
            .iter()
            .filter(|descriptor| descriptor.event == event)
            .cloned()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestHook;

    #[async_trait]
    impl Hook for TestHook {
        fn name(&self) -> &str {
            "test"
        }

        async fn on_tool_start(&self, name: &str, _input: &Value) -> Result<HookAction> {
            if name == "blocked_tool" {
                Ok(HookAction::Block("Test block".to_string()))
            } else {
                Ok(HookAction::Continue)
            }
        }
    }

    #[tokio::test]
    async fn test_hook_registry() {
        let mut registry = HookRegistry::new();
        registry.register(TestHook);

        // Should continue for allowed tool
        let action = registry
            .on_tool_start("bash", &serde_json::json!({"command": "ls"}))
            .await
            .unwrap();
        assert!(matches!(action, HookAction::Continue));

        // Should block for blocked tool
        let action = registry
            .on_tool_start("blocked_tool", &serde_json::json!({}))
            .await
            .unwrap();
        assert!(matches!(action, HookAction::Block(_)));
    }

    #[test]
    fn test_load_for_config_merges_global_and_workspace_hooks() {
        let temp = tempfile::tempdir().unwrap();
        let fake_home = temp.path().join("home");
        let workdir = temp.path().join("workspace");
        let global_root = fake_home.join(".amadeus");
        let workspace_root = workdir.join(".amadeus");
        std::fs::create_dir_all(&global_root).unwrap();
        std::fs::create_dir_all(&workspace_root).unwrap();
        std::fs::write(
            global_root.join("hook.json"),
            r#"{"hooks":[{"type":"shell","name":"global","event":"pre_tool_use","command":"true"}]}"#,
        )
        .unwrap();
        std::fs::write(
            workspace_root.join("hook.json"),
            r#"{"hooks":[{"type":"shell","name":"workspace","event":"post_tool_use","command":"true"}]}"#,
        )
        .unwrap();

        let previous_home = std::env::var("HOME").ok();
        std::env::set_var("HOME", &fake_home);

        let config = Config {
            workdir: workdir.clone(),
            ..Config::default()
        };
        let registry = HookRegistry::load_for_config(&config).unwrap();

        assert_eq!(registry.len(), 2);
        assert_eq!(registry.names(), vec!["global", "workspace"]);
        assert_eq!(registry.descriptors().len(), 2);
        assert_eq!(registry.descriptors()[0].source, HookSource::Global);
        assert_eq!(registry.descriptors()[1].source, HookSource::Workspace);
        assert_eq!(registry.descriptors()[0].event, HookEvent::PreToolUse);
        assert_eq!(registry.descriptors()[1].event, HookEvent::PostToolUse);

        match previous_home {
            Some(value) => std::env::set_var("HOME", value),
            None => std::env::remove_var("HOME"),
        }
    }
}
