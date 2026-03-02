//! # MCP Client
//!
//! Client for connecting to MCP servers.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};

use crate::error::{AgentError, Result};

/// Configuration for an MCP server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerConfig {
    /// The command to run the MCP server.
    pub command: String,
    /// Arguments to pass to the command.
    #[serde(default)]
    pub args: Vec<String>,
    /// Environment variables to set.
    #[serde(default)]
    pub env: HashMap<String, String>,
}

/// Schema for an MCP tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolSchema {
    /// Tool name.
    pub name: String,
    /// Tool description.
    pub description: Option<String>,
    /// JSON schema for input parameters.
    pub input_schema: Value,
}

/// Client for communicating with an MCP server.
pub struct McpClient {
    /// The child process.
    process: Child,
    /// Stdin for sending requests.
    stdin: ChildStdin,
    /// Reader for stdout.
    stdout_reader: BufReader<ChildStdout>,
    /// Request ID counter.
    request_id: u64,
}

impl McpClient {
    /// Connect to an MCP server.
    pub async fn connect(config: &McpServerConfig) -> Result<Self> {
        let mut cmd = Command::new(&config.command);
        cmd.args(&config.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());

        for (key, value) in &config.env {
            cmd.env(key, value);
        }

        let mut process = cmd
            .spawn()
            .map_err(|e| AgentError::Command(format!("Failed to start MCP server: {}", e)))?;

        let stdin = process
            .stdin
            .take()
            .ok_or_else(|| AgentError::Command("Failed to get stdin".to_string()))?;

        let stdout = process
            .stdout
            .take()
            .ok_or_else(|| AgentError::Command("Failed to get stdout".to_string()))?;

        let stdout_reader = BufReader::new(stdout);

        let mut client = Self {
            process,
            stdin,
            stdout_reader,
            request_id: 0,
        };

        // Initialize the connection
        client.initialize().await?;

        Ok(client)
    }

    /// Send a request and get a response.
    async fn send_request(&mut self, method: &str, params: Value) -> Result<Value> {
        self.request_id += 1;
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": self.request_id,
            "method": method,
            "params": params
        });

        let request_str = serde_json::to_string(&request)
            .map_err(|e| AgentError::Command(format!("Failed to serialize request: {}", e)))?;

        // Send request
        self.stdin
            .write_all(request_str.as_bytes())
            .await
            .map_err(|e| AgentError::Command(format!("Failed to write to MCP server: {}", e)))?;
        self.stdin
            .write_all(b"\n")
            .await
            .map_err(|e| AgentError::Command(format!("Failed to write newline: {}", e)))?;

        // Read response
        let mut response_line = String::new();
        self.stdout_reader
            .read_line(&mut response_line)
            .await
            .map_err(|e| AgentError::Command(format!("Failed to read from MCP server: {}", e)))?;

        let response: Value = serde_json::from_str(&response_line)
            .map_err(|e| AgentError::Command(format!("Invalid JSON response: {}", e)))?;

        // Check for error
        if let Some(error) = response.get("error") {
            return Err(AgentError::Command(format!(
                "MCP error: {}",
                error
            )));
        }

        Ok(response
            .get("result")
            .cloned()
            .unwrap_or(Value::Null))
    }

    /// Initialize the MCP connection.
    async fn initialize(&mut self) -> Result<()> {
        let _result = self
            .send_request(
                "initialize",
                serde_json::json!({
                    "protocolVersion": "2024-11-05",
                    "capabilities": {},
                    "clientInfo": {
                        "name": "amadeus",
                        "version": "0.1.0"
                    }
                }),
            )
            .await?;

        // Send initialized notification
        self.stdin
            .write_all(b"{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\n")
            .await
            .map_err(|e| AgentError::Command(format!("Failed to send initialized: {}", e)))?;

        Ok(())
    }

    /// List available tools from the server.
    pub async fn list_tools(&mut self) -> Result<Vec<McpToolSchema>> {
        let result = self
            .send_request("tools/list", serde_json::json!({}))
            .await?;

        let tools = result
            .get("tools")
            .and_then(|t| t.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|t| {
                        Some(McpToolSchema {
                            name: t.get("name")?.as_str()?.to_string(),
                            description: t.get("description").and_then(|d| d.as_str()).map(String::from),
                            input_schema: t.get("inputSchema").cloned().unwrap_or(Value::Null),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        Ok(tools)
    }

    /// Call a tool on the server.
    pub async fn call_tool(&mut self, name: &str, args: Value) -> Result<String> {
        let result = self
            .send_request(
                "tools/call",
                serde_json::json!({
                    "name": name,
                    "arguments": args
                }),
            )
            .await?;

        // Extract content from result
        if let Some(content) = result.get("content").and_then(|c| c.as_array()) {
            let text_parts: Vec<&str> = content
                .iter()
                .filter_map(|item| {
                    if item.get("type")?.as_str()? == "text" {
                        item.get("text")?.as_str()
                    } else {
                        None
                    }
                })
                .collect();
            Ok(text_parts.join("\n"))
        } else if result.get("isError").and_then(|e| e.as_bool()).unwrap_or(false) {
            Err(AgentError::Command(format!(
                "Tool '{}' returned an error: {:?}",
                name, result
            )))
        } else {
            Ok(result.to_string())
        }
    }

    /// Get the list of tools (cloned for adapter use).
    pub async fn get_tools(&mut self) -> Result<Vec<McpToolSchema>> {
        self.list_tools().await
    }
}

impl Drop for McpClient {
    fn drop(&mut self) {
        let _ = self.process.start_kill();
    }
}
