// @amadeus-header
// summary: Session listing, detail, history, statistics, todo, and restore API data types.
// layer: api
// status: active
// feature_flags:
// - api
// provides:
// - type: crate::api::types::SessionDetailResponse
// - type: crate::api::types::SessionsResponse
// - type: crate::api::types::SessionSummary
// uses:
// - protocol: serde serialization
// invariants:
// - Serialized fields retain the session HTTP contract.
// side_effects: none
// tests:
// - cmd: cargo test -p api --all-features
// @end-amadeus-header

use serde::{Deserialize, Serialize};

/*
 * ============================================================================
 * SESSION ENDPOINT TYPES
 * ============================================================================
 */

/// Response for the `/sessions` endpoint.
///
/// Lists all available conversation sessions.
#[derive(Debug, Serialize)]
pub struct SessionsResponse {
    /// List of available sessions.
    pub sessions: Vec<SessionSummary>,
}

/// Summary of a single session.
#[derive(Debug, Serialize)]
pub struct SessionSummary {
    /// Unique session identifier (filename).
    pub id: String,
    /// ISO 8601 timestamp of when the session was created.
    pub timestamp: String,
    /// Model used for this session.
    pub model: String,
    /// Total tokens used in this session.
    pub total_tokens: u32,
    /// Number of tool calls made.
    pub tool_calls: usize,
    /// Duration of the session in milliseconds.
    pub duration_ms: u64,
    /// Number of messages in the conversation.
    pub message_count: usize,
    /// Number of todos stored with the session.
    pub todo_count: usize,
}

/// Response for the `/sessions/{id}` endpoint.
///
/// Full details of a specific session.
#[derive(Debug, Serialize)]
pub struct SessionDetailResponse {
    /// Unique session identifier.
    pub id: String,
    /// ISO 8601 timestamp of when the session was created.
    pub timestamp: String,
    /// Model used for this session.
    pub model: String,
    /// System prompt used.
    pub system_prompt: String,
    /// Conversation history.
    pub history: Vec<MessageSummary>,
    /// Todos captured in the session.
    pub todos: Vec<TodoSummary>,
    /// Session statistics.
    pub stats: SessionStatsResponse,
}

/// Summary of a todo in the session.
#[derive(Debug, Serialize)]
pub struct TodoSummary {
    /// Stable todo identifier.
    pub id: String,
    /// Todo description.
    pub text: String,
    /// Current status.
    pub status: String,
}

/// Statistics for a session.
#[derive(Debug, Serialize)]
pub struct SessionStatsResponse {
    /// Total tokens used.
    pub total_tokens: u32,
    /// Number of tool calls made.
    pub tool_calls: usize,
    /// Duration in milliseconds.
    pub duration_ms: u64,
}

/// Summary of a message in the conversation.
#[derive(Debug, Serialize)]
pub struct MessageSummary {
    /// Role: "user", "assistant", or "system".
    pub role: String,
    /// Text content of the message.
    pub content: String,
}

/// Request to restore a session.
#[derive(Debug, Deserialize)]
pub struct RestoreSessionRequest {
    /// Whether to clear existing history before restoring.
    #[serde(default)]
    pub clear_history: bool,
}

/// Response for session restore.
#[derive(Debug, Serialize)]
pub struct RestoreSessionResponse {
    /// Whether the restore was successful.
    pub success: bool,
    /// Number of messages restored.
    pub message_count: usize,
}
