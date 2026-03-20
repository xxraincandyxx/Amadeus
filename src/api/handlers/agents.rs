//! # Agent Management Handlers
//!
//! HTTP handlers for multi-agent management endpoints.

use axum::{
    extract::{Path, State},
    response::sse::Sse,
    Json,
};
use std::collections::HashMap;
use std::sync::Arc;

use crate::agent::profile::AgentProfile;
use crate::api::http::AppState;
use crate::api::types::{
    AgentChatResponse, CreateAgentRequest, CreateAgentResponse, ErrorResponse, KillAgentRequest,
    KillAgentResponse, ListAgentsResponse, SwitchAgentRequest, SwitchAgentResponse,
};
use crate::client::LLMClient;
use tokio::sync::RwLock;

/// List all agents.
pub async fn list_agents<C: LLMClient + Clone + 'static>(
    State(state): State<Arc<AppState<C>>>,
) -> Result<Json<ListAgentsResponse>, Json<ErrorResponse>> {
    let agent_manager = state.agent_manager.read().await;
    let agents = agent_manager.list_agents();
    let active_id = agent_manager.active_agent_id();

    let agents_api = agents
        .into_iter()
        .map(|a| crate::api::types::AgentInfo {
            id: a.id.to_string(),
            name: a.name,
            profile: a.profile.to_string(),
            status: format!("{:?}", a.status).to_lowercase(),
            task_count: a.task_count,
        })
        .collect();

    Ok(Json(ListAgentsResponse {
        agents: agents_api,
        active_agent_id: active_id.map(|id| id.to_string()),
    }))
}

/// Create a new agent.
pub async fn create_agent<C: LLMClient + Clone + 'static>(
    State(state): State<Arc<AppState<C>>>,
    Json(request): Json<CreateAgentRequest>,
) -> Result<Json<CreateAgentResponse>, Json<ErrorResponse>> {
    let profile = match request.profile.as_str() {
        "default" => AgentProfile::Default,
        "debug" => AgentProfile::Debug,
        "docs" => AgentProfile::Docs,
        "review" | "code_review" => AgentProfile::CodeReview,
        _ => AgentProfile::Custom(format!("Custom profile: {}", request.profile)),
    };

    let mut agent_manager = state.agent_manager.write().await;
    match agent_manager.create_agent(request.name, profile).await {
        Ok(agent_id) => {
            if let Some(agent_info) = agent_manager.get_agent(&agent_id) {
                Ok(Json(CreateAgentResponse {
                    agent: crate::api::types::AgentInfo {
                        id: agent_info.id.to_string(),
                        name: agent_info.name,
                        profile: agent_info.profile.to_string(),
                        status: format!("{:?}", agent_info.status).to_lowercase(),
                        task_count: agent_info.task_count,
                    },
                }))
            } else {
                Err(Json(ErrorResponse::new(
                    "AgentError",
                    "Failed to get agent info after creation",
                )))
            }
        }
        Err(e) => Err(Json(ErrorResponse::from_agent_error(&e))),
    }
}

/// Get info for a specific agent.
pub async fn get_agent<C: LLMClient + Clone + 'static>(
    State(_state): State<Arc<AppState<C>>>,
    Path(agent_id): Path<String>,
) -> Result<Json<crate::api::types::AgentInfo>, Json<ErrorResponse>> {
    use crate::core::id::AgentId;

    let _agent_uuid = agent_id
        .parse::<AgentId>()
        .map_err(|_| Json(ErrorResponse::new("InvalidAgentId", "Invalid agent ID format")))?;

    // For now, return a placeholder since get_agent returns Option
    Ok(Json(crate::api::types::AgentInfo {
        id: agent_id,
        name: "agent".to_string(),
        profile: "default".to_string(),
        status: "idle".to_string(),
        task_count: 0,
    }))
}

/// Delete (kill) an agent.
pub async fn kill_agent<C: LLMClient + Clone + 'static>(
    State(state): State<Arc<AppState<C>>>,
    Path(agent_id): Path<String>,
    Json(_request): Json<KillAgentRequest>,
) -> Result<Json<KillAgentResponse>, Json<ErrorResponse>> {
    use crate::core::id::AgentId;

    let agent_uuid = agent_id
        .parse::<AgentId>()
        .map_err(|_| Json(ErrorResponse::new("InvalidAgentId", "Invalid agent ID format")))?;

    let mut agent_manager = state.agent_manager.write().await;
    match agent_manager.kill(&agent_uuid) {
        Ok(()) => Ok(Json(KillAgentResponse { success: true })),
        Err(e) => Err(Json(ErrorResponse::from_agent_error(&e))),
    }
}

/// Switch to a different agent.
pub async fn switch_agent<C: LLMClient + Clone + 'static>(
    State(state): State<Arc<AppState<C>>>,
    Path(agent_id): Path<String>,
    Json(_request): Json<SwitchAgentRequest>,
) -> Result<Json<SwitchAgentResponse>, Json<ErrorResponse>> {
    use crate::core::id::AgentId;

    let agent_uuid = agent_id
        .parse::<AgentId>()
        .map_err(|_| Json(ErrorResponse::new("InvalidAgentId", "Invalid agent ID format")))?;

    let mut agent_manager = state.agent_manager.write().await;
    match agent_manager.switch_to(&agent_uuid) {
        Ok(()) => {
            let new_active = agent_manager
                .active_agent_id()
                .map(|id| id.to_string())
                .unwrap_or_default();
            Ok(Json(SwitchAgentResponse {
                success: true,
                active_agent_id: new_active,
            }))
        }
        Err(e) => Err(Json(ErrorResponse::from_agent_error(&e))),
    }
}

/// Chat with a specific agent (non-streaming).
pub async fn agent_chat<C: LLMClient + Clone + 'static>(
    State(_state): State<Arc<AppState<C>>>,
    Path(agent_id): Path<String>,
    Json(request): Json<crate::api::types::AgentChatRequest>,
) -> Result<Json<AgentChatResponse>, Json<ErrorResponse>> {
    // TODO: Implement actual chat with agent
    Ok(Json(AgentChatResponse {
        content: format!("Agent '{}' received: {}", agent_id, request.message),
        tool_calls: vec![],
        stop_reason: "end_turn".to_string(),
    }))
}

/// Stream events from a specific agent.
pub async fn agent_stream<C: LLMClient + Clone + 'static>(
    State(_state): State<Arc<AppState<C>>>,
    Path(agent_id): Path<String>,
    axum::extract::Query(params): axum::extract::Query<HashMap<String, String>>,
) -> Result<Sse<impl futures::Stream<Item = Result<axum::response::sse::Event, std::convert::Infallible>>>, Json<ErrorResponse>> {
    use axum::response::sse::Event;
    use futures::stream;

    let message = params
        .get("message")
        .cloned()
        .unwrap_or_else(|| "Hello".to_string());

    // TODO: Implement actual streaming from agent
    let stream = stream::iter(vec![
        Ok(Event::default().data(format!("Agent {}: Processing '{}'", agent_id, message))),
        Ok(Event::default().data("Agent: Working on it...".to_string())),
        Ok(Event::default().data("Agent: Done!".to_string())),
    ]);

    Ok(Sse::new(stream))
}
