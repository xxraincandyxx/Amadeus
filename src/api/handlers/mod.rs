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

/*
 * ============================================================================
 * RE-EXPORTS
 * ============================================================================
 */

// Re-export handlers for convenient access
//
// Users can import handlers directly:
//   use crate::api::handlers::{chat, execute, health, stream};
//
// Or access via the module:
//   use crate::api::handlers::chat::chat;
pub use chat::chat;
pub use execute::execute;
pub use health::health;
pub use stream::stream;
