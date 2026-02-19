use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
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

impl Default for RunResult {
    fn default() -> Self {
        Self {
            text: String::new(),
            tool_calls: Vec::new(),
        }
    }
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
    Done {
        result: RunResult,
    },
    Error {
        message: String,
    },
}
