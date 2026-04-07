// @amadeus-header
// summary: Tool implementation and support code for web.
// layer: tools
// status: active
// feature_flags: none
// provides:
// - module: crate::tools::web
// - type: crate::tools::web::WebFetchInput
// - type: crate::tools::web::WebFetchTool
// - tool: web_fetch
// uses:
// - module: crate::error
// - module: crate::tools::schema::web_fetch_tool
// - module: crate::tools::tool_trait::Tool
// - runtime: tokio async runtime
// - protocol: reqwest HTTP client
// - protocol: serde serialization
// - format: JSON values
// invariants:
// - Declared tool interfaces stay aligned with runtime behavior and schema.
// side_effects:
// - Performs network or HTTP operations.
// tests:
// - tests/tool_approval_test.rs
// @end-amadeus-header

//! # Web Fetch Tool
//!
//! Fetch and convert URL content to LLM-friendly input.
//!
//! ## Features
//!
//! - HTTP/HTTPS URL fetching
//! - Configurable timeout
//! - Response size limiting
//! - Content truncation
//! - Error handling for various HTTP scenarios

use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;
use serde_json::Value;
use std::time::Duration;
use tokio::time::timeout;

use crate::error::{AgentError, Result};
use crate::tools::schema::web_fetch_tool;
use crate::tools::tool_trait::Tool;

#[derive(Debug, Clone, Deserialize)]
pub struct WebFetchInput {
    pub url: String,
    #[serde(default)]
    pub format: Option<String>,
    #[serde(default = "default_timeout")]
    pub timeout_secs: Option<u64>,
    #[serde(default = "default_max_bytes")]
    pub max_bytes: Option<usize>,
}

fn default_timeout() -> Option<u64> {
    Some(20)
}

fn default_max_bytes() -> Option<usize> {
    Some(50000)
}

pub struct WebFetchTool {
    client: Client,
    default_timeout_secs: u64,
    default_max_bytes: usize,
}

impl WebFetchTool {
    pub fn new(timeout_secs: u64, max_bytes: usize) -> Self {
        let client = Client::builder()
            .user_agent("Amadeus/0.1.0")
            .redirect(reqwest::redirect::Policy::limited(5))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            default_timeout_secs: timeout_secs,
            default_max_bytes: max_bytes,
        }
    }

    pub fn from_config(config: &crate::agent::config::Config) -> Self {
        Self::new(20, config.max_output_bytes)
    }

    fn validate_url(&self, url: &str) -> Result<()> {
        let parsed = url::Url::parse(url).map_err(|e| AgentError::ToolInput {
            tool: "web_fetch".to_string(),
            reason: format!("Invalid URL: {}", e),
        })?;

        match parsed.scheme() {
            "http" | "https" => Ok(()),
            scheme => Err(AgentError::ToolInput {
                tool: "web_fetch".to_string(),
                reason: format!(
                    "Unsupported URL scheme: {}. Only http and https are allowed.",
                    scheme
                ),
            }),
        }
    }
}

#[async_trait]
impl Tool for WebFetchTool {
    fn name(&self) -> &'static str {
        "web_fetch"
    }

    fn schema(&self) -> &'static Value {
        web_fetch_tool()
    }

    async fn execute(&self, input: Value) -> Result<String> {
        let parsed: WebFetchInput =
            serde_json::from_value(input).map_err(|e| AgentError::ToolInput {
                tool: "web_fetch".to_string(),
                reason: e.to_string(),
            })?;

        self.validate_url(&parsed.url)?;

        let timeout_secs = parsed.timeout_secs.unwrap_or(self.default_timeout_secs);
        let max_bytes = parsed.max_bytes.unwrap_or(self.default_max_bytes);

        let request_timeout = Duration::from_secs(timeout_secs);

        let response = timeout(request_timeout, async {
            self.client
                .get(&parsed.url)
                .header("Accept", "text/html,application/xhtml+xml,application/xml;q=0.9,text/plain;q=0.8,*/*;q=0.7")
                .send()
                .await
        })
        .await
        .map_err(|_| AgentError::Timeout(timeout_secs))?
        .map_err(|e| AgentError::Api(format!("HTTP request failed: {}", e)))?;

        let status = response.status();
        if !status.is_success() {
            return Err(AgentError::Api(format!(
                "HTTP error: {} {}",
                status.as_u16(),
                status.canonical_reason().unwrap_or("Unknown")
            )));
        }

        // Get content type
        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("text/plain");

        // Check if content is text-based
        if !content_type.starts_with("text/")
            && !content_type.contains("application/json")
            && !content_type.contains("application/xml")
            && !content_type.contains("application/javascript")
        {
            return Err(AgentError::ToolInput {
                tool: "web_fetch".to_string(),
                reason: format!(
                    "Unsupported content type: {}. Only text-based content is supported.",
                    content_type
                ),
            });
        }

        let body = response
            .text()
            .await
            .map_err(|e| AgentError::Api(format!("Failed to read response body: {}", e)))?;

        // Truncate if needed
        let result = if body.len() > max_bytes {
            let truncated = &body[..max_bytes];
            format!(
                "Content from: {}\n\n{}\n\n... (truncated {} bytes)",
                parsed.url,
                truncated,
                body.len() - max_bytes
            )
        } else {
            format!("Content from: {}\n\n{}", parsed.url, body)
        };

        Ok(result)
    }
}
