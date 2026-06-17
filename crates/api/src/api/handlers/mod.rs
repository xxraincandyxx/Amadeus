// @amadeus-header
// summary: Module root for the handlers subsystem and its exports.
// layer: api
// status: active
// feature_flags:
// - api
// provides:
// - module: crate::api::handlers
// uses: none
// invariants:
// - Module exports stay aligned with child modules and re-exports.
// side_effects: none
// tests:
// - tests/mod.rs
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
//! | `stream` | GET `/stream` | SSE streaming chat |
//! | `tasks` | POST `/tasks` | Multi-agent task execution |
//! | `list_sessions` | GET `/sessions` | List saved sessions |
//! | `get_session` | GET `/sessions/{id}` | Get session details |
//! | `restore_session` | POST `/sessions/{id}/restore` | Restore a session |
//! | `get_config` | GET `/config` | Get current config |
//! | `update_config` | PATCH `/config` | Update config settings |
//! | `get_history` | GET `/history` | Get conversation history |
//! | `list_skills` | GET `/skills` | List available skills |
//! | `submit_approval` | POST `/approvals/{id}` | Submit approval decision |
//!
//! ## Error Handling
//!
//! All handlers return `Result<Json<T>, Json<ErrorResponse>>`.
//! Errors are converted to JSON error responses with:
//! - `error`: Error type name
//! - `message`: Human-readable description

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
