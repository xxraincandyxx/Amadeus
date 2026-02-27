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

    match state.supervisor.execute(task).await {
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
