// @amadeus-header
// summary: Module root for the handlers subsystem and its exports.
// layer: api
// status: active
// feature_flags:
// - api
// provides:
// - module: crate::api::handlers
// - fn: crate::api::handlers::sse_event
// uses:
// - protocol: axum server-sent events
// - protocol: serde serialization
// invariants:
// - Module exports stay aligned with child modules and re-exports.
// - SSE construction never panics; serialization failures degrade to an `error` event.
// side_effects: none
// tests:
// - cmd: cargo test -p api --all-features
// @end-amadeus-header

//! # HTTP Request Handlers
//!
//! Handlers for the REST API endpoints. Each handler processes
//! a specific type of HTTP request and returns the appropriate response.
//!
//! ## Handler Architecture
//!
//! ```text
//! HTTP Request
//!      |
//!      v
//! +-----------------+
//! |   Handler       |  (axum extractor + processing)
//! |                 |
//! | - Parse request |
//! | - Execute logic |
//! | - Build response|
//! +--------+--------+
//!          |
//!          v
//! HTTP Response (JSON)
//! ```
//!
//! ## Available Handlers
//!
//! | Handler | Endpoint | Purpose |
//! |---------|----------|---------|
//! | `health` | GET `/health` | Health check |
//! | `chat` | POST `/chat` | Send message to agent |
//! | `execute` | POST `/execute` | Run bash command |
//! | `external_sessions` | `/v1/sessions/*` | Stable external session protocol |
//! | `tasks` | POST `/tasks` | Multi-agent task execution |
//! | `list_sessions` | GET `/sessions` | List saved sessions |
//! | `get_session` | GET `/sessions/{id}` | Get session details |
//! | `restore_session` | POST `/sessions/{id}/restore` | Restore a session |
//! | `get_config` | GET `/config` | Get current config |
//! | `update_config` | PATCH `/config` | Update config settings |
//! | `get_history` | GET `/history` | Get conversation history |
//! | `list_skills` | GET `/skills` | List available skills |
//! | `stream` | Unregistered | Historical stateless SSE handler |
//! | `submit_approval` | Unregistered | Historical global approval handler |
//!
//! ## Error Handling
//!
//! All handlers return `Result<Json<T>, Json<ErrorResponse>>`.
//! Errors are converted to JSON error responses with:
//! - `error`: Error type name
//! - `message`: Human-readable description

use axum::response::sse::Event;
use serde::Serialize;

/// Builds a named SSE event from a serializable payload.
///
/// Serialization of the API event payloads is expected to succeed, but a failure
/// must not abort the stream: SSE mapping runs inside the response task, so a
/// panic there drops the client connection mid-turn. On failure this degrades to
/// the protocol's own `error` event, which every client already handles.
pub(crate) fn sse_event(name: &str, payload: impl Serialize) -> Event {
    match Event::default().event(name).json_data(payload) {
        Ok(event) => event,
        Err(error) => {
            let fallback = serde_json::json!({
                "message": format!("failed to serialize `{name}` event: {error}"),
            });
            Event::default().event("error").data(fallback.to_string())
        }
    }
}

/*
 * ============================================================================
 * MODULE DECLARATIONS
 * ============================================================================
 */

/// Health check handler.
///
/// Simple GET endpoint to verify server is running.
pub mod health;

/// Chat handler.
///
/// POST endpoint for sending messages to the agent.
pub mod chat;

/// Execute handler.
///
/// POST endpoint for direct bash command execution.
pub mod execute;

/// Stream handler.
///
/// GET endpoint for SSE streaming responses.
pub mod stream;

/// Tasks handler for orchestra task dispatch.
pub mod tasks;

/// Sessions handler for session management.
pub mod sessions;

/// Config handler for configuration management.
pub mod config;

/// History handler for conversation history.
pub mod history;

/// Skills handler for listing available skills.
pub mod skills;

/// Approvals handler for tool approval flow.
pub mod approvals;

/// Agents handler for multi-agent management.
pub mod agents;
pub mod external_sessions;
pub mod summarize;

/// Compaction configuration handler.
pub mod compaction;

/// System prompt inspection and custom building.
pub mod prompts;

/// Memory provider and entry inspection.
pub mod memory;

/// Tool catalog handler.
pub mod tools_catalog;

/// RAG document ingest, query, list, and delete.
pub mod rag;

/*
 * ============================================================================
 * RE-EXPORTS
 * ============================================================================
 */

// Re-export handlers for convenient access
pub use agents::{
    agent_chat, agent_stream, create_agent, get_agent, kill_agent, list_agents, switch_agent,
};
pub use approvals::{list_pending_approvals, register_approval_channel, submit_approval};
pub use chat::chat;
pub use compaction::{get_compaction_config, get_compaction_triggers, update_compaction_config};
pub use config::{get_config, update_config};
pub use execute::execute;
pub use external_sessions::{
    cancel_external_session, close_external_session, compact_external_session,
    create_external_session, external_session_checkpoint, external_session_events,
    external_session_history, get_external_session, list_external_session_approvals,
    list_external_sessions, restore_external_session_checkpoint, submit_external_approval,
    submit_external_message,
};
pub use health::health;
pub use history::get_history;
pub use memory::{delete_entry, list_memory_providers, load_memory_entries, store_entry};
pub use prompts::{build_prompt, list_prompt_sections};
pub use rag::{rag_delete_document, rag_ingest, rag_list_documents, rag_query};
pub use sessions::{get_session, list_sessions, restore_session};
pub use skills::list_skills;
pub use stream::stream;
pub use summarize::summarize;
pub use tasks::handle_task;
pub use tools_catalog::get_tool_catalog;

#[cfg(test)]
mod tests {
    use super::sse_event;
    use serde::{Serialize, Serializer};

    struct Unserializable;

    impl Serialize for Unserializable {
        fn serialize<S: Serializer>(&self, _serializer: S) -> Result<S::Ok, S::Error> {
            Err(serde::ser::Error::custom("payload cannot be serialized"))
        }
    }

    fn rendered(event: axum::response::sse::Event) -> String {
        format!("{event:?}")
    }

    #[test]
    fn serializable_payload_keeps_name_and_data() {
        let output = rendered(sse_event("text", serde_json::json!({ "content": "hi" })));
        assert!(output.contains("text"), "{output}");
        assert!(output.contains("content"), "{output}");
    }

    #[test]
    fn failed_serialization_degrades_to_error_event() {
        let output = rendered(sse_event("token_usage", Unserializable));
        assert!(output.contains("event: error"), "{output}");
        assert!(output.contains("failed to serialize"), "{output}");
    }
}
