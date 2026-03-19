#![allow(dead_code)]

use std::time::{Duration, Instant};

use amadeus::agent::events::{AgentEvent, ApprovalRequest, RunResult};
use amadeus::agent::messages::Message;

#[derive(Debug, Clone)]
pub struct TimestampedEvent {
    pub event: AgentEvent,
    pub elapsed: Duration,
}

#[derive(Debug, Clone)]
pub struct EventTimeline {
    events: Vec<TimestampedEvent>,
    history_snapshot: Vec<Message>,
    start: Instant,
}

impl EventTimeline {
    pub fn new() -> Self {
        Self {
            events: Vec::new(),
            history_snapshot: Vec::new(),
            start: Instant::now(),
        }
    }

    pub fn push(&mut self, event: AgentEvent) {
        self.events.push(TimestampedEvent {
            event,
            elapsed: self.start.elapsed(),
        });
    }

    pub fn set_history_snapshot(&mut self, history: Vec<Message>) {
        self.history_snapshot = history;
    }

    pub fn raw_events(&self) -> Vec<AgentEvent> {
        self.events.iter().map(|e| e.event.clone()).collect()
    }

    pub fn timestamped_events(&self) -> &[TimestampedEvent] {
        &self.events
    }

    pub fn history_snapshot(&self) -> &[Message] {
        &self.history_snapshot
    }

