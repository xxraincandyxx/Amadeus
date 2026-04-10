// @amadeus-header
// summary: MCP integration adapters for legacy tool compatibility and unified tool-pack composition.
// layer: infra
// status: active
// feature_flags: none
// provides:
// - module: crate::mcp::adapter
// - type: crate::mcp::adapter::McpToolAdapter
// - fn: crate::mcp::adapter::create_mcp_adapters
// - fn: crate::mcp::adapter::create_mcp_tool_pack
// uses:
// - module: crate::mcp::client
// - module: crate::tools::platform
// - module: crate::tools::tool_trait
// invariants:
// - MCP tool schemas map into native ToolSpec values before model exposure.
// side_effects: none
// tests:
// - cmd: cargo test -p core mcp_tool_pack --features full
// @end-amadeus-header

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value;
use tokio::sync::Mutex;

use crate::error::Result;
use crate::mcp::client::{McpClient, McpToolSchema};
use crate::permissions::PermissionMode;
use crate::tools::platform::{
    ToolExecutionResult, ToolExecutor, ToolLevel, ToolPack, ToolPolicy, ToolRegistration,
    ToolSource, ToolSpec,
};
use crate::tools::tool_trait::Tool;

/// Adapter that wraps an MCP tool as a legacy SDK Tool.
pub struct McpToolAdapter {
    client: Arc<Mutex<McpClient>>,
    schema: McpToolSchema,
    cached_schema: &'static Value,
    cached_name: &'static str,
}

impl McpToolAdapter {
    pub fn new(client: Arc<Mutex<McpClient>>, schema: McpToolSchema) -> Self {
        let json_schema = serde_json::json!({
            "name": schema.name.clone(),
            "description": schema.description.clone().unwrap_or_default(),
            "parameters": schema.input_schema.clone()
        });
        let cached_schema: &'static Value = Box::leak(Box::new(json_schema));
        let cached_name: &'static str = Box::leak(schema.name.clone().into_boxed_str());

        Self {
            client,
            schema,
            cached_schema,
            cached_name,
        }
    }

    pub fn tool_name(&self) -> &str {
        &self.schema.name
    }

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

struct McpToolExecutor {
    client: Arc<Mutex<McpClient>>,
}

#[async_trait]
impl ToolExecutor for McpToolExecutor {
    async fn execute(&self, canonical_name: &str, input: Value) -> Result<ToolExecutionResult> {
        let mut client = self.client.lock().await;
        let output = client.call_tool(canonical_name, input).await?;
        Ok(ToolExecutionResult {
            output,
            is_error: false,
            metadata: None,
        })
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

/// Create a unified MCP tool pack for the composed catalog.
pub async fn create_mcp_tool_pack(config: &crate::mcp::McpServerConfig) -> Result<ToolPack> {
    let mut client = McpClient::connect(config).await?;
    let tools = client.list_tools().await?;
    let client = Arc::new(Mutex::new(client));

    Ok(ToolPack {
        name: "mcp".to_string(),
        tools: tools
            .into_iter()
            .map(|schema| ToolRegistration {
                spec: mcp_tool_spec(schema),
                policy: ToolPolicy::default(),
                executor: Arc::new(McpToolExecutor {
                    client: Arc::clone(&client),
                }),
            })
            .collect(),
    })
}

fn mcp_tool_spec(schema: McpToolSchema) -> ToolSpec {
    ToolSpec {
        name: schema.name,
        description: schema.description.unwrap_or_default(),
        input_schema: schema.input_schema,
        required_permission: PermissionMode::DangerFullAccess,
        source: ToolSource::Mcp,
        level: ToolLevel::Primitive,
        tags: vec!["mcp".to_string()],
        aliases: Vec::new(),
        pack: "mcp".to_string(),
        prompt_approval: false,
        visible_in_modes: vec![
            PermissionMode::DangerFullAccess,
            PermissionMode::Prompt,
            PermissionMode::Allow,
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mcp_tool_pack_entries_use_native_tool_specs() {
        let spec = mcp_tool_spec(McpToolSchema {
            name: "remote_search".to_string(),
            description: Some("Search remotely".to_string()),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {"query": {"type": "string"}}
            }),
        });

        assert_eq!(spec.source, ToolSource::Mcp);
        assert_eq!(spec.pack, "mcp");
        assert_eq!(spec.required_permission, PermissionMode::DangerFullAccess);
        assert_eq!(spec.description, "Search remotely");
    }
}
