// @amadeus-header
// summary: TUI module code for api client.
// layer: ui
// status: active
// feature_flags:
// - tui
// provides:
// - module: crate::ui::api_client
// - type: crate::ui::api_client::AgentInfo
// - type: crate::ui::api_client::ListAgentsResponse
// - type: crate::ui::api_client::CreateAgentRequest
// - type: crate::ui::api_client::CreateAgentResponse
// - type: crate::ui::api_client::ApiClient
// uses:
// - runtime: tokio async runtime
// - protocol: reqwest HTTP client
// - protocol: serde serialization
// invariants:
// - Listed interfaces stay aligned with the implementation in this file.
// side_effects:
// - Performs network or HTTP operations.
// tests:
// - tests/tui_snapshot_test.rs
// @end-amadeus-header

//! # API Client for TUI
//!
//! HTTP client wrapper for TUI to communicate with the API server.

use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Information about an agent from API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentInfo {
    pub id: String,
    pub name: String,
    pub profile: String,
    pub status: String,
    pub task_count: usize,
}

/// Response from listing agents.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListAgentsResponse {
    pub agents: Vec<AgentInfo>,
    pub active_agent_id: Option<String>,
}

/// Request to create an agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateAgentRequest {
    pub name: Option<String>,
    pub profile: String,
}

/// Response from creating an agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateAgentResponse {
    pub agent: AgentInfo,
}

/// API client for TUI to communicate with the server.
pub struct ApiClient {
    client: Client,
    base_url: String,
    /// Cached agent list (refreshed on demand).
    agents: Arc<RwLock<Vec<AgentInfo>>>,
    /// Currently active agent index.
    active_index: Arc<RwLock<usize>>,
}

impl ApiClient {
    /// Create a new API client.
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            client: Client::new(),
            base_url: base_url.into(),
            agents: Arc::new(RwLock::new(Vec::new())),
            active_index: Arc::new(RwLock::new(0)),
        }
    }

    /// Get the base URL.
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Refresh the agent list from the server.
    pub async fn refresh_agents(&self) -> Result<ListAgentsResponse, String> {
        let response = self
            .client
            .get(format!("{}/agents", self.base_url))
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if !response.status().is_success() {
            return Err(format!("API error: {}", response.status()));
        }

        let data: ListAgentsResponse = response.json().await.map_err(|e| e.to_string())?;

        // Update cached agents
        *self.agents.write().await = data.agents.clone();

        // Update active index
        if let Some(ref active_id) = data.active_agent_id {
            if let Some(index) = data.agents.iter().position(|a| a.id == *active_id) {
                *self.active_index.write().await = index;
            }
        }

        Ok(data)
    }

    /// List all agents (cached).
    pub async fn list_agents(&self) -> Vec<AgentInfo> {
        self.agents.read().await.clone()
    }

    /// Get currently active agent.
    pub async fn active_agent(&self) -> Option<AgentInfo> {
        let index = *self.active_index.read().await;
        let agents = self.agents.read().await;
        agents.get(index).cloned()
    }

    /// Create a new agent.
    pub async fn create_agent(
        &self,
        name: Option<String>,
        profile: &str,
    ) -> Result<AgentInfo, String> {
        let request = CreateAgentRequest {
            name,
            profile: profile.to_string(),
        };

        let response = self
            .client
            .post(format!("{}/agents", self.base_url))
            .json(&request)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if !response.status().is_success() {
            return Err(format!("Failed to create agent: {}", response.status()));
        }

        let data: CreateAgentResponse = response.json().await.map_err(|e| e.to_string())?;

        // Refresh agent list
        self.refresh_agents().await?;

        Ok(data.agent)
    }

    /// Kill an agent.
    pub async fn kill_agent(&self, agent_id: &str) -> Result<(), String> {
        let response = self
            .client
            .delete(format!("{}/agents/{}", self.base_url, agent_id))
            .json(&serde_json::json!({ "reason": "User requested" }))
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if !response.status().is_success() {
            return Err(format!("Failed to kill agent: {}", response.status()));
        }

        // Refresh agent list
        self.refresh_agents().await?;

        Ok(())
    }

    /// Switch to a different agent.
    pub async fn switch_agent(&self, agent_id: &str) -> Result<(), String> {
        let request = serde_json::json!({ "agent_id": agent_id });

        let response = self
            .client
            .post(format!("{}/agents/{}/switch", self.base_url, agent_id))
            .json(&request)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if !response.status().is_success() {
            return Err(format!("Failed to switch agent: {}", response.status()));
        }

        // Update active index
        let agents = self.agents.read().await;
        if let Some(index) = agents.iter().position(|a| a.id == agent_id) {
            *self.active_index.write().await = index;
        }

        Ok(())
    }

    /// Switch to next agent.
    pub async fn switch_next(&self) -> Result<Option<AgentInfo>, String> {
        let mut index = *self.active_index.read().await;
        let agents = self.agents.read().await;

        if agents.is_empty() {
            return Ok(None);
        }

        index = (index + 1) % agents.len();
        let new_agent = agents.get(index).cloned();

        drop(agents);
        *self.active_index.write().await = index;

        if let Some(ref agent) = new_agent {
            self.switch_agent(&agent.id).await?;
        }

        Ok(new_agent)
    }

    /// Switch to previous agent.
    pub async fn switch_prev(&self) -> Result<Option<AgentInfo>, String> {
        let mut index = *self.active_index.read().await;
        let agents = self.agents.read().await;

        if agents.is_empty() {
            return Ok(None);
        }

        index = if index == 0 {
            agents.len() - 1
        } else {
            index - 1
        };
        let new_agent = agents.get(index).cloned();

        drop(agents);
        *self.active_index.write().await = index;

        if let Some(ref agent) = new_agent {
            self.switch_agent(&agent.id).await?;
        }

        Ok(new_agent)
    }

    /// Send a message to the active agent (non-streaming).
    pub async fn chat(&self, message: &str) -> Result<String, String> {
        let agent = self.active_agent().await.ok_or("No active agent")?;

        let request = serde_json::json!({
            "message": message
        });

        let response = self
            .client
            .post(format!("{}/agents/{}/chat", self.base_url, agent.id))
            .json(&request)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if !response.status().is_success() {
            return Err(format!("Chat failed: {}", response.status()));
        }

        #[derive(Deserialize)]
        struct ChatResponse {
            content: String,
        }

        let data: ChatResponse = response.json().await.map_err(|e| e.to_string())?;
        Ok(data.content)
    }

    /// Stream events from the active agent.
    pub async fn stream(&self, message: &str) -> Result<String, String> {
        let agent = self.active_agent().await.ok_or("No active agent")?;

        let url = format!(
            "{}/agents/{}/stream?message={}",
            self.base_url,
            agent.id,
            urlencoding::encode(message)
        );

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| e.to_string())?;
        let body = response.text().await.map_err(|e| e.to_string())?;

        Ok(body)
    }
}
