// @amadeus-header
// summary: HTTP handler implementation for chat routes.
// layer: api
// status: active
// feature_flags:
// - api
// provides:
// - module: crate::api::handlers::chat
// - fn: crate::api::handlers::chat::chat
// uses:
// - module: crate::agent::worker::Task
// - module: crate::api::http::AppState
// - module: crate::api::types
// - module: crate::client::LLMClient
// - protocol: axum HTTP handlers
// invariants:
// - Handler request and response handling stays aligned with route contracts.
// side_effects:
// - Performs network or HTTP operations.
// tests:
// - tests/agent_integration_test.rs
// @end-amadeus-header

//! # Chat Handler
//!
//! Handles POST /chat requests to send messages to the agent.

use axum::{extract::State, Json};
use std::sync::Arc;

use crate::agent::worker::Task;
use crate::api::http::AppState;
use crate::api::types::{ChatRequest, ChatResponse, ErrorResponse};
use crate::client::LLMClient;

/// Process a chat request and return the agent's response.
pub async fn chat<C: LLMClient + Clone + 'static>(
    State(state): State<Arc<AppState<C>>>,
    Json(request): Json<ChatRequest>,
) -> std::result::Result<Json<ChatResponse>, Json<ErrorResponse>> {
    let task = Task::new("chat-request".to_string(), request.message);
    let mut agent_manager = state.agent_manager.write().await;

    match agent_manager
        .execute_task(Some(state.default_team_id), task)
        .await
    {
        Ok(result) => Ok(Json(ChatResponse {
            content: result.output.unwrap_or_default(),
            tool_calls: result
                .tool_calls
                .into_iter()
                .map(|tc| crate::api::types::ToolCall {
                    name: tc.name,
                    input: tc.input,
                    output: tc.output,
                })
                .collect(),
            stop_reason: "end_turn".to_string(),
        })),
        Err(e) => Err(Json(ErrorResponse::from_agent_error(&e))),
    }
}
