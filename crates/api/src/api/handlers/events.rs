// @amadeus-header
// summary: Maps session bridge events to the external server-sent event protocol.
// layer: api
// status: active
// feature_flags:
// - api
// provides:
// - fn: crate::api::handlers::events::bridge_event_to_sse
// uses:
// - fn: crate::api::handlers::sse_event
// - event: crate::bridge::BridgeEvent
// - protocol: server-sent events
// invariants:
// - Event names and payload fields remain compatible with external session clients.
// side_effects: none
// tests:
// - cmd: cargo test -p api --all-features
// @end-amadeus-header

use std::convert::Infallible;

use axum::response::sse::Event;

use super::sse_event;
use crate::bridge::BridgeEvent;

pub(crate) fn bridge_event_to_sse(
    bridge_event: BridgeEvent,
    context_window_size: u32,
) -> Option<Result<Event, Infallible>> {
    match bridge_event {
        BridgeEvent::SessionCreated { session } | BridgeEvent::SessionUpdated { session } => {
            Some(Ok(sse_event("session_state", session)))
        }
        BridgeEvent::ChildSessionSpawned {
            parent_session_id,
            request_id,
            prompt,
            depth,
            session,
        } => Some(Ok(sse_event(
            "subagent_session",
            serde_json::json!({
                "parent_session_id": parent_session_id,
                "request_id": request_id,
                "prompt": prompt,
                "depth": depth,
                "session": session
            }),
        ))),
        BridgeEvent::Agent { event, .. } => match event {
            crate::agent::AgentEvent::TextDelta { delta } => Some(Ok(sse_event(
                "text",
                serde_json::json!({ "content": delta }),
            ))),
            crate::agent::AgentEvent::ThinkingDelta { delta } => Some(Ok(sse_event(
                "thinking",
                serde_json::json!({ "delta": delta }),
            ))),
            crate::agent::AgentEvent::ThinkingComplete { thinking } => Some(Ok(sse_event(
                "thinking_complete",
                serde_json::json!({ "thinking": thinking }),
            ))),
            crate::agent::AgentEvent::ToolStart {
                id,
                name,
                command,
                parent_id,
            } => Some(Ok(sse_event(
                "tool_start",
                serde_json::json!({
                    "id": id,
                    "name": name,
                    "command": command,
                    "parent_id": parent_id
                }),
            ))),
            crate::agent::AgentEvent::ToolInputDelta {
                id,
                delta,
                parent_id,
            } => Some(Ok(sse_event(
                "tool_input",
                serde_json::json!({
                    "id": id,
                    "delta": delta,
                    "parent_id": parent_id
                }),
            ))),
            crate::agent::AgentEvent::ToolOutputDelta {
                id,
                delta,
                parent_id,
            } => Some(Ok(sse_event(
                "tool_output",
                serde_json::json!({
                    "id": id,
                    "delta": delta,
                    "parent_id": parent_id
                }),
            ))),
            crate::agent::AgentEvent::ToolComplete {
                id,
                name,
                output,
                is_error,
                parent_id,
                ..
            } => Some(Ok(sse_event(
                "tool_done",
                serde_json::json!({
                    "id": id,
                    "name": name,
                    "output": output,
                    "is_error": is_error,
                    "parent_id": parent_id
                }),
            ))),
            crate::agent::AgentEvent::ApprovalRequired { request } => Some(Ok(sse_event(
                "approval_request",
                serde_json::json!({
                    "id": request.id,
                    "tool": request.tool,
                    "action": request.reason,
                    "input": request.input
                }),
            ))),
            crate::agent::AgentEvent::SubAgentRequested { id, prompt, depth } => {
                Some(Ok(sse_event(
                    "subagent_requested",
                    serde_json::json!({
                        "id": id,
                        "prompt": prompt,
                        "depth": depth
                    }),
                )))
            }
            crate::agent::AgentEvent::TokenUsage {
                input_tokens,
                output_tokens,
                total_tokens,
            } => {
                let context_percent = if context_window_size > 0 {
                    ((total_tokens as f32 / context_window_size as f32) * 100.0).min(100.0) as u8
                } else {
                    0
                };
                Some(Ok(sse_event(
                    "token_usage",
                    serde_json::json!({
                        "input_tokens": input_tokens,
                        "output_tokens": output_tokens,
                        "total_tokens": total_tokens,
                        "context_percent": context_percent
                    }),
                )))
            }
            crate::agent::AgentEvent::ToolProgress {
                id,
                message,
                percent,
                parent_id,
            } => Some(Ok(sse_event(
                "tool_progress",
                serde_json::json!({
                    "id": id,
                    "message": message,
                    "percent": percent,
                    "parent_id": parent_id
                }),
            ))),
            crate::agent::AgentEvent::Compaction {
                original_count,
                compacted_count,
                tokens_saved,
                messages_summarized,
                ..
            } => Some(Ok(sse_event(
                "compaction",
                serde_json::json!({
                    "original_count": original_count,
                    "compacted_count": compacted_count,
                    "tokens_saved": tokens_saved,
                    "messages_summarized": messages_summarized
                }),
            ))),
            crate::agent::AgentEvent::Done { result } => Some(Ok(sse_event(
                "done",
                serde_json::json!({
                    "stop_reason": "end_turn",
                    "result": result
                }),
            ))),
            crate::agent::AgentEvent::Error { message } => Some(Ok(sse_event(
                "error",
                serde_json::json!({ "message": message }),
            ))),
            crate::agent::AgentEvent::SessionSaved { path } => Some(Ok(sse_event(
                "session_saved",
                serde_json::json!({ "path": path }),
            ))),
        },
    }
}
