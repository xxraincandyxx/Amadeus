// @amadeus-header
// summary: HTTP handler implementation for tasks routes.
// layer: api
// status: active
// feature_flags:
// - api
// provides:
// - module: crate::api::handlers::tasks
// - fn: crate::api::handlers::tasks::handle_task
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

use crate::agent::worker::Task;
use crate::api::http::AppState;
use crate::api::types::{TaskRequest, TaskResponse};
use crate::client::LLMClient;
use axum::{extract::State, Json};
use std::sync::Arc;

/// Handle a multi-agent task request.
pub async fn handle_task<C: LLMClient + Clone + 'static>(
    State(state): State<Arc<AppState<C>>>,
    Json(payload): Json<TaskRequest>,
) -> Json<TaskResponse> {
    let task = Task::new(payload.id, payload.prompt).requires(payload.capabilities);
    let mut agent_manager = state.agent_manager.write().await;

    match agent_manager
        .execute_task(Some(state.default_team_id), task)
        .await
    {
        Ok(res) => Json(TaskResponse {
            task_id: res.task_id,
            worker_id: res.worker_id.to_string(),
            success: res.success,
            output: res.output,
            error: res.error,
            duration_ms: res.duration_ms,
        }),
        Err(e) => {
            let error_msg = e.to_string();
            Json(TaskResponse {
                task_id: "error".to_string(),
                worker_id: "system".to_string(),
                success: false,
                output: None,
                error: Some(error_msg),
                duration_ms: 0,
            })
        }
    }
}
