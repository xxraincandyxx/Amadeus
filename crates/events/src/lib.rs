// @amadeus-header
// summary: Shared agent event model types reused across runtime surfaces.
// layer: core
// status: active
// feature_flags: none
// provides:
// - module: crate
// - type: crate::RunResult
// - type: crate::ToolCall
// - type: crate::ApprovalDecision
// - type: crate::ApprovalRequest
// - type: crate::AgentEvent
// uses:
// - module: amadeus_compaction
// - protocol: serde serialization
// - format: JSON values
// invariants:
// - Event payloads remain transport-agnostic and serialization-stable.
// side_effects: none
// tests:
// - cmd: cargo test -p events
// @end-amadeus-header

//! Shared agent event model types.

use amadeus_compaction::CompressionStatus;
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalDecision {
    Approve,
    Deny,
    AlwaysApprove,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalRequest {
    pub id: String,
    pub tool: String,
    pub input: Value,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AgentEvent {
    TextDelta {
        delta: String,
    },
    ThinkingDelta {
        delta: String,
    },
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
    ApprovalRequired {
        request: ApprovalRequest,
    },
    SubAgentRequested {
        id: String,
        prompt: String,
        depth: usize,
    },
    TokenUsage {
        input_tokens: u32,
        output_tokens: u32,
        total_tokens: u32,
    },
    ToolProgress {
        id: String,
        message: String,
        percent: Option<u8>,
        parent_id: Option<String>,
    },
    Compaction {
        original_count: usize,
        compacted_count: usize,
        tokens_saved: usize,
        messages_summarized: usize,
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
    fn run_result_default_is_empty() {
        let result = RunResult::default();
        assert!(result.text.is_empty());
        assert!(result.tool_calls.is_empty());
    }

    #[test]
    fn approval_decisions_are_distinct() {
        assert_ne!(ApprovalDecision::Approve, ApprovalDecision::Deny);
        assert_ne!(ApprovalDecision::Approve, ApprovalDecision::AlwaysApprove);
        assert_ne!(ApprovalDecision::Deny, ApprovalDecision::AlwaysApprove);
    }
}
