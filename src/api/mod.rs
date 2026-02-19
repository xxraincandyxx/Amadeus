//! # Public API Module
//!
//! This module provides the public SDK API for the claude-agent library.
//! It re-exports all public types for convenient access and provides
//! HTTP server functionality for REST API access.
//!
//! ## Architecture
//!
//! ```text
//! ┌───────────────────────────────────────────────────────────────┐
//! │                        Public API                             │
//! ├───────────────────────────────────────────────────────────────┤
//! │  api::prelude  │  api::types  │  api::http  │  api::handlers  │
//! └────────┬───────────────┬─────────────┬─────────────┬──────────┘
//!          │               │             │             │
//!          ▼               ▼             ▼             ▼
//! ┌─────────────┐   ┌───────────┐   ┌─────────┐   ┌──────────┐
//! │   Agent     │   │  Config   │   │ Client  │   │  Tools   │
//! │  (loop)     │   │ Provider  │   │ LLMCl.  │   │ BashTool │
//! └─────────────┘   └───────────┘   └─────────┘   └──────────┘
//! ```
//!
//! ## SDK Usage
//!
//! ### Using the Prelude
//!
//! The prelude exports all commonly used types:
//!
//! ```rust,ignore
//! use claude_agent::api::prelude::*;
//!
//! // Load configuration from environment
//! let config = Config::load()?;
//!
//! // Create a client based on provider
//! let client = match config.provider {
//!     Provider::Anthropic => AnthropicClient::new(
//!         config.api_key,
//!         config.base_url,
//!         config.model,
//!     ).into(),
//!     Provider::OpenAI => OpenAIClient::new(
//!         config.api_key,
//!         config.base_url,
//!         config.model,
//!     ).into(),
//! };
//!
//! // Create and run agent
//! let agent = Agent::new(client, ".".to_string(), 300, false);
//! let history = Arc::new(RwLock::new(Vec::new()));
//! let result = agent.run("your prompt", history).await?;
//! ```
//!
//! ### Using Specific Imports
//!
//! ```rust,ignore
//! use claude_agent::api::{Agent, Config, AnthropicClient};
//! ```
//!
//! ## HTTP Server Usage
//!
//! Start the HTTP server for REST API access:
//!
//! ```bash
//! # Start server on default port (3000)
//! cargo run -- --server
//!
//! # Start server on custom port
//! cargo run -- --server 8080
//! ```
//!
//! ### Endpoints
//!
//! | Endpoint | Method | Description |
//! |----------|--------|-------------|
//! | `/health` | GET | Health check |
//! | `/chat` | POST | Send message to agent |
//! | `/execute` | POST | Execute bash command |
//! | `/stream` | GET | SSE streaming chat |
//!
//! ## Example HTTP Requests
//!
//! ### Chat Endpoint
//!
//! ```bash
//! curl -X POST http://localhost:3000/chat \
//!   -H "Content-Type: application/json" \
//!   -d '{"message": "list files in src/", "timeout_secs": 60}'
//! ```
//!
//! ### Execute Endpoint
//!
//! ```bash
//! curl -X POST http://localhost:3000/execute \
//!   -H "Content-Type: application/json" \
//!   -d '{"command": "ls -la"}'
//! ```

/*
 * ============================================================================
 * MODULE DECLARATIONS
 * ============================================================================
 */

/// Prelude module for convenient imports.
///
/// Contains all commonly used types for SDK usage.
/// Import with: `use claude_agent::api::prelude::*;`
pub mod prelude;

/// HTTP request and response types.
///
/// Defines JSON structures for the REST API endpoints.
pub mod types;

/// HTTP server setup and configuration.
///
/// Provides the `run_server` function and router creation.
pub mod http;

/// Request handlers for HTTP endpoints.
///
/// Each handler processes a specific endpoint type.
pub mod handlers;

/*
 * ============================================================================
 * RE-EXPORTS FROM OTHER MODULES
 * ============================================================================
 *
 * Re-export all public types from the crate for convenient access.
 * Users can import directly from api:: instead of navigating the full path.
 */

// -------------------------------------------------------------------------
// ERROR TYPES
// -------------------------------------------------------------------------

/// Custom error type for agent operations.
///
/// Re-exported from `crate::error`.
pub use crate::error::AgentError;

/// Result type alias using AgentError.
///
/// Re-exported from `crate::error`.
pub use crate::error::Result;

// -------------------------------------------------------------------------
// AGENT TYPES
// -------------------------------------------------------------------------

/// The main agent that orchestrates LLM interaction.
///
/// Generic over the LLM client type.
/// Re-exported from `crate::agent::loop_agent`.
pub use crate::agent::loop_agent::Agent;

/// Configuration loaded from environment variables.
///
/// Re-exported from `crate::agent::config`.
pub use crate::agent::config::Config;

/// LLM provider enum (Anthropic or OpenAI).
///
/// Re-exported from `crate::agent::config`.
pub use crate::agent::config::Provider;

/// A message in the conversation.
///
/// Re-exported from `crate::agent::messages`.
pub use crate::agent::messages::Message;

/// A content block within a message.
///
/// Can be Text, ToolUse, or ToolResult.
/// Re-exported from `crate::agent::messages`.
pub use crate::agent::messages::ContentBlock;

/// Events emitted during agent execution.
///
/// Re-exported from `crate::agent::events`.
pub use crate::agent::events::AgentEvent;

// -------------------------------------------------------------------------
// CLIENT TYPES
// -------------------------------------------------------------------------

/// Trait for LLM API clients.
///
/// Implemented by AnthropicClient and OpenAIClient.
/// Re-exported from `crate::client`.
pub use crate::client::LLMClient;

/// Events emitted during streaming responses.
///
/// Re-exported from `crate::client`.
pub use crate::client::StreamEvent;

/// Client for the Anthropic Messages API.
///
/// Re-exported from `crate::client::anthropic`.
pub use crate::client::AnthropicClient;

/// Client for the OpenAI Chat Completions API.
///
/// Re-exported from `crate::client::openai`.
pub use crate::client::OpenAIClient;

// -------------------------------------------------------------------------
// TOOL TYPES
// -------------------------------------------------------------------------

/// Tool for executing bash commands.
///
/// Re-exported from `crate::tools::bash`.
pub use crate::tools::bash::BashTool;

/// Get the bash tool JSON schema.
///
/// Returns a reference to the lazily-initialized schema.
/// Re-exported from `crate::tools::schema`.
pub use crate::tools::schema::bash_tool;
