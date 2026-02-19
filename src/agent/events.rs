use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::loop_agent::RunResult;

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
