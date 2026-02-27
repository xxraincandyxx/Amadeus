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
    // For single chat, we can dispatch a generic task to the supervisor
    let task = Task::new("chat-request".to_string(), request.message);

    match state.supervisor.execute(task).await {
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
