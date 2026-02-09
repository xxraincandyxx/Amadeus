use reqwest::{Client, StatusCode};
use serde_json::Value;
use crate::agent::messages::{Message, ContentBlock};
use crate::error::{Result, AgentError};

const API_VERSION: &str = "2023-06-01";

pub struct AnthropicClient {
    client: Client,
    api_key: String,
    base_url: String,
    model: String,
}

impl AnthropicClient {
    pub fn new(api_key: String, base_url: Option<String>, model: String) -> Self {
        let base_url = base_url.unwrap_or_else(|| "https://api.anthropic.com".to_string());
        Self {
            client: Client::new(),
            api_key,
            base_url,
            model,
        }
    }

    pub async fn create_message(
        &self,
        system: &str,
        messages: &[Message],
        tools: &[Value],
        max_tokens: u32,
    ) -> Result<(String, Vec<ContentBlock>)> {
        let url = format!("{}/v1/messages", self.base_url);

        let body = serde_json::json!({
            "model": self.model,
            "max_tokens": max_tokens,
            "system": system,
            "messages": messages,
            "tools": tools,
        });

        let response = self
            .client
            .post(&url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", API_VERSION)
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await?;

        if response.status() != StatusCode::OK {
            let status_code = response.status().as_u16();
            let error_text = response.text().await?;
            return Err(AgentError::InvalidResponse(format!("API error {}: {}", status_code, error_text)));
        }

        let json: Value = response.json().await?;
        let stop_reason = json["stop_reason"]
            .as_str()
            .unwrap_or("")
            .to_string();
        let content: Vec<ContentBlock> = serde_json::from_value(json["content"].clone())?;

        Ok((stop_reason, content))
    }
}
