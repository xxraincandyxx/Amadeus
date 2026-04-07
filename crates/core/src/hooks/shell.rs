// @amadeus-header
// summary: Source file for shell.
// layer: infra
// status: active
// feature_flags: none
// provides:
// - module: crate::hooks::shell
// - type: crate::hooks::shell::ShellHook
// uses:
// - module: crate::error::Result
// - module: crate::hooks
// - format: JSON values
// invariants:
// - Listed interfaces stay aligned with the implementation in this file.
// side_effects:
// - Runs external commands or subprocesses.
// tests:
// - cmd: cargo test --features full
// @end-amadeus-header

//! # Shell Hook
//!
//! Execute shell commands as hooks.
//!
//! ## Configuration
//!
//! ```json
//! {
//!   "type": "shell",
//!   "name": "pre-commit",
//!   "event": "tool_start",
//!   "command": "pre-commit run --files {FILES}",
//!   "tools": ["write_file", "edit_file"],
//!   "env": {
//!     "CUSTOM_VAR": "value"
//!   }
//! }
//! ```
//!
//! ## Environment Variables
//!
//! The following environment variables are available in the command:
//! - `TOOL_NAME` - The name of the tool being invoked
//! - `TOOL_INPUT` - JSON string of the tool input
//! - `TOOL_OUTPUT` - The tool output (only for tool_complete events)
//! - `TOOL_DURATION_MS` - Duration in milliseconds (only for tool_complete events)

use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use std::process::Command;

use crate::error::Result;
use crate::hooks::{Hook, HookAction};

/// A hook that executes a shell command.
pub struct ShellHook {
    /// Unique name for this hook.
    pub name: String,
    /// The event this hook triggers on.
    pub event: super::HookEvent,
    /// The shell command to execute.
    pub command: String,
    /// Tools to match (empty = all tools).
    pub tools: Vec<String>,
    /// Additional environment variables.
    pub env: HashMap<String, String>,
    /// Whether to block on non-zero exit.
    pub block_on_error: bool,
}

impl ShellHook {
    /// Create a new shell hook.
    pub fn new(
        name: impl Into<String>,
        event: super::HookEvent,
        command: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            event,
            command: command.into(),
            tools: Vec::new(),
            env: HashMap::new(),
            block_on_error: false,
        }
    }

    /// Set the tools filter.
    pub fn with_tools(mut self, tools: Vec<String>) -> Self {
        self.tools = tools;
        self
    }

    /// Set additional environment variables.
    pub fn with_env(mut self, env: HashMap<String, String>) -> Self {
        self.env = env;
        self
    }

    /// Set whether to block on non-zero exit.
    pub fn with_block_on_error(mut self, block: bool) -> Self {
        self.block_on_error = block;
        self
    }

    /// Load from a JSON configuration.
    pub fn from_config(config: &Value) -> Result<Self> {
        let name = config
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("shell_hook")
            .to_string();

        let event = match config
            .get("event")
            .and_then(|v| v.as_str())
            .unwrap_or("tool_start")
        {
            "tool_complete" => super::HookEvent::ToolComplete,
            _ => super::HookEvent::ToolStart,
        };

        let command = config
            .get("command")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let tools = config
            .get("tools")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        let env = config
            .get("env")
            .and_then(|v| v.as_object())
            .map(|obj| {
                obj.iter()
                    .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                    .collect()
            })
            .unwrap_or_default();

        let block_on_error = config
            .get("block_on_error")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        Ok(Self {
            name,
            event,
            command,
            tools,
            env,
            block_on_error,
        })
    }

    /// Execute the shell command.
    fn execute(
        &self,
        tool_name: &str,
        tool_input: &Value,
        tool_output: Option<&str>,
        duration_ms: Option<u64>,
    ) -> std::io::Result<(bool, String)> {
        let tool_input_str = serde_json::to_string(tool_input).unwrap_or_default();

        let output = Command::new("sh")
            .arg("-c")
            .arg(&self.command)
            .env("TOOL_NAME", tool_name)
            .env("TOOL_INPUT", &tool_input_str)
            .env("TOOL_OUTPUT", tool_output.unwrap_or(""))
            .env("TOOL_DURATION_MS", duration_ms.unwrap_or(0).to_string())
            .envs(&self.env)
            .output()?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let combined = if stderr.is_empty() {
            stdout
        } else {
            format!("{}\n{}", stdout, stderr)
        };

        Ok((output.status.success(), combined))
    }
}

