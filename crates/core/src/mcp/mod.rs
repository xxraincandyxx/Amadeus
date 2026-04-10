// @amadeus-header
// summary: Module root for the mcp subsystem and its exports.
// layer: infra
// status: active
// feature_flags: none
// provides:
// - module: crate::mcp
// uses: none
// invariants:
// - Module exports stay aligned with child modules and re-exports.
// side_effects: none
// tests:
// - tests/mod.rs
// @end-amadeus-header

//! # MCP (Model Context Protocol) Support
//!
//! Connect to MCP servers and use their tools.
//!
//! ## Overview
//!
//! MCP is a protocol that allows agents to discover and use tools from
//! external servers. Each MCP server provides:
//! - Tools that can be executed
//! - Resources that can be read
//! - Prompts that can be used
//!
//! ## Usage
//!
//! ```rust,ignore
//! use amadeus::mcp::{McpClient, McpServerConfig};
//!
//! let config = McpServerConfig {
//!     command: "npx".to_string(),
//!     args: vec!["-y".to_string(), "@modelcontextprotocol/server-filesystem".to_string()],
//!     env: HashMap::new(),
//! };
//!
//! let client = McpClient::connect(&config).await?;
//! let tools = client.list_tools().await?;
//!
//! // Use McpToolAdapter to integrate with Agent
//! ```

pub mod adapter;
pub mod client;

pub use adapter::{create_mcp_tool_pack, McpToolAdapter};
pub use client::{McpClient, McpServerConfig, McpToolSchema};
