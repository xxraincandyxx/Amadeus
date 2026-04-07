// @amadeus-header
// summary: HTTP handler implementation for sessions routes.
// layer: api
// status: active
// feature_flags:
// - api
// provides:
// - module: crate::api::handlers::sessions
// - fn: crate::api::handlers::sessions::list_sessions
// - fn: crate::api::handlers::sessions::get_session
// - fn: crate::api::handlers::sessions::restore_session
// uses:
// - module: crate::agent::loop_agent::Agent
// - module: crate::agent::messages::ContentBlock
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

//! # Sessions Handler
//!
//! Handles session management endpoints for listing, loading, and restoring
//! conversation sessions.

use axum::{
    extract::{Path, State},
    Json,
};
use std::sync::Arc;

use crate::agent::loop_agent::Agent;
use crate::agent::messages::ContentBlock;
use crate::api::http::AppState;
use crate::api::types::{
    ErrorResponse, RestoreSessionRequest, RestoreSessionResponse, SessionDetailResponse,
    SessionStatsResponse, SessionSummary, SessionsResponse, TodoSummary,
};
use crate::client::LLMClient;

/// Extract text content from a message's content blocks.
fn extract_text_content(content: &[ContentBlock]) -> String {
    content
        .iter()
        .filter_map(|block| match block {
            ContentBlock::Text { text } => Some(text.clone()),
            ContentBlock::ToolResult { content, .. } => Some(content.clone()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// GET /sessions
///
/// List all available conversation sessions.
pub async fn list_sessions<C: LLMClient + Clone + 'static>(
    State(state): State<Arc<AppState<C>>>,
) -> std::result::Result<Json<SessionsResponse>, Json<ErrorResponse>> {
    let agent = crate::agent::loop_agent::AgentBuilder::new(
        state.client.clone(),
        Arc::clone(&state.config),
    )
    .build();

    match agent.list_sessions() {
        Ok(sessions) => {
            let summaries: Vec<SessionSummary> = sessions
                .into_iter()
                .map(|(path, session)| {
                    let id = path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("unknown")
                        .to_string();
                    SessionSummary {
                        id,
                        timestamp: session.timestamp,
                        model: session.model,
                        total_tokens: session.stats.total_tokens,
                        tool_calls: session.stats.tool_calls,
                        duration_ms: session.stats.duration_ms,
                        message_count: session.history.len(),
                        todo_count: session.todos.len(),
                    }
                })
                .collect();

            Ok(Json(SessionsResponse {
                sessions: summaries,
            }))
        }
        Err(e) => Err(Json(ErrorResponse::new("SessionListError", e.to_string()))),
    }
}

/// GET /sessions/:id
///
/// Get details of a specific session.
pub async fn get_session<C: LLMClient + Clone + 'static>(
    State(state): State<Arc<AppState<C>>>,
    Path(session_id): Path<String>,
) -> std::result::Result<Json<SessionDetailResponse>, Json<ErrorResponse>> {
    let config = &state.config;
    let log_dir = match &config.session_log_dir {
        Some(dir) => dir.clone(),
        None => {
            return Err(Json(ErrorResponse::new(
                "SessionNotFound",
                "Session logging is not configured",
            )))
        }
    };

    // Try both .json and .json.gz extensions
    let path = log_dir.join(&session_id);
    let path = if path.exists() {
        path
    } else {
        let gz_path = log_dir.join(format!("{}.gz", session_id));
        if gz_path.exists() {
            gz_path
        } else {
            return Err(Json(ErrorResponse::new(
                "SessionNotFound",
                format!("Session '{}' not found", session_id),
            )));
        }
    };

    // load_session is an associated function, not a method
    match Agent::<C>::load_session(&path) {
        Ok(session) => {
            let history = session
                .history
                .iter()
                .map(|msg| crate::api::types::MessageSummary {
                    role: msg.role.clone(),
                    content: extract_text_content(&msg.content),
                })
                .collect();

            Ok(Json(SessionDetailResponse {
                id: session_id,
                timestamp: session.timestamp,
                model: session.model,
                system_prompt: session.system_prompt,
                history,
                todos: session
                    .todos
                    .into_iter()
                    .map(|todo| TodoSummary {
                        id: todo.id,
                        text: todo.text,
                        status: todo.status.to_string(),
                    })
                    .collect(),
                stats: SessionStatsResponse {
                    total_tokens: session.stats.total_tokens,
                    tool_calls: session.stats.tool_calls,
                    duration_ms: session.stats.duration_ms,
                },
            }))
        }
        Err(e) => Err(Json(ErrorResponse::new("SessionLoadError", e.to_string()))),
    }
}

/// POST /sessions/:id/restore
///
/// Restore a session into the current conversation history.
pub async fn restore_session<C: LLMClient + Clone + 'static>(
    State(state): State<Arc<AppState<C>>>,
    Path(session_id): Path<String>,
    Json(_request): Json<RestoreSessionRequest>,
) -> std::result::Result<Json<RestoreSessionResponse>, Json<ErrorResponse>> {
    let config = &state.config;
    let log_dir = match &config.session_log_dir {
        Some(dir) => dir.clone(),
        None => {
            return Err(Json(ErrorResponse::new(
                "SessionNotFound",
                "Session logging is not configured",
            )))
        }
    };

    // Try both .json and .json.gz extensions
    let path = log_dir.join(&session_id);
    let path = if path.exists() {
        path
    } else {
        let gz_path = log_dir.join(format!("{}.gz", session_id));
        if gz_path.exists() {
            gz_path
        } else {
            return Err(Json(ErrorResponse::new(
                "SessionNotFound",
                format!("Session '{}' not found", session_id),
            )));
        }
    };

    // load_session is an associated function, not a method
    let session = match Agent::<C>::load_session(&path) {
        Ok(s) => s,
        Err(e) => return Err(Json(ErrorResponse::new("SessionLoadError", e.to_string()))),
    };

    let message_count = session.history.len();

    // Note: For stateless REST API, session restore is informational.
    // The caller would need to use the restored history in subsequent requests.
    // In a stateful implementation, we would restore into the agent's history here.

    Ok(Json(RestoreSessionResponse {
        success: true,
        message_count,
    }))
}
