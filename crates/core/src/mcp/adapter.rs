// @amadeus-header
// summary: MCP integration code for adapter.
// layer: infra
// status: active
// feature_flags: none
// provides:
// - module: crate::mcp::adapter
// - type: crate::mcp::adapter::McpToolAdapter
// - fn: crate::mcp::adapter::create_mcp_adapters
// uses:
// - module: crate::error::Result
// - module: crate::mcp::client
// - module: crate::tools::tool_trait::Tool
// - runtime: tokio async runtime
// - format: JSON values
// invariants:
// - Listed interfaces stay aligned with the implementation in this file.
// side_effects: none
// tests:
// - cmd: cargo test --features full
// @end-amadeus-header

//! # MCP Tool Adapter
//!
//! Adapts MCP tools to the SDK Tool trait.

use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::error::Result;
use crate::mcp::client::{McpClient, McpToolSchema};
use crate::tools::tool_trait::Tool;

/// Adapter that wraps an MCP tool as an SDK Tool.
pub struct McpToolAdapter {
    /// The MCP client (shared, mutex-protected for async access).
    client: Arc<Mutex<McpClient>>,
    /// The tool schema.
    schema: McpToolSchema,
    /// Cached JSON schema for the Tool trait (leaked for static lifetime).
    cached_schema: &'static Value,
    /// Cached name for static lifetime.
    cached_name: &'static str,
}

impl McpToolAdapter {
    /// Create a new MCP tool adapter.
    pub fn new(client: Arc<Mutex<McpClient>>, schema: McpToolSchema) -> Self {
        // Convert MCP schema to SDK schema format
        let json_schema = serde_json::json!({
            "name": schema.name.clone(),
            "description": schema.description.clone().unwrap_or_default(),
            "parameters": schema.input_schema.clone()
        });

        // Leak the boxed Value for static lifetime
        let cached_schema: &'static Value = Box::leak(Box::new(json_schema));

        // Leak the name for static lifetime
        let cached_name: &'static str = Box::leak(schema.name.clone().into_boxed_str());

        Self {
            client,
            schema,
            cached_schema,
            cached_name,
        }
    }

    /// Get the tool name.
    pub fn tool_name(&self) -> &str {
        &self.schema.name
    }

    /// Get the tool description.
    pub fn tool_description(&self) -> Option<&str> {
        self.schema.description.as_deref()
    }
}

#[async_trait]
impl Tool for McpToolAdapter {
    fn name(&self) -> &'static str {
        self.cached_name
    }

    fn schema(&self) -> &'static Value {
        self.cached_schema
    }

    async fn execute(&self, input: Value) -> Result<String> {
        let mut client = self.client.lock().await;
        client.call_tool(&self.schema.name, input).await
    }
}

/// Helper to create MCP tool adapters from a client.
pub async fn create_mcp_adapters(
    config: &crate::mcp::McpServerConfig,
) -> Result<Vec<McpToolAdapter>> {
    let mut client = McpClient::connect(config).await?;
    let tools = client.list_tools().await?;
    let client_arc = Arc::new(Mutex::new(client));

    Ok(tools
        .into_iter()
        .map(|schema| McpToolAdapter::new(Arc::clone(&client_arc), schema))
        .collect())
}
