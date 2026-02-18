//! # Claude Agent Library
//!
//! A Rust-based AI coding agent implementation supporting both Anthropic and OpenAI APIs.
//!
//! ## Architecture
//!
//! The library is organized into several modules:
//!
//! - **`api`**: Public SDK API with HTTP server support (re-exports all types)
//! - **`error`**: Custom error types and Result alias for the crate
//! - **`agent`**: Agent loop, configuration, and message types
//! - **`client`**: LLM client trait and provider implementations (Anthropic, OpenAI)
//! - **`tools`**: Tool implementations (bash execution, schemas)
//! - **`ui`**: Terminal UI components (colors, REPL)
//!
//! ## SDK Usage
//!
//! ### Using the API Module (Recommended)
//!
//! ```rust,ignore
//! use claude_agent::api::prelude::*;
//!
//! // Load config from environment
//! let config = Config::load()?;
//!
//! // Create an Anthropic client
//! let client = AnthropicClient::new(
//!     config.api_key,
//!     config.base_url,
//!     config.model,
//! );
//!
//! // Create and run the agent
//! let agent = Agent::new(client, "/path/to/workdir".to_string(), 300, false);
//! let result = agent.run("your prompt", history).await?;
//! ```
//!
//! ## HTTP Server Usage
//!
//! ```bash
//! # Start server on default port (3000)
//! cargo run -- --server
//!
//! # Start server on custom port
//! cargo run -- --server 8080
//! ```
//!
//! ## Provider Abstraction
//!
//! The `LLMClient` trait enables swapping between providers:
//!
//! ```rust,ignore
//! // Use Anthropic (default)
//! let client = AnthropicClient::new(api_key, None, "claude-sonnet-4-5-20250929".to_string());
//!
//! // Or use OpenAI
//! let client = OpenAIClient::new(api_key, None, "gpt-4".to_string());
//! ```

/*
 * ============================================================================
 * MODULE DECLARATIONS
 * ============================================================================
 *
 * In Rust, the `pub mod` keyword declares modules that are part of this crate.
 * Each `pub mod x;` looks for either:
 *   - A file named `x.rs` in the same directory, OR
 *   - A directory named `x/` with a file `x/mod.rs` inside
 *
 * The `pub` keyword makes the module visible to external crates that depend
 * on this library. Without `pub`, the module would be private to this crate.
 */

// Declare the api module - contains public SDK API and HTTP server
// This looks for src/api/mod.rs (it's a directory module)
pub mod api;

// Declare the error module - contains custom error types
// This looks for src/error.rs
pub mod error;

// Declare the agent module - contains agent loop, config, messages
// This looks for src/agent/mod.rs (it's a directory module)
pub mod agent;

// Declare the client module - contains LLM client implementations
// This looks for src/client/mod.rs
pub mod client;

// Declare the tools module - contains bash tool, file tools, registry and schemas
// This looks for src/tools/mod.rs
pub mod tools;

// Declare the ui module - contains colors and REPL
// This looks for src/ui/mod.rs
pub mod ui;

/*
 * ============================================================================
 * RE-EXPORTS
 * ============================================================================
 *
 * The `pub use` keyword re-exports items from modules, making them available
 * at the crate root level. This provides a cleaner API for users.
 *
 * Without re-exports, users would need to write:
 *   use claude_agent::error::{AgentError, Result};
 *
 * With re-exports, users can write:
 *   use claude_agent::{AgentError, Result};
 */

// Re-export AgentError and Result from the error module
// This makes them available directly from the crate root
pub use error::{AgentError, Result};
