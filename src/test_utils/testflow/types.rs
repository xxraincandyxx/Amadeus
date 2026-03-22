//! # Testflow Types
//!
//! Type definitions for session recording and playback.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub const TESTFLOW_VERSION: &str = "1.0.0";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionLog {
    pub version: String,
    pub metadata: SessionMetadata,
    pub timeline: Vec<TimelineEvent>,
    pub summaries: SessionSummaries,
    pub snapshots: SessionSnapshots,
}

impl Default for SessionLog {
    fn default() -> Self {
        Self {
            version: TESTFLOW_VERSION.to_string(),
            metadata: SessionMetadata::default(),
            timeline: Vec::new(),
            summaries: SessionSummaries::default(),
            snapshots: SessionSnapshots::default(),
        }
    }
}

impl SessionLog {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push_event(&mut self, event: TimelineEvent) {
        self.timeline.push(event);
    }

    pub fn finalize(&mut self) {
        self.metadata.ended_at = Some(Utc::now());
        if let Some(start) = self.metadata.created_at {
            self.metadata.duration_ms = (Utc::now() - start).num_milliseconds() as u64;
        }
        self.compute_summaries();
    }

    fn compute_summaries(&mut self) {
        let summaries = &mut self.summaries;

        for event in &self.timeline {
            match &event.event_type {
                RecordedEvent::AgentEvent { event, .. } => match event {
                    AgentEventData::ToolStart { .. } => {
                        summaries.total_tools_executed += 1;
                    }
                    AgentEventData::TokenUsage { total_tokens, .. } => {
                        summaries.total_tokens += total_tokens;
                    }
                    AgentEventData::Compaction {
                        tokens_saved,
                        status,
                        ..
                    } => {
                        if status == "Inflated" || status == "Noop" {
                            // Don't count these as real compaction events
                        } else {
                            summaries.compaction_events += 1;
                            summaries.tokens_saved_by_compaction += tokens_saved;
                        }
                        summaries.compaction_events += 1;
                        summaries.tokens_saved_by_compaction += tokens_saved;
                    }
                    _ => {}
                },
                RecordedEvent::ApprovalRequest { tool_name, .. } => {
                    summaries.approvals_requested += 1;
                    summaries
                        .tools_by_name
                        .entry(tool_name.clone())
                        .or_insert(0);
                }
                RecordedEvent::ApprovalResponse { decision, .. } => match decision {
                    ApprovalDecision::Approve | ApprovalDecision::AlwaysApprove => {
                        summaries.approvals_approved += 1
                    }
                    ApprovalDecision::Deny => summaries.approvals_denied += 1,
                },
                RecordedEvent::LlmRequest { .. } => {
                    summaries.total_turns += 1;
                }
                RecordedEvent::Error { message, .. } => {
                    summaries.errors.push(message.clone());
                }
                _ => {}
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SessionMetadata {
    pub session_id: String,
    pub created_at: Option<DateTime<Utc>>,
    pub ended_at: Option<DateTime<Utc>>,
    pub duration_ms: u64,
    pub platform: String,
    pub rust_version: String,
    pub amadeus_version: String,
    pub feature_flags: Vec<String>,
    pub config_snapshot: ConfigSnapshot,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ConfigSnapshot {
    pub provider: String,
    pub model: String,
    pub workdir: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineEvent {
    pub seq: usize,
    pub timestamp_ms: u64,
    pub event_type: RecordedEvent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RecordedEvent {
    SessionStart,
    SessionEnd {
        reason: SessionEndReason,
        final_state: SessionState,
    },
    UserInput {
        input_id: String,
        content: String,
        source: InputSource,
    },
    LlmRequest {
        request_id: String,
        turn: usize,
        message_count: usize,
        tools_available: Vec<String>,
    },
    LlmResponse {
        request_id: String,
        stop_reason: String,
        duration_ms: u64,
    },
    AgentEvent {
        event: AgentEventData,
    },
    ApprovalRequest {
        approval_id: String,
        tool_id: String,
        tool_name: String,
        input: serde_json::Value,
        reason: String,
    },
    ApprovalResponse {
        approval_id: String,
        decision: ApprovalDecision,
    },
    ToolExecutionStart {
        tool_id: String,
        tool_name: String,
    },
    ToolInputStream {
        tool_id: String,
        delta: String,
    },
    ToolOutputStream {
        tool_id: String,
        delta: String,
    },
    ToolComplete {
        tool_id: String,
        tool_name: String,
        output: String,
        is_error: bool,
        duration_ms: u64,
    },
    KeyboardInput {
        key: String,
        modifiers: Vec<String>,
        context: String,
    },
    GuiRender {
        frame_id: u64,
        components_updated: Vec<String>,
        render_duration_us: u64,
    },
    GuiStateChange {
        component: String,
        state: serde_json::Value,
    },
    Error {
        message: String,
        context: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentEventData {
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
        input: serde_json::Value,
        output: String,
        is_error: bool,
        parent_id: Option<String>,
    },
    ApprovalRequired {
        id: String,
        tool: String,
        input: serde_json::Value,
        reason: String,
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
        #[serde(default)]
        status: String,
    },
    Done {
        text: String,
        tool_call_count: usize,
    },
    Error {
        message: String,
    },
    SessionSaved {
        path: String,
    },
}

impl From<crate::agent::events::AgentEvent> for AgentEventData {
    fn from(event: crate::agent::events::AgentEvent) -> Self {
        use crate::agent::events::AgentEvent;
        match event {
            AgentEvent::TextDelta { delta } => AgentEventData::TextDelta { delta },
            AgentEvent::ThinkingDelta { delta } => AgentEventData::ThinkingDelta { delta },
            AgentEvent::ThinkingComplete { thinking } => {
                AgentEventData::ThinkingComplete { thinking }
            }
            AgentEvent::ToolStart {
                id,
                name,
                parent_id,
            } => AgentEventData::ToolStart {
                id,
                name,
                parent_id,
            },
            AgentEvent::ToolInputDelta {
                id,
                delta,
                parent_id,
            } => AgentEventData::ToolInputDelta {
                id,
                delta,
                parent_id,
            },
            AgentEvent::ToolOutputDelta {
                id,
                delta,
                parent_id,
            } => AgentEventData::ToolOutputDelta {
                id,
                delta,
                parent_id,
            },
            AgentEvent::ToolComplete {
                id,
                name,
                input,
                output,
                is_error,
                parent_id,
            } => AgentEventData::ToolComplete {
                id,
                name,
                input,
                output,
                is_error,
                parent_id,
            },
            AgentEvent::ApprovalRequired { request } => AgentEventData::ApprovalRequired {
                id: request.id,
                tool: request.tool,
                input: request.input,
                reason: request.reason,
            },
            AgentEvent::TokenUsage {
                input_tokens,
                output_tokens,
                total_tokens,
            } => AgentEventData::TokenUsage {
                input_tokens,
                output_tokens,
                total_tokens,
            },
            AgentEvent::ToolProgress {
                id,
                message,
                percent,
                parent_id,
            } => AgentEventData::ToolProgress {
                id,
                message,
                percent,
                parent_id,
            },
            AgentEvent::Compaction {
                original_count,
                compacted_count,
                tokens_saved,
                messages_summarized,
                status,
            } => AgentEventData::Compaction {
                original_count,
                compacted_count,
                tokens_saved,
                messages_summarized,
                status: format!("{:?}", status),
            },
            AgentEvent::Done { result } => AgentEventData::Done {
                text: result.text,
                tool_call_count: result.tool_calls.len(),
            },
            AgentEvent::Error { message } => AgentEventData::Error { message },
            AgentEvent::SessionSaved { path } => AgentEventData::SessionSaved { path },
            AgentEvent::SubAgentRequested {
                id,
                prompt: _,
                depth,
            } => AgentEventData::ToolStart {
                id,
                name: format!("sub_agent:depth{}", depth),
                parent_id: None,
            },
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionEndReason {
    UserExit,
    Completed,
    Error,
    Timeout,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionState {
    Completed,
    Cancelled,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InputSource {
    Keyboard,
    Api,
    Replay,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalDecision {
    Approve,
    Deny,
    AlwaysApprove,
}

impl From<crate::agent::events::ApprovalDecision> for ApprovalDecision {
    fn from(d: crate::agent::events::ApprovalDecision) -> Self {
        use crate::agent::events::ApprovalDecision as AD;
        match d {
            AD::Approve => ApprovalDecision::Approve,
            AD::Deny => ApprovalDecision::Deny,
            AD::AlwaysApprove => ApprovalDecision::AlwaysApprove,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SessionSummaries {
    pub total_turns: usize,
    pub total_tools_executed: usize,
    pub tools_by_name: HashMap<String, usize>,
    pub approvals_requested: usize,
    pub approvals_approved: usize,
    pub approvals_denied: usize,
    pub total_tokens: u32,
    pub compaction_events: usize,
    pub tokens_saved_by_compaction: usize,
    pub errors: Vec<String>,
    pub gui_stats: GuiStats,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GuiStats {
    pub total_frames: u64,
    pub avg_render_time_us: u64,
    pub max_render_time_us: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TuiFrameSnapshot {
    pub session_id: String,
    pub frame_id: u64,
    pub timestamp_ms: u64,
    pub width: u16,
    pub height: u16,
    pub cursor: Option<TuiCursorSnapshot>,
    pub cells: Vec<TuiCellSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TuiCursorSnapshot {
    pub x: u16,
    pub y: u16,
    pub visible: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TuiCellSnapshot {
    pub x: u16,
    pub y: u16,
    pub symbol: String,
    pub fg: String,
    pub bg: String,
    pub underline_color: String,
    pub add_modifier: String,
    pub sub_modifier: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SessionSnapshots {
    pub final_history: Vec<HistoryEntry>,
    pub final_result: Option<FinalResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub role: String,
    pub content: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FinalResult {
    pub text: String,
    pub tool_call_count: usize,
}

#[derive(Debug, Clone)]
pub struct RecorderConfig {
    pub capture_gui_events: bool,
    pub capture_tool_io: bool,
    pub capture_llm_requests: bool,
    pub max_output_size: usize,
    pub redact_patterns: Vec<String>,
}

impl Default for RecorderConfig {
    fn default() -> Self {
        Self {
            capture_gui_events: true,
            capture_tool_io: true,
            capture_llm_requests: true,
            max_output_size: 100_000,
            redact_patterns: vec![
                "ANTHROPIC_API_KEY".to_string(),
                "OPENAI_API_KEY".to_string(),
                "sk-ant-".to_string(),
                "sk-proj-".to_string(),
            ],
        }
    }
}

impl RecorderConfig {
    pub fn minimal() -> Self {
        Self {
            capture_gui_events: false,
            capture_tool_io: false,
            capture_llm_requests: true,
            max_output_size: 10_000,
            redact_patterns: vec![],
        }
    }

    pub fn full() -> Self {
        Self::default()
    }
}
