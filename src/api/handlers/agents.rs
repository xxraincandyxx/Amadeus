//! # Agent Management Handlers
//!
//! HTTP handlers for multi-agent management endpoints.

use axum::{
    extract::{Path, State},
    routing::delete,
    response::sse::{Event, Sse},
    Json,
};
use futures::stream::{self, StreamExt};
use std::collections::HashMap;
use std::sync::Arc;

use crate::api::http::AppState;
use crate::api::types::{
    AgentChatResponse, CreateAgentRequest, CreateAgentResponse, ErrorResponse, KillAgentRequest,
    KillAgentResponse, ListAgentsResponse, SwitchAgentRequest, SwitchAgentResponse, ToolCall,
};
use crate::client::LLMClient;

/// List all agents.
pub async fn list_agents<C: LLMClient + Clone + 'static>(
    State(_state): State<Arc<AppState<C>>>,
) -> Result<Json<ListAgentsResponse>, Json<ErrorResponse>> {
    // For now, return a placeholder response
    // The actual implementation would use agent_manager
    Ok(Json(ListAgentsResponse {
        agents: vec![],
        active_agent_id: None,
    }))
}

/// Create a new agent.
pub async fn create_agent<C: LLMClient + Clone + 'static>(
    State(_state): State<Arc<AppState<C>>>,
    Json(request): Json<CreateAgentRequest>,
) -> Result<Json<CreateAgentResponse>, Json<ErrorResponse>> {
    // For now, return a placeholder response
    // The actual implementation would create an agent via agent_manager
    let profile = if request.profile.is_empty() { "default".to_string() } else { request.profile };
    let name = request.name.unwrap_or_else(|| "default-1".to_string());

    Ok(Json(CreateAgentResponse {
        agent: crate::api::types::AgentInfo {
            id: "agent-001".to_string(),
            name,
            profile,
            status: "idle".to_string(),
            task_count: 0,
        },
    }))
}

/// Get info for a specific agent.
pub async fn get_agent<C: LLMClient + Clone + 'static>(
    State(_state): State<Arc<AppState<C>>>,
    Path(agent_id): Path<String>,
) -> Result<Json<crate::api::types::AgentInfo>, Json<ErrorResponse>> {
    Ok(Json(crate::api::types::AgentInfo {
        id: agent_id,
        name: "agent-1".to_string(),
        profile: "default".to_string(),
        status: "idle".to_string(),
        task_count: 0,
    }))
}

/// Delete (kill) an agent.
pub async fn kill_agent<C: LLMClient + Clone + 'static>(
    State(_state): State<Arc<AppState<C>>>,
    Path(agent_id): Path<String>,
    Json(_request): Json<KillAgentRequest>,
) -> Result<Json<KillAgentResponse>, Json<ErrorResponse>> {
    // TODO: Actually kill the agent
    let _ = agent_id;
    Ok(Json(KillAgentResponse { success: true }))
}

/// Switch to a different agent.
pub async fn switch_agent<C: LLMClient + Clone + 'static>(
    State(_state): State<Arc<AppState<C>>>,
    Path(agent_id): Path<String>,
    Json(request): Json<SwitchAgentRequest>,
) -> Result<Json<SwitchAgentResponse>, Json<ErrorResponse>> {
    // TODO: Actually switch the agent
    let _ = request;
    Ok(SwitchAgentResponse {
        success: true,
        active_agent_id: agent_id,
    }.into())
}

/// Chat with a specific agent (non-streaming).
pub async fn agent_chat<C: LLMClient + Clone + 'static>(
    State(_state): State<Arc<AppState<C>>>,
    Path(agent_id): Path<String>,
    Json(request): Json<crate::api::types::AgentChatRequest>,
) -> Result<Json<AgentChatResponse>, Json<ErrorResponse>> {
    Ok(AgentChatResponse {
        content: format!("Agent '{}' received: {}", agent_id, request.message),
        tool_calls: vec![],
        stop_reason: "end_turn".to_string(),
    }.into())
}

/// Stream events from a specific agent.
pub async fn agent_stream<C: LLMClient + Clone + 'static>(
    State(_state): State<Arc<AppState<C>>>,
    Path(agent_id): Path<String>,
    axum::extract::Query(params): axum::extract::Query<HashMap<String, String>>,
) -> Result<Sse<impl futures::Stream<Item = Result<Event, std::convert::Infallible>>>, Json<ErrorResponse>> {
    let message = params
        .get("message")
        .cloned()
        .unwrap_or_else(|| "Hello".to_string());

    // Create a stream that yields events
    let stream = stream::iter(vec![
        Ok(Event::default().data(format!("Agent {}: Processing '{}'", agent_id, message))),
        Ok(Event::default().data("Agent: Working on it...".to_string())),
        Ok(Event::default().data("Agent: Done!".to_string())),
    ]);

    Ok(Sse::new(stream))
}
