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

use crate::error::Result;

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
    ToolStart,
    /// After a tool completes.
    ToolComplete,
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
    async fn on_tool_complete(&self, name: &str, output: &str, duration_ms: u64) -> Result<()> {
        let _ = (name, output, duration_ms);
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
}

impl HookRegistry {
    /// Create a new empty hook registry.
    pub fn new() -> Self {
        Self { hooks: Vec::new() }
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
    pub async fn on_tool_complete(&self, name: &str, output: &str, duration_ms: u64) -> Result<()> {
        for hook in &self.hooks {
            if !hook.matches_tool(name) {
                continue;
            }
            hook.on_tool_complete(name, output, duration_ms).await?;
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
}
