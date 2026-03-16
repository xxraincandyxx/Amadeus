use serde::{Deserialize, Serialize};

use crate::agent::events::AgentEvent;

use super::report::CapturedEvent;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BenchmarkMetrics {
    pub total_duration_ms: u64,
    pub first_event_ms: Option<u64>,
    pub first_text_delta_ms: Option<u64>,
    pub text_delta_count: usize,
    pub thinking_delta_count: usize,
    pub tool_call_count: usize,
    pub tool_error_count: usize,
    pub approval_count: usize,
    pub compaction_count: usize,
    pub error_count: usize,
    pub token_input_total: u64,
    pub token_output_total: u64,
    pub token_total: u64,
    pub final_output_len: usize,
    pub tools_used: Vec<String>,
    pub error_messages: Vec<String>,
}

impl BenchmarkMetrics {
    pub fn from_events(events: &[CapturedEvent], final_text: &str) -> Self {
        let mut metrics = BenchmarkMetrics {
            total_duration_ms: events.last().map(|event| event.elapsed_ms).unwrap_or(0),
            final_output_len: final_text.len(),
            ..Self::default()
        };

        for event in events {
            if metrics.first_event_ms.is_none() {
                metrics.first_event_ms = Some(event.elapsed_ms);
            }

            match &event.event {
                AgentEvent::TextDelta { .. } => {
                    metrics.text_delta_count += 1;
                    if metrics.first_text_delta_ms.is_none() {
                        metrics.first_text_delta_ms = Some(event.elapsed_ms);
                    }
                }
                AgentEvent::ThinkingDelta { .. } => {
                    metrics.thinking_delta_count += 1;
                }
                AgentEvent::ToolStart { name, .. } => {
                    metrics.tool_call_count += 1;
                    metrics.tools_used.push(name.clone());
                }
                AgentEvent::ToolComplete { is_error, .. } => {
                    if *is_error {
                        metrics.tool_error_count += 1;
                    }
                }
                AgentEvent::ApprovalRequired { .. } => {
                    metrics.approval_count += 1;
                }
                AgentEvent::Compaction { .. } => {
                    metrics.compaction_count += 1;
                }
                AgentEvent::TokenUsage {
                    input_tokens,
                    output_tokens,
                    total_tokens,
                } => {
                    metrics.token_input_total += u64::from(*input_tokens);
                    metrics.token_output_total += u64::from(*output_tokens);
                    metrics.token_total += u64::from(*total_tokens);
                }
                AgentEvent::Error { message } => {
                    metrics.error_count += 1;
                    metrics.error_messages.push(message.clone());
                }
                AgentEvent::ToolOutputDelta { .. } => {}
                AgentEvent::SubAgentRequested { .. } => {}
                _ => {}
            }
        }

        metrics
    }
}
