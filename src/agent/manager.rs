// @amadeus-header
// summary: Agent subsystem code for manager.
// layer: agent
// status: active
// feature_flags: none
// provides:
// - module: crate::agent::manager
// - type: crate::agent::manager::AgentStatus
// - type: crate::agent::manager::AgentInfo
// - type: crate::agent::manager::AgentManager
// uses:
// - module: crate::agent::config::Config
// - module: crate::agent::loop_agent::Agent
// - module: crate::agent::profile::AgentProfile
// - module: crate::client::LLMClient
// - module: crate::core::id::AgentId
// - module: crate::error
// - protocol: serde serialization
// invariants:
// - Listed interfaces stay aligned with the implementation in this file.
// side_effects: none
// tests:
// - tests/agent_integration_test.rs
// @end-amadeus-header

//! Agent Manager - handles multiple agents and coordination between them.

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::agent::config::Config;
use crate::agent::loop_agent::Agent;
use crate::agent::profile::AgentProfile;
use crate::client::LLMClient;
use crate::core::id::AgentId;
use crate::error::{AgentError, Result};

/// Status of an agent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentStatus {
    /// Agent is idle, waiting for input
    Idle,
    /// Agent is currently processing a request
    Running,
    /// Agent has an error
    Error,
}

/// Information about an agent (returned to API/UI).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentInfo {
    /// Unique agent identifier.
    pub id: AgentId,
    /// User-defined name for the agent.
    pub name: String,
    /// Agent profile/type.
    pub profile: AgentProfile,
    /// Current status.
    pub status: AgentStatus,
    /// Number of tasks completed.
    pub task_count: usize,
}

/// Manages multiple agents and coordinates between them.
pub struct AgentManager<C: LLMClient> {
    /// The LLM client shared by all agents.
    client: C,
    /// Configuration for agents.
    config: Arc<Config>,
    /// Active agents.
    agents: Vec<AgentHandle<C>>,
    /// Currently active agent index.
    active_index: usize,
    /// Counter for agent names.
    name_counter: usize,
}

struct AgentHandle<C: LLMClient> {
    /// The agent instance.
    agent: Agent<C>,
    /// User-defined name.
    name: String,
    /// Agent profile.
    profile: AgentProfile,
    /// Current status.
    status: AgentStatus,
    /// Number of completed tasks.
    task_count: usize,
}

impl<C: LLMClient + Clone + 'static> AgentManager<C> {
    /// Create a new agent manager.
    pub fn new(client: C, config: Arc<Config>) -> Self {
        Self {
            client,
            config,
            agents: Vec::new(),
            active_index: 0,
            name_counter: 0,
        }
    }

    /// Create a new agent with the given profile.
    /// Returns the AgentId of the newly created agent.
    pub async fn create_agent(
        &mut self,
        name: Option<String>,
        profile: AgentProfile,
    ) -> Result<AgentId> {
        // Generate name if not provided
        let name = name.unwrap_or_else(|| {
            self.name_counter += 1;
            format!("{}-{}", profile.display_name(), self.name_counter)
        });

        // Determine if we need to enable call_peer
        let enable_call_peer = !self.agents.is_empty(); // Enable when 2+ agents

        // Create the agent with appropriate system prompt
        let agent = Agent::builder(self.client.clone(), Arc::clone(&self.config)).build();

        let id = AgentId::new();

        // Add to agents list
        self.agents.push(AgentHandle {
            agent,
            name,
            profile: profile.clone(),
            status: AgentStatus::Idle,
            task_count: 0,
        });

        // If this is the first agent, set as active
        if self.agents.len() == 1 {
            self.active_index = 0;
        }

        // Update call_peer for all agents if needed
        if enable_call_peer {
            self.update_peer_tools().await;
        }

        Ok(id)
    }

    /// List all active agents.
    pub fn list_agents(&self) -> Vec<AgentInfo> {
        self.agents
            .iter()
            .enumerate()
            .map(|(i, handle)| {
                // Generate a temporary ID based on index for now
                let id = AgentId::new();
                AgentInfo {
                    id,
                    name: handle.name.clone(),
                    profile: handle.profile.clone(),
                    status: if i == self.active_index {
                        AgentStatus::Running // Active agent is "running" from user perspective
                    } else {
                        handle.status
                    },
                    task_count: handle.task_count,
                }
            })
            .collect()
    }

    /// Get info for a specific agent.
    pub fn get_agent(&self, _agent_id: &AgentId) -> Option<AgentInfo> {
        // For now, just return the first agent or find by index
        // TODO: Implement proper ID-based lookup
        self.agents.first().map(|handle| {
            let id = AgentId::new();
            AgentInfo {
                id,
                name: handle.name.clone(),
                profile: handle.profile.clone(),
                status: handle.status,
                task_count: handle.task_count,
            }
        })
    }

    /// Get the currently active agent.
    pub fn active_agent(&self) -> Option<&Agent<C>> {
        self.agents.get(self.active_index).map(|h| &h.agent)
    }

    /// Get the currently active agent ID.
    pub fn active_agent_id(&self) -> Option<AgentId> {
        self.agents.get(self.active_index).map(|_h| AgentId::new())
    }

    /// Switch to a different agent by ID.
    pub fn switch_to(&mut self, _agent_id: &AgentId) -> Result<()> {
        // For now, just switch to first agent
        // TODO: Implement proper ID-based lookup
        if self.agents.is_empty() {
            return Err(AgentError::Command("No agents available".to_string()));
        }
        self.active_index = 0;
        Ok(())
    }

    /// Switch to the next agent.
    pub fn switch_next(&mut self) {
        if !self.agents.is_empty() {
            self.active_index = (self.active_index + 1) % self.agents.len();
        }
    }

    /// Switch to the previous agent.
    pub fn switch_prev(&mut self) {
        if !self.agents.is_empty() {
            self.active_index = if self.active_index == 0 {
                self.agents.len() - 1
            } else {
                self.active_index - 1
            };
        }
    }

    /// Kill (remove) an agent.
    pub fn kill(&mut self, _agent_id: &AgentId) -> Result<()> {
        // Don't allow killing the last agent
        if self.agents.len() == 1 {
            return Err(AgentError::Command(
                "Cannot kill the last agent".to_string(),
            ));
        }

        // Remove the last agent for now
        // TODO: Implement proper ID-based removal
        self.agents.pop();

        // Adjust active index if needed
        if self.active_index >= self.agents.len() {
            self.active_index = 0;
        }

        Ok(())
    }

    /// Get peer information for call_peer tool.
    /// Excludes the specified agent from the list.
    pub fn get_peers(&self, _exclude_agent_id: &AgentId) -> Vec<crate::tools::peer::PeerInfo> {
        // Return empty for now
        // TODO: Implement proper peer info
        Vec::new()
    }

    /// Check if call_peer should be enabled (2+ agents).
    pub fn is_call_peer_enabled(&self) -> bool {
        self.agents.len() >= 2
    }

    /// Update the call_peer tool for all agents.
    async fn update_peer_tools(&self) {
        // TODO: Update agent's tool registry with new peer list
    }

    /// Get the total number of agents.
    pub fn agent_count(&self) -> usize {
        self.agents.len()
    }
}
