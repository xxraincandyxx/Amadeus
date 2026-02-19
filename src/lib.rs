//! # Claude Agent Library
//!
//! A Rust-based AI coding agent SDK supporting Anthropic and OpenAI APIs.
//!
//! ## Architecture
//!
//! The library is organized into several modules:
//!
//! - **`core`**: Core primitives (Workspace, Branch, Commit, Event, State)
//! - **`agent`**: Agent system with collaboration patterns
//! - **`client`**: LLM client trait and provider implementations
//! - **`tools`**: Tool implementations (bash, file operations)
//! - **`concurrency`**: Lock and transaction management
//! - **`storage`**: File-based persistence
//! - **`api`**: HTTP server and SDK API
//! - **`ui`**: Terminal UI components
//!
//! ## V2 API Usage
//!
//! ```rust,ignore
//! use claude_agent::core::{Workspace, AgentId};
//! use claude_agent::agent::{Agent, AgentConfig};
//! use claude_agent::client::AnthropicClient;
//! use std::sync::Arc;
//! use tokio::sync::RwLock;
//!
//! // Create workspace
//! let workspace = Arc::new(RwLock::new(Workspace::create("./project").await?));
//!
//! // Create agent with config
//! let config = AgentConfig::from_env()?;
//! let client = AnthropicClient::new(config.api_key.clone(), None, config.model.clone());
//! let mut agent = Agent::new(client, config, workspace.clone());
//!
//! // Run agent
//! let result = agent.run("analyze this codebase").await?;
//! ```
//!
//! ## Collaboration Patterns
//!
//! ```rust,ignore
//! use claude_agent::agent::{Supervisor, Pipeline, Mesh, Race};
//!
//! // Supervisor-Worker
//! let mut supervisor = Supervisor::new(workspace.clone(), client, config);
//! supervisor.spawn_workers().await?;
//! let result = supervisor.dispatch(Task::new("task-1", "prompt")).await?;
//!
//! // Pipeline
//! let pipeline = Pipeline::new(workspace.clone(), client)
//!     .stage(StageConfig::new("parse", parser_config))
//!     .stage(StageConfig::new("analyze", analyzer_config));
//! let result = pipeline.run(input).await?;
//!
//! // Race
//! let race = Race::new(workspace.clone(), client)
//!     .add(config_a)
//!     .add(config_b)
//!     .stop_on(StopCondition::FirstSuccess);
//! let result = race.run("solve this problem").await?;
//! ```
//!
//! ## Legacy API
//!
//! The V1 API is still available in the `agent::legacy` module:
//!
//! ```rust,ignore
//! use claude_agent::agent::legacy::Agent;
//! use claude_agent::agent::config::Config;
//!
//! let config = Arc::new(Config::load()?);
//! let client = AnthropicClient::new(config.api_key.clone(), None, config.model.clone());
//! let agent = Agent::new(client, Arc::clone(&config));
//! let result = agent.run("prompt", history).await?;
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

// Declare the core module - contains Workspace, Branch, Commit, Event, State
// This looks for src/core/mod.rs (it's a directory module)
pub mod core;

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

// Declare the storage module - contains file persistence
// This looks for src/storage/mod.rs
pub mod storage;

// Declare the concurrency module - contains locks and transactions
// This looks for src/concurrency/mod.rs
pub mod concurrency;

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
