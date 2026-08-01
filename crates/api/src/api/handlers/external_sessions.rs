// @amadeus-header
// summary: Versioned session-oriented HTTP interface for external GUI and SDK clients.
// layer: api
// status: active
// feature_flags:
// - api
// provides:
// - module: crate::api::handlers::external_sessions
// - route: /v1/sessions
// - route: /v1/sessions/:id
// - route: /v1/sessions/:id/messages
// - route: /v1/sessions/:id/events
// - route: /v1/sessions/:id/history
// - route: /v1/sessions/:id/approvals
// - route: /v1/sessions/:id/checkpoint
// - route: /v1/sessions/:id/compact
// - route: /v1/sessions/:id/cancel
// uses:
// - module: crate::bridge
// - module: crate::api::http
// - protocol: JSON and server-sent events
// invariants:
// - External interactive operations are scoped by session identifier.
// - Event streams send periodic keep-alives while sessions are idle.
// side_effects:
// - Starts, compacts, and cancels agent turns.
// - Sends approval decisions across async channels.
// tests:
// - cmd: cargo test -p api --features full
// @end-amadeus-header

use std::collections::HashMap;
use std::convert::Infallible;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::Json;
use futures::stream::{self, Stream};
use serde::{Deserialize, Serialize};

use crate::agent::{AgentProfile, ApprovalDecision, Message, SessionCheckpoint};
use crate::api::handlers::agents::bridge_event_to_sse;
use crate::api::http::AppState;
use crate::api::types::ErrorResponse;
use crate::bridge::BridgeSessionInfo;
use crate::client::LLMClient;

type ApiResult<T> = Result<T, (StatusCode, Json<ErrorResponse>)>;
type BoxedSseStream = Pin<Box<dyn Stream<Item = Result<Event, Infallible>> + Send>>;

#[derive(Debug, Deserialize)]
pub struct CreateSessionRequest {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default = "default_profile")]
    pub profile: String,
}

#[derive(Debug, Serialize)]
pub struct SessionListResponse {
    pub sessions: Vec<BridgeSessionInfo>,
    pub active_session_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SubmitMessageRequest {
    pub content: String,
}

#[derive(Debug, Serialize)]
pub struct AcceptedResponse {
    pub accepted: bool,
    pub session_id: String,
}

#[derive(Debug, Serialize)]
pub struct HistoryResponse {
    pub messages: Vec<Message>,
    pub total: usize,
}

#[derive(Debug, Deserialize)]
pub struct ApprovalDecisionRequest {
    pub decision: String,
}

#[derive(Debug, Serialize)]
pub struct OperationResponse {
    pub success: bool,
}

#[derive(Debug, Serialize)]
/// Result of a manual external-session compaction operation.
pub struct CompactionResponse {
    pub original_count: usize,
    pub compacted_count: usize,
    pub original_tokens: usize,
    pub new_tokens: usize,
    pub tokens_saved: usize,
    pub messages_summarized: usize,
    pub status: String,
}

fn default_profile() -> String {
    "default".to_string()
}

fn profile_from_string(profile: &str) -> AgentProfile {
    match profile {
        "default" => AgentProfile::Default,
        "debug" => AgentProfile::Debug,
        "docs" => AgentProfile::Docs,
        "review" | "code_review" => AgentProfile::CodeReview,
        other => AgentProfile::Custom(other.to_string()),
    }
}

fn api_error(error: crate::error::AgentError) -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::BAD_REQUEST,
        Json(ErrorResponse::from_agent_error(&error)),
    )
}

fn compaction_status_label(status: crate::agent::CompressionStatus) -> String {
    match status {
        crate::agent::CompressionStatus::Compressed => "compressed",
        crate::agent::CompressionStatus::Inflated => "inflated",
        crate::agent::CompressionStatus::EmptySummary => "empty_summary",
        crate::agent::CompressionStatus::Noop => "noop",
        crate::agent::CompressionStatus::TruncatedOnly => "truncated_only",
    }
    .to_string()
}

pub async fn list_external_sessions<C: LLMClient + Clone + 'static>(
    State(state): State<Arc<AppState<C>>>,
) -> Json<SessionListResponse> {
    Json(SessionListResponse {
        sessions: state.session_bridge.list_sessions().await,
        active_session_id: state.session_bridge.active_session_id().await,
    })
}

pub async fn create_external_session<C: LLMClient + Clone + 'static>(
    State(state): State<Arc<AppState<C>>>,
    Json(request): Json<CreateSessionRequest>,
) -> ApiResult<(StatusCode, Json<BridgeSessionInfo>)> {
    let session = state
        .session_bridge
        .create_session(request.name, profile_from_string(&request.profile))
        .await
        .map_err(api_error)?;
    Ok((StatusCode::CREATED, Json(session)))
}

pub async fn get_external_session<C: LLMClient + Clone + 'static>(
    State(state): State<Arc<AppState<C>>>,
    Path(session_id): Path<String>,
) -> ApiResult<Json<BridgeSessionInfo>> {
    state
        .session_bridge
        .get_session(&session_id)
        .await
        .map(Json)
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse::new("SessionNotFound", "Session not found")),
            )
        })
}

pub async fn close_external_session<C: LLMClient + Clone + 'static>(
    State(state): State<Arc<AppState<C>>>,
    Path(session_id): Path<String>,
) -> ApiResult<Json<OperationResponse>> {
    state
        .session_bridge
        .close_session(&session_id)
        .await
        .map_err(api_error)?;
    Ok(Json(OperationResponse { success: true }))
}

