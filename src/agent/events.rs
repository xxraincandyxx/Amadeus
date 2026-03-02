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
    /// The consumer (TUI/Platform) must respond with approval decision.
    ApprovalRequired {
        /// Unique ID for this approval request.
        id: String,
        /// Tool name that requires approval.
        tool: String,
        /// Tool input that will be executed if approved.
        input: Value,
        /// Reason why approval is needed.
        reason: String,
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
