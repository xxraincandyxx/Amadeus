use serde::{Deserialize, Serialize};
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
    ToolStart {
        id: String,
        name: String,
    },
    ToolInputDelta {
        id: String,
        delta: String,
    },
    ToolComplete {
        id: String,
        name: String,
        input: Value,
        output: String,
        is_error: bool,
    },
    /// Approval is required before tool execution.
    /// The consumer (TUI/Platform) must respond via Agent::send_approval_decision().
    ApprovalRequired {
        /// The approval request details.
        request: ApprovalRequest,
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
