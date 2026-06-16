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
use std::path::PathBuf;
use std::time::Duration;

use crate::error::Result;
use crate::hooks::{Hook, HookAction, HookEvent};
use crate::permissions::PermissionMode;
use crate::security::{CommandRequest, CommandRunner, SandboxProfile};

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
    pub timeout_secs: u64,
    pub max_output_bytes: usize,
    pub workdir: PathBuf,
    pub permission_mode: PermissionMode,
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
            timeout_secs: 10,
            max_output_bytes: 65_536,
            workdir: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            permission_mode: PermissionMode::DangerFullAccess,
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
        let timeout_secs = config
            .get("timeout_seconds")
            .and_then(|v| v.as_u64())
            .unwrap_or(10);
        let max_output_bytes = config
            .get("max_output_bytes")
            .and_then(|v| v.as_u64())
            .and_then(|value| usize::try_from(value).ok())
            .unwrap_or(65_536);

        Ok(Self {
            name,
            event,
            command,
            tools,
            env,
            block_on_error,
            timeout_secs,
            max_output_bytes,
            workdir: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            permission_mode: PermissionMode::DangerFullAccess,
        })
    }

    /// Execute the shell command.
    async fn execute(
        &self,
        event: HookEvent,
        tool_name: &str,
        tool_input: &Value,
        tool_output: Option<&str>,
        is_error: bool,
        duration_ms: Option<u64>,
    ) -> Result<ShellHookResult> {
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

        let mut env = self.env.clone();
        env.insert("HOOK_EVENT".to_string(), hook_event_name(event).to_string());
        env.insert("HOOK_TOOL_NAME".to_string(), tool_name.to_string());
        env.insert("HOOK_TOOL_INPUT".to_string(), tool_input_str);
        env.insert(
            "HOOK_TOOL_OUTPUT".to_string(),
            tool_output.unwrap_or("").to_string(),
        );
        env.insert(
            "HOOK_TOOL_DURATION_MS".to_string(),
            duration_ms.unwrap_or(0).to_string(),
        );
        env.insert(
            "HOOK_TOOL_IS_ERROR".to_string(),
            if is_error { "1" } else { "0" }.to_string(),
        );

        let result = CommandRunner::new()
            .run(CommandRequest {
                command: self.command.clone(),
                cwd: self.workdir.clone(),
                permission_mode: self.permission_mode,
                sandbox: SandboxProfile::DangerFullAccess,
                timeout: Duration::from_secs(self.timeout_secs),
                max_output_bytes: self.max_output_bytes,
                env,
                stdin: Some(payload.to_string().into_bytes()),
            })
            .await?;

        Ok(ShellHookResult {
            exit_code: result.exit_code,
            success: result.exit_code == 0 && !result.timed_out,
            output: result.output,
        })
    }
}

fn hook_event_name(event: HookEvent) -> &'static str {
    match event {
        HookEvent::PreToolUse => "pre_tool_use",
        HookEvent::PostToolUse => "post_tool_use",
        HookEvent::PostToolUseFailure => "post_tool_use_failure",
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
            .await?;

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
            .await?;

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
