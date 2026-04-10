// @amadeus-header
// summary: HTTP handler implementation for execute routes.
// layer: api
// status: active
// feature_flags:
// - api
// provides:
// - module: crate::api::handlers::execute
// - fn: crate::api::handlers::execute::execute
// uses:
// - module: crate::api::http::AppState
// - module: crate::api::types
// - module: crate::client::LLMClient
// - module: crate::tools::bash::BashTool
// - module: crate::tools::tool_trait::Tool
// - protocol: axum HTTP handlers
// invariants:
// - Handler request and response handling stays aligned with route contracts.
// side_effects:
// - Performs network or HTTP operations.
// tests:
// - tests/agent_integration_test.rs
// @end-amadeus-header

//! # Execute Handler
//!
//! Handles POST /execute requests to run bash commands directly.

use axum::{extract::State, Json};
use std::sync::Arc;

use crate::api::http::AppState;
use crate::api::types::{ErrorResponse, ExecuteRequest, ExecuteResponse};
use crate::client::LLMClient;
use crate::tools::bash::BashTool;
use crate::tools::tool_trait::Tool;

/// Process a command execution request.
pub async fn execute<C: LLMClient + Clone + 'static>(
    State(state): State<Arc<AppState<C>>>,
    Json(request): Json<ExecuteRequest>,
) -> std::result::Result<Json<ExecuteResponse>, Json<ErrorResponse>> {
    let bash = BashTool::from_config(&state.config);

    let input = serde_json::json!({
        "command": request.command,
    });

    match bash.execute(input).await {
        Ok(output) => Ok(Json(ExecuteResponse {
            output,
            exit_code: 0,
            timed_out: false,
        })),
        Err(e) => Err(Json(ErrorResponse::from_agent_error(&e))),
    }
}