    pub fn len(&self) -> usize {
        self.events.len()
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    pub fn total_duration(&self) -> Duration {
        self.events.last().map(|e| e.elapsed).unwrap_or_default()
    }

    // --- Text ---

    pub fn full_text(&self) -> String {
        self.events
            .iter()
            .filter_map(|e| match &e.event {
                AgentEvent::TextDelta { delta } => Some(delta.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("")
    }

    pub fn text_delta_count(&self) -> usize {
        self.events
            .iter()
            .filter(|e| matches!(&e.event, AgentEvent::TextDelta { .. }))
            .count()
    }

    // --- Thinking ---

    pub fn full_thinking(&self) -> String {
        self.events
            .iter()
            .filter_map(|e| match &e.event {
                AgentEvent::ThinkingDelta { delta } => Some(delta.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("")
    }

    pub fn has_thinking(&self) -> bool {
        self.events
            .iter()
            .any(|e| matches!(&e.event, AgentEvent::ThinkingDelta { .. }))
    }

    // --- Tools ---

    pub fn tool_starts(&self) -> Vec<(String, String)> {
        self.events
            .iter()
            .filter_map(|e| match &e.event {
                AgentEvent::ToolStart { id, name, .. } => Some((id.clone(), name.clone())),
                _ => None,
            })
            .collect()
    }

    pub fn tool_completions(&self) -> Vec<ToolCompletionInfo> {
        self.events
            .iter()
            .filter_map(|e| match &e.event {
                AgentEvent::ToolComplete {
                    id,
                    name,
                    input,
                    output,
                    is_error,
                    ..
                } => Some(ToolCompletionInfo {
                    id: id.clone(),
                    name: name.clone(),
                    input: input.clone(),
                    output: output.clone(),
                    is_error: *is_error,
                }),
                _ => None,
            })
            .collect()
    }

    pub fn tool_count(&self) -> usize {
        self.events
            .iter()
            .filter(|e| matches!(&e.event, AgentEvent::ToolStart { .. }))
            .count()
    }

    pub fn tool_names(&self) -> Vec<String> {
        self.events
            .iter()
            .filter_map(|e| match &e.event {
                AgentEvent::ToolStart { name, .. } => Some(name.clone()),
                _ => None,
            })
            .collect()
    }

    pub fn tool_errors(&self) -> Vec<ToolCompletionInfo> {
        self.tool_completions()
            .into_iter()
            .filter(|t| t.is_error)
            .collect()
    }

    pub fn tool_input_for(&self, tool_id: &str) -> String {
        self.events
            .iter()
            .filter_map(|e| match &e.event {
                AgentEvent::ToolInputDelta { id, delta, .. } if id == tool_id => {
                    Some(delta.as_str())
                }
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("")
    }

    // --- Approvals ---

    pub fn approval_requests(&self) -> Vec<ApprovalRequest> {
        self.events
            .iter()
            .filter_map(|e| match &e.event {
                AgentEvent::ApprovalRequired { request } => Some(request.clone()),
                _ => None,
            })
            .collect()
    }

    pub fn has_approval_requests(&self) -> bool {
        self.events
            .iter()
            .any(|e| matches!(&e.event, AgentEvent::ApprovalRequired { .. }))
    }

    // --- Token usage ---

    pub fn token_usage_events(&self) -> Vec<(u32, u32, u32)> {
        self.events
            .iter()
            .filter_map(|e| match &e.event {
                AgentEvent::TokenUsage {
                    input_tokens,
                    output_tokens,
                    total_tokens,
                } => Some((*input_tokens, *output_tokens, *total_tokens)),
                _ => None,
            })
            .collect()
    }

    pub fn total_tokens(&self) -> u32 {
        self.token_usage_events()
            .iter()
            .map(|(_, _, total)| total)
            .sum()
    }

    // --- Compaction ---

    pub fn compaction_events(&self) -> Vec<CompactionInfo> {
        self.events
            .iter()
            .filter_map(|e| match &e.event {
                AgentEvent::Compaction {
                    original_count,
                    compacted_count,
                    tokens_saved,
                    messages_summarized,
                    status: _,
                } => Some(CompactionInfo {
                    original_count: *original_count,
                    compacted_count: *compacted_count,
                    tokens_saved: *tokens_saved,
                    messages_summarized: *messages_summarized,
                }),
                _ => None,
            })
            .collect()
    }

    pub fn had_compaction(&self) -> bool {
        self.events
            .iter()
            .any(|e| matches!(&e.event, AgentEvent::Compaction { .. }))
    }

    // --- Errors ---

    pub fn errors(&self) -> Vec<String> {
        self.events
            .iter()
            .filter_map(|e| match &e.event {
                AgentEvent::Error { message } => Some(message.clone()),
                _ => None,
            })
            .collect()
    }

    pub fn has_errors(&self) -> bool {
        self.events
            .iter()
            .any(|e| matches!(&e.event, AgentEvent::Error { .. }))
    }

    // --- Done / Result ---

    pub fn run_result(&self) -> Option<RunResult> {
        self.events.iter().find_map(|e| match &e.event {
            AgentEvent::Done { result } => Some(result.clone()),
            _ => None,
        })
    }

    pub fn is_done(&self) -> bool {
        self.events
            .iter()
            .any(|e| matches!(&e.event, AgentEvent::Done { .. }))
    }

    // --- Session ---

    pub fn session_saved_path(&self) -> Option<String> {
        self.events.iter().find_map(|e| match &e.event {
            AgentEvent::SessionSaved { path } => Some(path.clone()),
            _ => None,
        })
    }

    // --- Event sequence labeling ---

    pub fn event_labels(&self) -> Vec<String> {
        self.events
            .iter()
            .map(|e| match &e.event {
                AgentEvent::TextDelta { .. } => "text".to_string(),
                AgentEvent::ThinkingDelta { .. } => "thinking".to_string(),
                AgentEvent::ThinkingComplete { .. } => "thinking_complete".to_string(),
                AgentEvent::ToolStart { name, .. } => format!("tool_start:{}", name),
                AgentEvent::ToolInputDelta { .. } => "tool_input".to_string(),
                AgentEvent::ToolOutputDelta { .. } => "tool_output".to_string(),
                AgentEvent::ToolComplete { name, .. } => format!("tool_complete:{}", name),
                AgentEvent::ApprovalRequired { request } => {
                    format!("approval:{}", request.tool)
                }
                AgentEvent::TokenUsage { .. } => "token_usage".to_string(),
                AgentEvent::ToolProgress { .. } => "tool_progress".to_string(),
                AgentEvent::Compaction { .. } => "compaction".to_string(),
                AgentEvent::Done { .. } => "done".to_string(),
                AgentEvent::Error { .. } => "error".to_string(),
                AgentEvent::SessionSaved { .. } => "session_saved".to_string(),
                AgentEvent::SubAgentRequested { .. } => "subagent_requested".to_string(),
            })
            .collect()
    }

    // --- History inspection ---

    pub fn history_len(&self) -> usize {
        self.history_snapshot.len()
    }

    pub fn history_roles(&self) -> Vec<String> {
        self.history_snapshot
            .iter()
            .map(|m| m.role.clone())
            .collect()
    }

    pub fn first_request_elapsed(&self) -> Option<Duration> {
        self.events.first().map(|e| e.elapsed)
    }
}

#[derive(Debug, Clone)]
pub struct ToolCompletionInfo {
    pub id: String,
    pub name: String,
    pub input: serde_json::Value,
    pub output: String,
    pub is_error: bool,
}

#[derive(Debug, Clone)]
pub struct CompactionInfo {
    pub original_count: usize,
    pub compacted_count: usize,
    pub tokens_saved: usize,
    pub messages_summarized: usize,
}
