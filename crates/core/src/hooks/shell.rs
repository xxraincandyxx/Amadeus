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
use std::io::Write;
use std::process::{Command, Stdio};

use crate::error::Result;
use crate::hooks::{Hook, HookAction, HookEvent};

struct ShellHookResult {
    exit_code: i32,
    success: bool,
    output: String,
}

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
            .unwrap_or("pre_tool_use")
        {
            "tool_start" | "pre_tool_use" => HookEvent::PreToolUse,
            "tool_complete" | "post_tool_use" => HookEvent::PostToolUse,
            "post_tool_use_failure" => HookEvent::PostToolUseFailure,
            _ => HookEvent::PreToolUse,
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
        event: HookEvent,
        tool_name: &str,
        tool_input: &Value,
        tool_output: Option<&str>,
        is_error: bool,
        duration_ms: Option<u64>,
    ) -> std::io::Result<ShellHookResult> {
        let tool_input_str = serde_json::to_string(tool_input).unwrap_or_default();
        let payload = serde_json::json!({
            "event": match event {
                HookEvent::PreToolUse => "pre_tool_use",
                HookEvent::PostToolUse => "post_tool_use",
                HookEvent::PostToolUseFailure => "post_tool_use_failure",
            },
            "tool_name": tool_name,
            "tool_input": tool_input,
            "tool_output": tool_output.unwrap_or(""),
            "is_error": is_error,
            "duration_ms": duration_ms.unwrap_or(0),
        });

        let mut child = Command::new("sh")
            .arg("-c")
            .arg(&self.command)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .env(
                "HOOK_EVENT",
                match event {
                    HookEvent::PreToolUse => "pre_tool_use",
                    HookEvent::PostToolUse => "post_tool_use",
                    HookEvent::PostToolUseFailure => "post_tool_use_failure",
                },
            )
            .env("HOOK_TOOL_NAME", tool_name)
            .env("HOOK_TOOL_INPUT", &tool_input_str)
            .env("HOOK_TOOL_OUTPUT", tool_output.unwrap_or(""))
            .env(
                "HOOK_TOOL_DURATION_MS",
                duration_ms.unwrap_or(0).to_string(),
            )
            .env("HOOK_TOOL_IS_ERROR", if is_error { "1" } else { "0" })
            .envs(&self.env)
            .spawn()?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(payload.to_string().as_bytes())?;
        }

        let output = child.wait_with_output()?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let combined = if stderr.is_empty() {
            stdout
        } else {
            format!("{}\n{}", stdout, stderr)
        };

        Ok(ShellHookResult {
            exit_code: output.status.code().unwrap_or(-1),
            success: output.status.success(),
            output: combined,
        })
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
        if self.event != HookEvent::PreToolUse {
            return Ok(HookAction::Continue);
        }

        let result = self
            .execute(HookEvent::PreToolUse, tool_name, input, None, false, None)
            .map_err(|e| {
                crate::error::AgentError::Command(format!("Hook execution failed: {}", e))
            })?;

        if result.exit_code == 2 {
            Ok(HookAction::Block(format!(
                "Hook '{}' blocked execution: {}",
                self.name, result.output
            )))
        } else {
            if !result.success && self.block_on_error {
                return Ok(HookAction::Block(format!(
                    "Hook '{}' blocked execution: {}",
                    self.name, result.output
                )));
            }
            if !result.success {
                tracing::warn!(
                    hook = %self.name,
                    output = %result.output.trim(),
                    exit_code = result.exit_code,
                    "Shell hook returned non-zero exit code"
                );
            } else if !result.output.trim().is_empty() {
                tracing::info!(hook = %self.name, output = %result.output.trim(), "Shell hook executed");
            }
            Ok(HookAction::Continue)
        }
    }

    async fn on_tool_complete(
        &self,
        tool_name: &str,
        input: &Value,
        output: &str,
        is_error: bool,
        duration_ms: u64,
    ) -> Result<()> {
        if (self.event == HookEvent::PostToolUse && is_error)
            || (self.event == HookEvent::PostToolUseFailure && !is_error)
            || self.event == HookEvent::PreToolUse
        {
            return Ok(());
        }

        let result = self
            .execute(
                self.event,
                tool_name,
                input,
                Some(output),
                is_error,
                Some(duration_ms),
            )
            .map_err(|e| {
                crate::error::AgentError::Command(format!("Hook execution failed: {}", e))
            })?;

        if !result.success {
            tracing::warn!(
                hook = %self.name,
                output = %result.output.trim(),
                exit_code = result.exit_code,
                "Shell hook returned non-zero exit code"
            );
        } else if !result.output.trim().is_empty() {
            tracing::info!(hook = %self.name, output = %result.output.trim(), "Shell hook executed");
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
            "event": "pre_tool_use",
            "command": "echo 'test'",
            "tools": ["bash"],
            "env": {
                "FOO": "bar"
            },
            "block_on_error": true
        });

        let hook = ShellHook::from_config(&config).unwrap();
        assert_eq!(hook.name, "test-hook");
        assert_eq!(hook.event, HookEvent::PreToolUse);
        assert_eq!(hook.command, "echo 'test'");
        assert_eq!(hook.tools, vec!["bash"]);
        assert_eq!(hook.env.get("FOO"), Some(&"bar".to_string()));
        assert!(hook.block_on_error);
    }

    #[test]
    fn test_shell_hook_matches_tool() {
        let hook = ShellHook::new("test", HookEvent::PreToolUse, "echo")
            .with_tools(vec!["bash".to_string(), "write_file".to_string()]);

        assert!(hook.matches_tool("bash"));
        assert!(hook.matches_tool("write_file"));
        assert!(!hook.matches_tool("read_file"));
    }

    #[tokio::test]
    async fn test_shell_hook_execute() {
        let hook = ShellHook::new("test", HookEvent::PreToolUse, "echo $HOOK_TOOL_NAME");

        let action = hook
            .on_tool_start("bash", &serde_json::json!({"command": "ls"}))
            .await
            .unwrap();

        assert!(matches!(action, HookAction::Continue));
    }

    #[tokio::test]
    async fn test_shell_hook_exit_code_two_blocks_pre_tool_use() {
        let hook = ShellHook::new("test", HookEvent::PreToolUse, "exit 2");

        let action = hook
            .on_tool_start("bash", &serde_json::json!({"command": "ls"}))
            .await
            .unwrap();

        assert!(matches!(action, HookAction::Block(_)));
    }
}
