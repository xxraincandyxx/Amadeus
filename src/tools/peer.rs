//! # Peer Help Tool
//!
//! Allows agents to request assistance from other workers in the swarm.

use async_trait::async_trait;
use serde_json::{json, Value};
use tokio::sync::{mpsc, oneshot};
use tracing::{debug, info};

#[cfg(feature = "supervisor")]
use crate::agent::worker::{HelpRequest, Task};
use crate::core::id::AgentId;
use crate::error::{AgentError, Result};
use crate::tools::tool_trait::Tool;

/// A tool that delegates tasks to other agents.
#[cfg(feature = "supervisor")]
pub struct PeerTool {
    requester_id: AgentId,
    help_tx: mpsc::Sender<HelpRequest>,
    schema: Value,
}

#[cfg(feature = "supervisor")]
impl PeerTool {
    /// Create a new PeerTool.
    pub fn new(requester_id: AgentId, help_tx: mpsc::Sender<HelpRequest>) -> Self {
        let schema = json!({
            "name": "call_peer",
            "description": "Delegate a sub-task to another agent with specific capabilities.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "task": {
                        "type": "string",
                        "description": "Clear instruction for the peer agent"
                    },
                    "capabilities": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "List of required capabilities (e.g., ['bash', 'search'])"
                    }
                },
                "required": ["task"]
            }
        });

        Self {
            requester_id,
            help_tx,
            schema,
        }
    }
}

#[cfg(feature = "supervisor")]
#[async_trait]
impl Tool for PeerTool {
    fn name(&self) -> &'static str {
        "call_peer"
    }

    fn schema(&self) -> &'static Value {
        // We use a Box::leak to provide a 'static reference since schemas are constant per tool
        Box::leak(Box::new(self.schema.clone()))
    }

    async fn execute(&self, input: Value) -> Result<String> {
        let task_prompt = input["task"]
            .as_str()
            .ok_or_else(|| AgentError::ToolInput {
                tool: "call_peer".to_string(),
                reason: "Missing 'task' field".to_string(),
            })?;

        let capabilities: Vec<String> = input["capabilities"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        info!(
            requester = %self.requester_id,
            task = %task_prompt,
            "Requesting peer help"
        );

        let (response_tx, response_rx) = oneshot::channel();
        
        let task = Task::new(format!("subtask-{}", uuid::Uuid::new_v4()), task_prompt)
            .requires(capabilities);

        let help_request = HelpRequest {
            task,
            response_tx,
            requester_id: self.requester_id,
        };

        if let Err(e) = self.help_tx.send(help_request).await {
            return Err(AgentError::Command(format!("Failed to contact supervisor: {}", e)));
        }

        debug!("Waiting for peer response...");
        let result = response_rx.await.map_err(|_| {
            AgentError::Command("Peer response channel closed".to_string())
        })?;

        if result.success {
            Ok(result.output.unwrap_or_else(|| "Task completed with no output".to_string()))
        } else {
            Err(AgentError::Command(format!(
                "Peer task failed: {}", 
                result.error.unwrap_or_else(|| "Unknown error".to_string())
            )))
        }
    }
}
