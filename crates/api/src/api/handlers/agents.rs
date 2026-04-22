// @amadeus-header
// summary: HTTP handler implementation for agents routes.
// layer: api
// status: active
// feature_flags:
// - api
// provides:
// - module: crate::api::handlers::agents
// - fn: crate::api::handlers::agents::list_agents
// - fn: crate::api::handlers::agents::create_agent
// - fn: crate::api::handlers::agents::get_agent
// - fn: crate::api::handlers::agents::kill_agent
// - fn: crate::api::handlers::agents::switch_agent
// - fn: crate::api::handlers::agents::agent_chat
// - fn: crate::api::handlers::agents::agent_stream
// uses:
// - module: crate::agent::profile::AgentProfile
// - module: crate::api::http::AppState
// - module: crate::client::LLMClient
// - protocol: axum HTTP handlers
// invariants:
// - Handler request and response handling stays aligned with route contracts.
// side_effects:
// - Performs network or HTTP operations.
// tests:
// - tests/agent_integration_test.rs
// @end-amadeus-header

//! # Agent Management Handlers
//!
//! HTTP handlers for orchestra agent-management endpoints.

use axum::{
    extract::{Path, State},
    response::sse::{Event, Sse},
    Json,
};
use std::collections::HashMap;
use std::convert::Infallible;
use std::pin::Pin;
use std::sync::Arc;

use crate::agent::profile::AgentProfile;
use crate::api::http::AppState;
use crate::api::types::{
    AgentChatRequest, AgentChatResponse, CreateAgentRequest, CreateAgentResponse, ErrorResponse,
    KillAgentRequest, KillAgentResponse, ListAgentsResponse, SwitchAgentRequest,
    SwitchAgentResponse, ToolCall,
};
use crate::bridge::{BridgeEvent, BridgeSessionInfo};
use crate::client::LLMClient;
use futures::stream::{self, Stream};

type BoxedSseStream = Pin<Box<dyn Stream<Item = Result<Event, Infallible>> + Send>>;

fn to_api_agent(session: BridgeSessionInfo) -> crate::api::types::AgentInfo {
    crate::api::types::AgentInfo {
        id: session.id,
        name: session.name,
        profile: session.profile,
        status: format!("{:?}", session.status).to_lowercase(),
        task_count: 0,
    }
}

fn profile_from_string(profile: &str) -> AgentProfile {
    match profile {
        "default" => AgentProfile::Default,
        "debug" => AgentProfile::Debug,
        "docs" => AgentProfile::Docs,
        "review" | "code_review" => AgentProfile::CodeReview,
        other => AgentProfile::Custom(other.to_string()),
    }
}

