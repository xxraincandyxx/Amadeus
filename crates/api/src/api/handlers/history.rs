// @amadeus-header
// summary: HTTP handler implementation for history routes.
// layer: api
// status: active
// feature_flags:
// - api
// provides:
// - module: crate::api::handlers::history
// - fn: crate::api::handlers::history::get_history
// uses:
// - module: crate::api::http::AppState
// - module: crate::api::types::HistoryResponse
// - module: crate::client::LLMClient
// - protocol: axum HTTP handlers
// invariants:
// - Handler request and response handling stays aligned with route contracts.
// side_effects:
// - Performs network or HTTP operations.
// tests:
// - tests/agent_integration_test.rs
// @end-amadeus-header

//! # History Handler
//!
//! Handles the conversation history endpoint.

use axum::{extract::State, Json};
use std::sync::Arc;

use crate::api::http::AppState;
use crate::api::types::HistoryResponse;
use crate::client::LLMClient;

/// GET /history
///
/// Get the current conversation history.
///
/// Note: In the current stateless REST implementation, this returns an empty
/// history unless using a session-aware endpoint. For stateful conversations,
/// use the /stream endpoint with a persistent connection or /sessions endpoints.
pub async fn get_history<C: LLMClient + Clone + 'static>(
    State(_state): State<Arc<AppState<C>>>,
) -> Json<HistoryResponse> {
    // In a stateful implementation, we would access the agent's history here.
    // For the stateless REST API, we return an empty history.
    // Use /sessions endpoints to access saved conversation histories.

    Json(HistoryResponse {
        messages: vec![],
        total: 0,
    })
}
