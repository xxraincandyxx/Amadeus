//! # Approvals Handler
//!
//! Handles approval flow endpoints for tool execution permissions.

use axum::{extract::Path, extract::State, Json};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc;

use crate::agent::events::ApprovalDecision;
use crate::api::http::AppState;
use crate::api::types::{ApprovalRequest, ApprovalResponse, ErrorResponse};
use crate::client::LLMClient;

/// Global approval channels registry.
/// Maps approval IDs to their response channels.
pub static APPROVAL_CHANNELS: std::sync::OnceLock<
    tokio::sync::RwLock<HashMap<String, mpsc::Sender<ApprovalDecision>>>,
> = std::sync::OnceLock::new();

fn get_approval_channels(
) -> &'static tokio::sync::RwLock<HashMap<String, mpsc::Sender<ApprovalDecision>>> {
    APPROVAL_CHANNELS.get_or_init(|| tokio::sync::RwLock::new(HashMap::new()))
}

/// Register an approval channel for a pending approval.
pub async fn register_approval_channel(approval_id: String, tx: mpsc::Sender<ApprovalDecision>) {
    let mut channels = get_approval_channels().write().await;
    channels.insert(approval_id, tx);
}

/// Remove an approval channel.
pub async fn remove_approval_channel(approval_id: &str) {
    let mut channels = get_approval_channels().write().await;
    channels.remove(approval_id);
}

/// POST /approvals/:id
///
/// Submit an approval decision for a pending tool execution.
///
/// This endpoint is used in conjunction with the SSE stream's `approval_request`
/// events. When the agent needs approval for a tool, it sends an event with an
/// approval ID. The client can then use this endpoint to approve, deny, or
/// modify the request.
pub async fn submit_approval<C: LLMClient + Clone + 'static>(
    State(_state): State<Arc<AppState<C>>>,
    Path(approval_id): Path<String>,
    Json(request): Json<ApprovalRequest>,
) -> std::result::Result<Json<ApprovalResponse>, Json<ErrorResponse>> {
    // Parse the decision
    let decision = match request.decision.to_lowercase().as_str() {
        "approve" | "approved" | "yes" | "allow" => ApprovalDecision::Approve,
        "always" | "always_approve" => ApprovalDecision::AlwaysApprove,
        "deny" | "denied" | "no" | "reject" => ApprovalDecision::Deny,
        _ => {
            return Err(Json(ErrorResponse::new(
                "InvalidDecision",
                format!(
                    "Invalid decision '{}'. Must be 'approve', 'deny', or 'always'",
                    request.decision
                ),
            )))
        }
    };

    // Try to send the decision to the waiting agent
    let channels = get_approval_channels().read().await;
    if let Some(tx) = channels.get(&approval_id) {
        match tx.send(decision).await {
            Ok(_) => Ok(Json(ApprovalResponse {
                success: true,
                decision: request.decision,
            })),
            Err(_) => Err(Json(ErrorResponse::new(
                "ChannelClosed",
                "The approval request has expired or been cancelled",
            ))),
        }
    } else {
        Err(Json(ErrorResponse::new(
            "ApprovalNotFound",
            format!("No pending approval with id '{}'", approval_id),
        )))
    }
}

/// GET /approvals
///
/// List all pending approvals (for debugging/admin purposes).
pub async fn list_pending_approvals<C: LLMClient + Clone + 'static>(
    State(_state): State<Arc<AppState<C>>>,
) -> Json<Vec<PendingApproval>> {
    let channels = get_approval_channels().read().await;
    let pending: Vec<PendingApproval> = channels
        .keys()
        .map(|id| PendingApproval {
            id: id.clone(),
            status: "pending".to_string(),
        })
        .collect();
    Json(pending)
}

/// Summary of a pending approval.
#[derive(Debug, Serialize)]
pub struct PendingApproval {
    pub id: String,
    pub status: String,
}
