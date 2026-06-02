// @amadeus-header
// summary: Command execution boundary with conservative sandbox profile handling.
// layer: policy
// status: active
// feature_flags: none
// provides:
// - module: crate::security::command_runner
// - type: crate::security::command_runner::CommandRunner
// - type: crate::security::command_runner::CommandRequest
// - type: crate::security::command_runner::CommandResult
// - type: crate::security::command_runner::SandboxProfile
// uses:
// - module: crate::error
// - module: crate::permissions
// - runtime: tokio async runtime
// invariants:
// - Read-only degraded execution never trusts shell heuristics as a write barrier.
// side_effects:
// - Runs external commands or subprocesses.
// tests:
// - cmd: cargo test -p core security --features full
// @end-amadeus-header

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tokio::time::timeout;

use crate::error::{AgentError, Result};
use crate::permissions::PermissionMode;

#[derive(Debug, Clone)]
pub enum SandboxProfile {
    ReadOnly {
        readable_roots: Vec<PathBuf>,
    },
    WorkspaceWrite {
        readable_roots: Vec<PathBuf>,
        writable_roots: Vec<PathBuf>,
    },
    DangerFullAccess,
}

#[derive(Debug, Clone)]
pub struct CommandRequest {
    pub command: String,
    pub cwd: PathBuf,
    pub permission_mode: PermissionMode,
    pub sandbox: SandboxProfile,
    pub timeout: Duration,
    pub max_output_bytes: usize,
    pub env: HashMap<String, String>,
    pub stdin: Option<Vec<u8>>,
}

#[derive(Debug, Clone)]
pub struct CommandResult {
    pub output: String,
    pub exit_code: i32,
    pub timed_out: bool,
}

#[derive(Debug, Clone, Default)]
pub struct CommandRunner;

impl CommandRunner {
    pub fn new() -> Self {
        Self
    }

    pub async fn run(&self, request: CommandRequest) -> Result<CommandResult> {
        if matches!(request.permission_mode, PermissionMode::ReadOnly)
            && !safe_degraded_read_only_command(&request.command)
        {
            return Err(AgentError::PermissionDenied {
                tool: "bash".to_string(),
                active_mode: PermissionMode::ReadOnly.as_str().to_string(),
                required_mode: PermissionMode::WorkspaceWrite.as_str().to_string(),
                reason: format!(
                    "read-only mode denied unsandboxed command: {}",
                    request.command
                ),
            });
        }

        let mut child = Command::new("sh")
            .arg("-c")
            .arg(&request.command)
            .current_dir(&request.cwd)
            .kill_on_drop(true)
            .stdin(if request.stdin.is_some() {
                std::process::Stdio::piped()
            } else {
                std::process::Stdio::null()
            })
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .envs(&request.env)
            .spawn()?;

        if let Some(stdin) = request.stdin {
            if let Some(mut child_stdin) = child.stdin.take() {
                child_stdin.write_all(&stdin).await?;
            }
        }

        let output = match timeout(request.timeout, child.wait_with_output()).await {
            Ok(output) => output?,
            Err(_) => {
                return Ok(CommandResult {
                    output: format!("Command timed out after {}s", request.timeout.as_secs()),
                    exit_code: -1,
                    timed_out: true,
                });
            }
        };

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let combined = truncate_output(format!("{}{}", stdout, stderr), request.max_output_bytes);

        Ok(CommandResult {
            output: combined,
            exit_code: output.status.code().unwrap_or(-1),
            timed_out: false,
        })
    }
}

pub fn safe_degraded_read_only_command(command: &str) -> bool {
    if contains_shell_metacharacter(command) {
        return false;
    }

    let mut parts = command.split_whitespace();
    let first = parts
        .next()
        .unwrap_or_default()
        .rsplit('/')
        .next()
        .unwrap_or_default();

    matches!(
        first,
        "cat"
            | "head"
            | "tail"
            | "wc"
            | "ls"
            | "find"
            | "grep"
            | "rg"
            | "pwd"
            | "date"
            | "whoami"
            | "file"
            | "stat"
            | "diff"
            | "sort"
            | "uniq"
            | "cut"
            | "printf"
            | "true"
            | "false"
    )
}

fn contains_shell_metacharacter(command: &str) -> bool {
    command.chars().any(|ch| {
        matches!(
            ch,
            '|' | '&' | ';' | '<' | '>' | '$' | '`' | '\n' | '\r' | '(' | ')' | '{' | '}'
        )
    })
}

fn truncate_output(output: String, max_output_bytes: usize) -> String {
    if output.len() > max_output_bytes {
        let mut boundary = max_output_bytes.min(output.len());
        while boundary > 0 && !output.is_char_boundary(boundary) {
            boundary -= 1;
        }
        format!(
            "{}\n\n... (truncated {} bytes)",
            &output[..boundary],
            output.len() - boundary
        )
    } else {
        output
    }
}

#[cfg(test)]
mod tests {
    use super::safe_degraded_read_only_command;

    #[test]
    fn read_only_degraded_rejects_interpreters_and_redirection() {
        assert!(!safe_degraded_read_only_command(
            "python -c 'open(\"x\",\"w\").write(\"y\")'"
        ));
        assert!(!safe_degraded_read_only_command("cat Cargo.toml > out"));
        assert!(safe_degraded_read_only_command("cat Cargo.toml"));
    }
}
