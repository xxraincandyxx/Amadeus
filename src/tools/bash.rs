//! # Bash Tool
//!
//! Execute shell commands with timeout, blocklist, and output truncation.
//!
//! ## Features
//!
//! - Async execution using `tokio::process::Command`
//! - Configurable timeout (returns `AgentError::Timeout`)
//! - Working directory support
//! - Combined stdout + stderr capture
//! - Command blocklist for security
//! - Output truncation to prevent context overflow

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::Value;
use tokio::process::Command;
use tokio::time::{timeout, Duration};

use crate::agent::config::Config;
use crate::error::{AgentError, Result};
use crate::tools::schema::bash_tool;
use crate::tools::tool_trait::Tool;

#[derive(Debug, Clone, Deserialize)]
pub struct BashInput {
    pub command: String,
}

pub struct BashTool {
    timeout_secs: u64,
    workdir: String,
    blocked_commands: Vec<String>,
    max_output_bytes: usize,
}

impl BashTool {
    pub fn from_config(config: &Config) -> Self {
        Self {
            timeout_secs: config.timeout_seconds,
            workdir: config.workdir.to_string_lossy().to_string(),
            blocked_commands: config.blocked_commands.clone(),
            max_output_bytes: config.max_output_bytes,
        }
    }

    pub fn new(
        timeout_secs: u64,
        workdir: String,
        blocked_commands: Vec<String>,
        max_output_bytes: usize,
    ) -> Self {
        Self {
            timeout_secs,
            workdir,
            blocked_commands,
            max_output_bytes,
        }
    }

    fn is_blocked(&self, command: &str) -> bool {
        self.blocked_commands.iter().any(|blocked| {
            if blocked == "rm -rf /" {
                command == "rm -rf /"
                    || command.starts_with("rm -rf / ")
                    || command.contains("; rm -rf /")
            } else {
                command.contains(blocked)
            }
        })
    }

    fn truncate_output(&self, output: String) -> String {
        if output.len() > self.max_output_bytes {
            let truncated = &output[..self.max_output_bytes];
            format!(
                "{}\n\n... (truncated {} bytes)",
                truncated,
                output.len() - self.max_output_bytes
            )
        } else {
            output
        }
    }

    async fn execute_with_timeout(&self, cmd: &str) -> Result<String> {
        let duration = Duration::from_secs(self.timeout_secs);

        let output = async {
            let result = Command::new("sh")
                .arg("-c")
                .arg(cmd)
                .current_dir(&self.workdir)
                .kill_on_drop(true)
                .output()
                .await?;

            let stdout = String::from_utf8_lossy(&result.stdout).to_string();
            let stderr = String::from_utf8_lossy(&result.stderr).to_string();

            Ok(format!("{}{}", stdout, stderr))
        };

        match timeout(duration, output).await {
            Ok(result) => result,
            Err(_) => Err(AgentError::Timeout(self.timeout_secs)),
        }
    }
}

#[async_trait]
impl Tool for BashTool {
    fn name(&self) -> &'static str {
        "bash"
    }

    fn schema(&self) -> &'static Value {
        bash_tool()
    }

    async fn execute(&self, input: Value) -> Result<String> {
        let parsed: BashInput =
            serde_json::from_value(input).map_err(|e| AgentError::ToolInput {
                tool: "bash".to_string(),
                reason: e.to_string(),
            })?;

        if self.is_blocked(&parsed.command) {
            return Err(AgentError::CommandBlocked(parsed.command));
        }

        let output = self.execute_with_timeout(&parsed.command).await?;
        Ok(self.truncate_output(output))
    }
}
