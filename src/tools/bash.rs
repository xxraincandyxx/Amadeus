use tokio::process::Command;
use tokio::time::{timeout, Duration};
use futures::future::join_all;
use crate::error::{Result, AgentError};
use crate::agent::messages::ToolInput;

pub struct BashTool {
    timeout_secs: u64,
    workdir: String,
}

impl BashTool {
    pub fn new(timeout_secs: u64, workdir: String) -> Self {
        Self {
            timeout_secs,
            workdir,
        }
    }

    pub async fn execute(&self, input: &ToolInput) -> Result<String> {
        self.execute_with_timeout(&input.command).await
    }

    async fn execute_with_timeout(&self, cmd: &str) -> Result<String> {
        let duration = Duration::from_secs(self.timeout_secs);

        let output = async {
            let result = Command::new("sh")
                .arg("-c")
                .arg(cmd)
                .current_dir(&self.workdir)
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

    // Concurrent execution for multiple independent commands
    pub async fn execute_all(&self, inputs: Vec<ToolInput>) -> Vec<Result<String>> {
        let futures = inputs
            .into_iter()
            .map(|input| {
                let cmd = input.command.clone();
                let tool = BashTool::new(self.timeout_secs, self.workdir.clone());
                async move {
                    tool.execute_with_timeout(&cmd).await
                }
            })
            .collect::<Vec<_>>();

        join_all(futures).await
    }
}
