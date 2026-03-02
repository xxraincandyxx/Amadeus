//! # Amadeus - Agent SDK
//!
//! A Rust SDK for building AI agents with LLM support.
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                     Amadeus SDK                              │
//! │                                                             │
//! │  Agent Loop │ Tool System │ LLM Clients │ Streaming         │
//! │                                                             │
//! └─────────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Usage
//!
//! ```rust,ignore
//! use amadeus::{
//!     Agent, Config, Provider,
//!     AnthropicClient, OpenAIClient,
//!     ToolRegistry, BashTool,
//! };
//! use std::sync::Arc;
//!
//! // Load configuration
//! let config = Arc::new(Config::load()?);
//!
//! // Create client
//! let client = match config.provider {
//!     Provider::Anthropic => AnthropicClient::new(
//!         config.api_key.clone(),
//!         config.base_url.clone(),
//!         config.model.clone(),
//!     ).into(),
//!     Provider::OpenAI => OpenAIClient::new(
//!         config.api_key.clone(),
//!         config.base_url.clone(),
//!         config.model.clone(),
//!     ).into(),
//! };
//!
//! // Create agent
//! let agent = Agent::new(client, config);
//!
//! // Run
//! let result = agent.run("Create a hello world program", &history).await?;
//! println!("{}", result.text);
//! ```
//!
//! ## Tools
//!
//! ```rust,ignore
//! use amadeus::{ToolRegistry, BashTool, ReadFileTool, WriteFileTool};
//!
//! let mut registry = ToolRegistry::new();
//! registry.register("bash", BashTool::new());
//! registry.register("read_file", ReadFileTool::new());
//! registry.register("write_file", WriteFileTool::new());
//! ```

/*
 * ============================================================================
 * SDK MODULES
 * ============================================================================
 */

/// Agent loop, configuration, and message types
pub mod agent;

/// LLM client trait and implementations (Anthropic, OpenAI)
pub mod client;

/// Tool system (bash, file operations, registry)
pub mod tools;

/// Project context loading
pub mod context;

/// Hooks system for extensibility
pub mod hooks;

/// Policy/approval system
pub mod policy;

/// Skills system for reusable prompts
pub mod skills;

/// MCP (Model Context Protocol) support
pub mod mcp;

/// Error types
pub mod error;

/// System prompts (configurable)
pub mod prompts;

/*
 * ============================================================================
 * OPTIONAL MODULES (for testing/examples)
 * ============================================================================
 */

/// HTTP API server (for testing SDK via HTTP)
#[cfg(feature = "api")]
pub mod api;

/// Terminal UI (for testing SDK performance)
#[cfg(feature = "tui")]
pub mod ui;

/// Core primitives (IDs, events)
pub mod core;

/// Concurrency primitives (locks, coordination)
#[cfg(feature = "concurrency")]
pub mod concurrency;

/*
 * ============================================================================
 * RE-EXPORTS
 * ============================================================================
 */

pub use error::{AgentError, Result};

#[cfg(feature = "concurrency")]
pub use concurrency::{LockEntry, LockError, LockManager, LockMode, LockStatus};

#[cfg(feature = "supervisor")]
pub use agent::{
    DispatchStrategy, Supervisor, SupervisorConfig, Task, TaskResult, WorkerConfig, WorkerInfo,
    WorkerStatus,
};