fn bridge_event_to_sse(
    bridge_event: BridgeEvent,
    context_window_size: u32,
) -> Option<Result<Event, Infallible>> {
    match bridge_event {
        BridgeEvent::SessionCreated { session } | BridgeEvent::SessionUpdated { session } => {
            Some(Ok(Event::default()
                .event("session_state")
                .json_data(session)
                .unwrap()))
        }
        BridgeEvent::ChildSessionSpawned {
            parent_session_id,
            request_id,
            prompt,
            depth,
            session,
        } => Some(Ok(Event::default()
            .event("subagent_session")
            .json_data(serde_json::json!({
                "parent_session_id": parent_session_id,
                "request_id": request_id,
                "prompt": prompt,
                "depth": depth,
                "session": session
            }))
            .unwrap())),
        BridgeEvent::Agent { event, .. } => match event {
            crate::agent::AgentEvent::TextDelta { delta } => Some(Ok(Event::default()
                .event("text")
                .json_data(serde_json::json!({ "content": delta }))
                .unwrap())),
            crate::agent::AgentEvent::ThinkingDelta { delta } => Some(Ok(Event::default()
                .event("thinking")
                .json_data(serde_json::json!({ "delta": delta }))
                .unwrap())),
            crate::agent::AgentEvent::ThinkingComplete { thinking } => Some(Ok(Event::default()
                .event("thinking_complete")
                .json_data(serde_json::json!({ "thinking": thinking }))
                .unwrap())),
            crate::agent::AgentEvent::ToolStart {
                id,
                name,
                command,
                parent_id,
            } => Some(Ok(Event::default()
                .event("tool_start")
                .json_data(serde_json::json!({
                    "id": id,
                    "name": name,
                    "command": command,
                    "parent_id": parent_id
                }))
                .unwrap())),
            crate::agent::AgentEvent::ToolInputDelta {
                id,
                delta,
                parent_id,
            } => Some(Ok(Event::default()
                .event("tool_input")
                .json_data(serde_json::json!({
                    "id": id,
                    "delta": delta,
                    "parent_id": parent_id
                }))
                .unwrap())),
            crate::agent::AgentEvent::ToolOutputDelta {
                id,
                delta,
                parent_id,
            } => Some(Ok(Event::default()
                .event("tool_output")
                .json_data(serde_json::json!({
                    "id": id,
                    "delta": delta,
                    "parent_id": parent_id
                }))
                .unwrap())),
            crate::agent::AgentEvent::ToolComplete {
                id,
                name,
                output,
                is_error,
                parent_id,
                ..
            } => Some(Ok(Event::default()
                .event("tool_done")
                .json_data(serde_json::json!({
                    "id": id,
                    "name": name,
                    "output": output,
                    "is_error": is_error,
                    "parent_id": parent_id
                }))
                .unwrap())),
            crate::agent::AgentEvent::ApprovalRequired { request } => Some(Ok(Event::default()
                .event("approval_request")
                .json_data(serde_json::json!({
                    "id": request.id,
                    "tool": request.tool,
                    "action": request.reason,
                    "input": request.input
                }))
                .unwrap())),
            crate::agent::AgentEvent::SubAgentRequested { id, prompt, depth } => {
                Some(Ok(Event::default()
                    .event("subagent_requested")
                    .json_data(serde_json::json!({
                        "id": id,
                        "prompt": prompt,
                        "depth": depth
                    }))
                    .unwrap()))
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
                Some(Ok(Event::default()
                    .event("token_usage")
                    .json_data(serde_json::json!({
                        "input_tokens": input_tokens,
                        "output_tokens": output_tokens,
                        "total_tokens": total_tokens,
                        "context_percent": context_percent
                    }))
                    .unwrap()))
            }
            crate::agent::AgentEvent::ToolProgress {
                id,
                message,
                percent,
                parent_id,
            } => Some(Ok(Event::default()
                .event("tool_progress")
                .json_data(serde_json::json!({
                    "id": id,
                    "message": message,
                    "percent": percent,
                    "parent_id": parent_id
                }))
                .unwrap())),
            crate::agent::AgentEvent::Compaction {
                original_count,
                compacted_count,
                tokens_saved,
                messages_summarized,
                ..
            } => Some(Ok(Event::default()
                .event("compaction")
                .json_data(serde_json::json!({
                    "original_count": original_count,
                    "compacted_count": compacted_count,
                    "tokens_saved": tokens_saved,
                    "messages_summarized": messages_summarized
                }))
                .unwrap())),
            crate::agent::AgentEvent::Done { result } => Some(Ok(Event::default()
                .event("done")
                .json_data(serde_json::json!({
                    "stop_reason": "end_turn",
                    "result": result
                }))
                .unwrap())),
            crate::agent::AgentEvent::Error { message } => Some(Ok(Event::default()
                .event("error")
                .json_data(serde_json::json!({ "message": message }))
                .unwrap())),
            crate::agent::AgentEvent::SessionSaved { path } => Some(Ok(Event::default()
                .event("session_saved")
                .json_data(serde_json::json!({ "path": path }))
                .unwrap())),
        },
    }
}

/// List all agents.
pub async fn list_agents<C: LLMClient + Clone + 'static>(
    State(state): State<Arc<AppState<C>>>,
) -> Result<Json<ListAgentsResponse>, Json<ErrorResponse>> {
    let agents_api = state
        .session_bridge
        .list_sessions()
        .await
        .into_iter()
        .map(to_api_agent)
        .collect();

    Ok(Json(ListAgentsResponse {
        agents: agents_api,
        active_agent_id: state.session_bridge.active_session_id().await,
    }))
}

/// Create a new agent.
pub async fn create_agent<C: LLMClient + Clone + 'static>(
    State(state): State<Arc<AppState<C>>>,
    Json(request): Json<CreateAgentRequest>,
) -> Result<Json<CreateAgentResponse>, Json<ErrorResponse>> {
    let profile = profile_from_string(&request.profile);
    let agent = state
        .session_bridge
        .create_session(request.name, profile)
        .await
        .map_err(|e| Json(ErrorResponse::from_agent_error(&e)))?;
    Ok(Json(CreateAgentResponse {
        agent: to_api_agent(agent),
    }))
}

