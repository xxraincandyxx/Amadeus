use serde::{Deserialize, Serialize};

use crate::agent::compaction::CompressionStatus;
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RunResult {
    pub text: String,
    pub tool_calls: Vec<ToolCall>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub name: String,
    pub input: Value,
    pub output: String,
    pub is_error: bool,
}

/// Decision from approval request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalDecision {
    /// Approve this single execution.
    Approve,
    /// Deny this execution.
    Deny,
    /// Approve and add to auto-approve list for future executions.
    AlwaysApprove,
}

/// Request for tool approval (serializable for logging/display).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalRequest {
    /// Unique ID for this approval request.
    pub id: String,
    /// Tool name that requires approval.
    pub tool: String,
    /// Tool input that will be executed if approved.
    pub input: Value,
    /// Reason why approval is needed.
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AgentEvent {
    TextDelta {
        delta: String,
    },
    /// Extended thinking/reasoning content from the model
    ThinkingDelta {
        delta: String,
    },
    /// Thinking block completed
    ThinkingComplete {
        thinking: String,
    },
    ToolStart {
        id: String,
        name: String,
        command: Option<String>,
        parent_id: Option<String>,
    },
    ToolInputDelta {
        id: String,
        delta: String,
        parent_id: Option<String>,
    },
    ToolOutputDelta {
        id: String,
        delta: String,
        parent_id: Option<String>,
    },
    ToolComplete {
        id: String,
        name: String,
        input: Value,
        output: String,
        is_error: bool,
        parent_id: Option<String>,
    },
    /// Approval is required before tool execution.
    /// The consumer (TUI/Platform) must respond via Agent::send_approval_decision().
    ApprovalRequired {
        /// The approval request details.
        request: ApprovalRequest,
    },
    /// A sub-agent has been requested. The UI should spawn a sub-session and
    /// complete it via Agent::complete_subagent.
    SubAgentRequested {
        id: String,
        prompt: String,
        depth: usize,
    },
    /// Token usage update from the LLM.
    TokenUsage {
        input_tokens: u32,
        output_tokens: u32,
        total_tokens: u32,
    },
    /// Progress update for a long-running tool.
    ToolProgress {
        id: String,
        message: String,
        percent: Option<u8>,
        parent_id: Option<String>,
    },
    /// Context compaction occurred to manage context window.
    Compaction {
        /// Original message count.
        original_count: usize,
        /// Message count after compaction.
        compacted_count: usize,
        /// Estimated tokens saved.
        tokens_saved: usize,
        /// Number of messages summarized.
        messages_summarized: usize,
        /// Outcome status of the compaction.
        status: CompressionStatus,
    },
    Done {
        result: RunResult,
    },
    Error {
        message: String,
    },
    SessionSaved {
        path: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_run_result_default() {
        let result = RunResult::default();
        assert!(result.text.is_empty());
        assert!(result.tool_calls.is_empty());
    }

    #[test]
    fn test_tool_call_creation() {
        let tool_call = ToolCall {
            name: "bash".to_string(),
            input: serde_json::json!({"command": "ls"}),
            output: "file1\nfile2".to_string(),
            is_error: false,
        };

        assert_eq!(tool_call.name, "bash");
        assert!(!tool_call.is_error);
    }

    #[test]
    fn test_approval_decision_variants() {
        assert_ne!(ApprovalDecision::Approve, ApprovalDecision::Deny);
        assert_ne!(ApprovalDecision::Approve, ApprovalDecision::AlwaysApprove);
        assert_ne!(ApprovalDecision::Deny, ApprovalDecision::AlwaysApprove);
    }

    #[test]
    fn test_approval_request_creation() {
        let request = ApprovalRequest {
            id: "req_123".to_string(),
            tool: "bash".to_string(),
            input: serde_json::json!({"command": "rm -rf /"}),
            reason: "Dangerous command".to_string(),
        };

        assert_eq!(request.id, "req_123");
        assert_eq!(request.tool, "bash");
    }

    #[test]
    fn test_agent_event_text_delta() {
        let event = AgentEvent::TextDelta {
            delta: "Hello".to_string(),
        };

        match event {
            AgentEvent::TextDelta { delta } => assert_eq!(delta, "Hello"),
            _ => panic!("Expected TextDelta"),
        }
    }

    #[test]
    fn test_agent_event_thinking_delta() {
        let event = AgentEvent::ThinkingDelta {
            delta: "Thinking...".to_string(),
        };

        match event {
            AgentEvent::ThinkingDelta { delta } => assert_eq!(delta, "Thinking..."),
            _ => panic!("Expected ThinkingDelta"),
        }
    }

    #[test]
    fn test_agent_event_thinking_complete() {
        let event = AgentEvent::ThinkingComplete {
            thinking: "Full thinking content".to_string(),
        };

        match event {
            AgentEvent::ThinkingComplete { thinking } => {
                assert_eq!(thinking, "Full thinking content")
            }
            _ => panic!("Expected ThinkingComplete"),
        }
    }

    #[test]
    fn test_agent_event_tool_start() {
        let event = AgentEvent::ToolStart {
            id: "tool_1".to_string(),
            name: "bash".to_string(),
            command: Some("cargo test".to_string()),
            parent_id: None,
        };

        match event {
            AgentEvent::ToolStart {
                id,
                name,
                command,
                parent_id,
            } => {
                assert_eq!(id, "tool_1");
                assert_eq!(name, "bash");
                assert_eq!(command.as_deref(), Some("cargo test"));
                assert!(parent_id.is_none());
            }
            _ => panic!("Expected ToolStart"),
        }
    }

    #[test]
    fn test_agent_event_tool_complete() {
        let event = AgentEvent::ToolComplete {
            id: "tool_1".to_string(),
            name: "bash".to_string(),
            input: serde_json::json!({"command": "ls"}),
            output: "files".to_string(),
            is_error: false,
            parent_id: None,
        };

        match event {
            AgentEvent::ToolComplete {
                id,
                name,
                input,
                output,
                is_error,
                parent_id,
            } => {
                assert_eq!(id, "tool_1");
                assert_eq!(name, "bash");
                assert_eq!(input["command"], "ls");
                assert_eq!(output, "files");
                assert!(!is_error);
                assert!(parent_id.is_none());
            }
            _ => panic!("Expected ToolComplete"),
        }
    }

    #[test]
    fn test_agent_event_tool_output_delta() {
        let event = AgentEvent::ToolOutputDelta {
            id: "tool_1".to_string(),
            delta: "partial output".to_string(),
            parent_id: Some("parent".to_string()),
        };

        match event {
            AgentEvent::ToolOutputDelta {
                id,
                delta,
                parent_id,
            } => {
                assert_eq!(id, "tool_1");
                assert_eq!(delta, "partial output");
                assert_eq!(parent_id.as_deref(), Some("parent"));
            }
            _ => panic!("Expected ToolOutputDelta"),
        }
    }

    #[test]
    fn test_agent_event_token_usage() {
        let event = AgentEvent::TokenUsage {
            input_tokens: 100,
            output_tokens: 50,
            total_tokens: 150,
        };

        match event {
            AgentEvent::TokenUsage {
                input_tokens,
                output_tokens,
                total_tokens,
            } => {
                assert_eq!(input_tokens, 100);
                assert_eq!(output_tokens, 50);
                assert_eq!(total_tokens, 150);
            }
            _ => panic!("Expected TokenUsage"),
        }
    }

    #[test]
    fn test_agent_event_tool_progress() {
        let event = AgentEvent::ToolProgress {
            id: "tool_1".to_string(),
            message: "Processing...".to_string(),
            percent: Some(50),
            parent_id: None,
        };

        match event {
            AgentEvent::ToolProgress {
                id,
                message,
                percent,
                parent_id,
            } => {
                assert_eq!(id, "tool_1");
                assert_eq!(message, "Processing...");
                assert_eq!(percent, Some(50));
                assert!(parent_id.is_none());
            }
            _ => panic!("Expected ToolProgress"),
        }
    }

    #[test]
    fn test_agent_event_compaction() {
        let event = AgentEvent::Compaction {
            original_count: 10,
            compacted_count: 5,
            tokens_saved: 1000,
            messages_summarized: 3,
            status: CompressionStatus::Compressed,
        };

        match event {
            AgentEvent::Compaction {
                original_count,
                compacted_count,
                tokens_saved,
                messages_summarized,
                status,
            } => {
                assert_eq!(original_count, 10);
                assert_eq!(compacted_count, 5);
                assert_eq!(tokens_saved, 1000);
                assert_eq!(messages_summarized, 3);
                assert_eq!(status, CompressionStatus::Compressed);
            }
            _ => panic!("Expected Compaction"),
        }
    }

    #[test]
    fn test_agent_event_done() {
        let result = RunResult {
            text: "Done".to_string(),
            tool_calls: vec![],
        };

        let event = AgentEvent::Done { result };

        match event {
            AgentEvent::Done { result } => {
                assert_eq!(result.text, "Done");
            }
            _ => panic!("Expected Done"),
        }
    }

    #[test]
    fn test_agent_event_error() {
        let event = AgentEvent::Error {
            message: "Something went wrong".to_string(),
        };

        match event {
            AgentEvent::Error { message } => {
                assert_eq!(message, "Something went wrong");
            }
            _ => panic!("Expected Error"),
        }
    }

    #[test]
    fn test_agent_event_session_saved() {
        let event = AgentEvent::SessionSaved {
            path: "/sessions/123.json".to_string(),
        };

        match event {
            AgentEvent::SessionSaved { path } => {
                assert_eq!(path, "/sessions/123.json");
            }
            _ => panic!("Expected SessionSaved"),
        }
    }

    #[test]
    fn test_agent_event_clone() {
        let event = AgentEvent::TextDelta {
            delta: "Hello".to_string(),
        };

        let cloned = event.clone();
        match cloned {
            AgentEvent::TextDelta { delta } => assert_eq!(delta, "Hello"),
            _ => panic!("Expected TextDelta"),
        }
    }

    #[test]
    fn test_agent_event_serialization() {
        let event = AgentEvent::TextDelta {
            delta: "Hello".to_string(),
        };

        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("TextDelta"));
        assert!(json.contains("Hello"));
    }

    #[test]
    fn test_agent_event_deserialization() {
        let json = r#"{"TextDelta":{"delta":"Hello"}}"#;
        let event: AgentEvent = serde_json::from_str(json).unwrap();

        match event {
            AgentEvent::TextDelta { delta } => assert_eq!(delta, "Hello"),
            _ => panic!("Expected TextDelta"),
        }
    }
}