#[async_trait]
impl Hook for ShellHook {
    fn name(&self) -> &str {
        &self.name
    }

    fn matches_tool(&self, tool_name: &str) -> bool {
        if self.tools.is_empty() {
            return true;
        }
        self.tools.iter().any(|t| t == tool_name)
    }

    async fn on_tool_start(&self, tool_name: &str, input: &Value) -> Result<HookAction> {
        if self.event != super::HookEvent::ToolStart {
            return Ok(HookAction::Continue);
        }

        let (success, output) = self.execute(tool_name, input, None, None).map_err(|e| {
            crate::error::AgentError::Command(format!("Hook execution failed: {}", e))
        })?;

        if !success && self.block_on_error {
            Ok(HookAction::Block(format!(
                "Hook '{}' blocked execution: {}",
                self.name, output
            )))
        } else {
            if !output.trim().is_empty() {
                tracing::info!(hook = %self.name, output = %output.trim(), "Shell hook executed");
            }
            Ok(HookAction::Continue)
        }
    }

    async fn on_tool_complete(
        &self,
        tool_name: &str,
        output: &str,
        duration_ms: u64,
    ) -> Result<()> {
        if self.event != super::HookEvent::ToolComplete {
            return Ok(());
        }

        let input = serde_json::json!({});
        let (success, cmd_output) = self
            .execute(tool_name, &input, Some(output), Some(duration_ms))
            .map_err(|e| {
                crate::error::AgentError::Command(format!("Hook execution failed: {}", e))
            })?;

        if !success {
            tracing::warn!(
                hook = %self.name,
                output = %cmd_output.trim(),
                "Shell hook returned non-zero exit code"
            );
        } else if !cmd_output.trim().is_empty() {
            tracing::info!(hook = %self.name, output = %cmd_output.trim(), "Shell hook executed");
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shell_hook_from_config() {
        let config = serde_json::json!({
            "type": "shell",
            "name": "test-hook",
            "event": "tool_start",
            "command": "echo 'test'",
            "tools": ["bash"],
            "env": {
                "FOO": "bar"
            },
            "block_on_error": true
        });

        let hook = ShellHook::from_config(&config).unwrap();
        assert_eq!(hook.name, "test-hook");
        assert_eq!(hook.event, super::super::HookEvent::ToolStart);
        assert_eq!(hook.command, "echo 'test'");
        assert_eq!(hook.tools, vec!["bash"]);
        assert_eq!(hook.env.get("FOO"), Some(&"bar".to_string()));
        assert!(hook.block_on_error);
    }

    #[test]
    fn test_shell_hook_matches_tool() {
        let hook = ShellHook::new("test", super::super::HookEvent::ToolStart, "echo")
            .with_tools(vec!["bash".to_string(), "write_file".to_string()]);

        assert!(hook.matches_tool("bash"));
        assert!(hook.matches_tool("write_file"));
        assert!(!hook.matches_tool("read_file"));
    }

    #[tokio::test]
    async fn test_shell_hook_execute() {
        let hook = ShellHook::new(
            "test",
            super::super::HookEvent::ToolStart,
            "echo $TOOL_NAME",
        );

        let action = hook
            .on_tool_start("bash", &serde_json::json!({"command": "ls"}))
            .await
            .unwrap();

        assert!(matches!(action, HookAction::Continue));
    }
}