pub async fn submit_external_message<C: LLMClient + Clone + 'static>(
    State(state): State<Arc<AppState<C>>>,
    Path(session_id): Path<String>,
    Json(request): Json<SubmitMessageRequest>,
) -> ApiResult<(StatusCode, Json<AcceptedResponse>)> {
    if request.content.trim().is_empty() {
        return Err((
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(ErrorResponse::new(
                "InvalidMessage",
                "Message content is empty",
            )),
        ));
    }
    state
        .session_bridge
        .submit_input(&session_id, request.content)
        .await
        .map_err(api_error)?;
    Ok((
        StatusCode::ACCEPTED,
        Json(AcceptedResponse {
            accepted: true,
            session_id,
        }),
    ))
}

pub async fn external_session_events<C: LLMClient + Clone + 'static>(
    State(state): State<Arc<AppState<C>>>,
    Path(session_id): Path<String>,
    Query(_params): Query<HashMap<String, String>>,
) -> ApiResult<Sse<BoxedSseStream>> {
    let rx = state
        .session_bridge
        .subscribe(&session_id)
        .await
        .map_err(api_error)?;
    let context_window_size = state.config.context_window_size;
    let events = stream::unfold(rx, move |mut rx| async move {
        loop {
            match rx.recv().await {
                Ok(event) => {
                    if let Some(event) = bridge_event_to_sse(event, context_window_size) {
                        return Some((event, rx));
                    }
                }
                Err(_) => return None,
            }
        }
    });
    let events: BoxedSseStream = Box::pin(events);
    Ok(Sse::new(events).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(1))
            .text("keep-alive"),
    ))
}

pub async fn external_session_history<C: LLMClient + Clone + 'static>(
    State(state): State<Arc<AppState<C>>>,
    Path(session_id): Path<String>,
) -> ApiResult<Json<HistoryResponse>> {
    let messages = state
        .session_bridge
        .history(&session_id)
        .await
        .map_err(api_error)?;
    let total = messages.len();
    Ok(Json(HistoryResponse { messages, total }))
}

pub async fn list_external_session_approvals<C: LLMClient + Clone + 'static>(
    State(state): State<Arc<AppState<C>>>,
    Path(session_id): Path<String>,
) -> ApiResult<Json<Vec<crate::agent::ApprovalRequest>>> {
    state
        .session_bridge
        .pending_approvals(&session_id)
        .await
        .map(Json)
        .map_err(api_error)
}

pub async fn submit_external_approval<C: LLMClient + Clone + 'static>(
    State(state): State<Arc<AppState<C>>>,
    Path((session_id, approval_id)): Path<(String, String)>,
    Json(request): Json<ApprovalDecisionRequest>,
) -> ApiResult<Json<OperationResponse>> {
    let decision = match request.decision.as_str() {
        "approve" => ApprovalDecision::Approve,
        "always_approve" => ApprovalDecision::AlwaysApprove,
        "deny" => ApprovalDecision::Deny,
        _ => {
            return Err((
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(ErrorResponse::new(
                    "InvalidDecision",
                    "Decision must be approve, always_approve, or deny",
                )),
            ))
        }
    };
    state
        .session_bridge
        .submit_approval(&session_id, &approval_id, decision)
        .await
        .map_err(api_error)?;
    Ok(Json(OperationResponse { success: true }))
}

pub async fn external_session_checkpoint<C: LLMClient + Clone + 'static>(
    State(state): State<Arc<AppState<C>>>,
    Path(session_id): Path<String>,
) -> ApiResult<Json<SessionCheckpoint>> {
    state
        .session_bridge
        .checkpoint(&session_id)
        .await
        .map(Json)
        .map_err(api_error)
}

pub async fn restore_external_session_checkpoint<C: LLMClient + Clone + 'static>(
    State(state): State<Arc<AppState<C>>>,
    Path(session_id): Path<String>,
    Json(checkpoint): Json<SessionCheckpoint>,
) -> ApiResult<Json<OperationResponse>> {
    state
        .session_bridge
        .restore_checkpoint(&session_id, &checkpoint)
        .await
        .map_err(api_error)?;
    Ok(Json(OperationResponse { success: true }))
}

/// Compact one idle external session and return the resulting history statistics.
pub async fn compact_external_session<C: LLMClient + Clone + 'static>(
    State(state): State<Arc<AppState<C>>>,
    Path(session_id): Path<String>,
) -> ApiResult<Json<CompactionResponse>> {
    let result = state
        .session_bridge
        .compact(&session_id)
        .await
        .map_err(api_error)?;
    Ok(Json(CompactionResponse {
        original_count: result.original_count,
        compacted_count: result.compacted_count,
        original_tokens: result.original_tokens,
        new_tokens: result.new_tokens,
        tokens_saved: result.tokens_saved,
        messages_summarized: result.messages_summarized,
        status: compaction_status_label(result.status),
    }))
}

pub async fn cancel_external_session<C: LLMClient + Clone + 'static>(
    State(state): State<Arc<AppState<C>>>,
    Path(session_id): Path<String>,
) -> ApiResult<Json<OperationResponse>> {
    state
        .session_bridge
        .cancel(&session_id)
        .await
        .map_err(api_error)?;
    Ok(Json(OperationResponse { success: true }))
}
