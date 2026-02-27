//! # Execute Handler
//!
//! Handles POST /execute requests to run bash commands directly.

use std::sync::Arc;
use axum::{extract::State, Json};

use crate::api::types::{ErrorResponse, ExecuteRequest, ExecuteResponse};
use crate::api::http::AppState;
use crate::client::LLMClient;
use crate::tools::bash::BashTool;
use crate::tools::tool_trait::Tool;

/// Process a command execution request.
pub async fn execute<C: LLMClient + Clone + 'static>(
    State(state): State<Arc<AppState<C>>>,
    Json(request): Json<ExecuteRequest>,
) -> std::result::Result<Json<ExecuteResponse>, Json<ErrorResponse>> {
    let bash = BashTool::from_config(state.supervisor.config());
    
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