/// Get info for a specific agent.
pub async fn get_agent<C: LLMClient + Clone + 'static>(
    State(state): State<Arc<AppState<C>>>,
    Path(agent_id): Path<String>,
) -> Result<Json<crate::api::types::AgentInfo>, Json<ErrorResponse>> {
    let agent = state
        .session_bridge
        .get_session(&agent_id)
        .await
        .ok_or_else(|| Json(ErrorResponse::new("AgentNotFound", "Agent not found")))?;
    Ok(Json(to_api_agent(agent)))
}

/// Delete (kill) an agent.
pub async fn kill_agent<C: LLMClient + Clone + 'static>(
    State(state): State<Arc<AppState<C>>>,
    Path(agent_id): Path<String>,
    Json(_request): Json<KillAgentRequest>,
) -> Result<Json<KillAgentResponse>, Json<ErrorResponse>> {
    state
        .session_bridge
        .close_session(&agent_id)
        .await
        .map_err(|e| Json(ErrorResponse::from_agent_error(&e)))?;
    Ok(Json(KillAgentResponse { success: true }))
}

/// Switch to a different agent.
pub async fn switch_agent<C: LLMClient + Clone + 'static>(
    State(state): State<Arc<AppState<C>>>,
    Path(agent_id): Path<String>,
    Json(request): Json<SwitchAgentRequest>,
) -> Result<Json<SwitchAgentResponse>, Json<ErrorResponse>> {
    let target_id = if request.agent_id.is_empty() {
        agent_id
    } else {
        request.agent_id
    };
    state
        .session_bridge
        .set_active_session(&target_id)
        .await
        .map_err(|e| Json(ErrorResponse::from_agent_error(&e)))?;
    Ok(Json(SwitchAgentResponse {
        success: true,
        active_agent_id: target_id,
    }))
}

/// Chat with a specific agent (non-streaming).
pub async fn agent_chat<C: LLMClient + Clone + 'static>(
    State(state): State<Arc<AppState<C>>>,
    Path(agent_id): Path<String>,
    Json(request): Json<AgentChatRequest>,
) -> Result<Json<AgentChatResponse>, Json<ErrorResponse>> {
    let mut rx = state
        .session_bridge
        .subscribe(&agent_id)
        .await
        .map_err(|e| Json(ErrorResponse::from_agent_error(&e)))?;
    state
        .session_bridge
        .submit_input(&agent_id, request.message)
        .await
        .map_err(|e| Json(ErrorResponse::from_agent_error(&e)))?;

    let timeout_secs = request.timeout_secs.unwrap_or(state.config.timeout_seconds);
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(timeout_secs);

    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        let bridge_event = tokio::time::timeout(remaining, rx.recv())
            .await
            .map_err(|_| Json(ErrorResponse::new("Timeout", "Agent chat timed out")))?
            .map_err(|_| Json(ErrorResponse::new("StreamClosed", "Agent stream closed")))?;
        if let BridgeEvent::Agent { event, .. } = bridge_event {
            match event {
                crate::agent::AgentEvent::Done { result } => {
                    let tool_calls = result
                        .tool_calls
                        .into_iter()
                        .map(|tool| ToolCall {
                            name: tool.name,
                            input: tool.input,
                            output: tool.output,
                        })
                        .collect();
                    return Ok(Json(AgentChatResponse {
                        content: result.text,
                        tool_calls,
                        stop_reason: "end_turn".to_string(),
                    }));
                }
                crate::agent::AgentEvent::Error { message } => {
                    return Err(Json(ErrorResponse::new("AgentError", message)));
                }
                _ => {}
            }
        }
    }
}

/// Stream events from a specific agent.
pub async fn agent_stream<C: LLMClient + Clone + 'static>(
    State(state): State<Arc<AppState<C>>>,
    Path(agent_id): Path<String>,
    axum::extract::Query(params): axum::extract::Query<HashMap<String, String>>,
) -> Result<
    Sse<BoxedSseStream>,
    Json<ErrorResponse>,
> {
    let rx = state
        .session_bridge
        .subscribe(&agent_id)
        .await
        .map_err(|e| Json(ErrorResponse::from_agent_error(&e)))?;
    if let Some(message) = params.get("message") {
        state
            .session_bridge
            .submit_input(&agent_id, message.clone())
            .await
            .map_err(|e| Json(ErrorResponse::from_agent_error(&e)))?;
    }

    let context_window_size = state.config.context_window_size;
    let stream = stream::unfold(rx, move |mut rx| async move {
        loop {
            match rx.recv().await {
                Ok(event) => {
                    if let Some(mapped) = bridge_event_to_sse(event, context_window_size) {
                        return Some((mapped, rx));
                    }
                }
                Err(_) => return None,
            }
        }
    });

    Ok(Sse::new(Box::pin(stream)))
}
