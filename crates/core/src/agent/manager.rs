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
// - module: crate::agent::team
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
use std::time::Instant;

use serde::{Deserialize, Serialize};

use crate::agent::config::Config;
use crate::agent::loop_agent::Agent;
use crate::agent::profile::AgentProfile;
use crate::agent::team::{AgentTeam, TeamLeader, TeamRegistry};
use crate::agent::worker::{Task, TaskResult, WorkerConfig};
use crate::client::LLMClient;
use crate::core::id::{AgentId, TeamId};
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
    /// Shared team/task coordination registry.
    teams: TeamRegistry,
}

struct AgentHandle<C: LLMClient> {
    /// Stable agent identifier.
    id: AgentId,
    /// The agent instance.
    agent: Agent<C>,
    /// User-defined name.
    name: String,
    /// Agent profile.
    profile: AgentProfile,
    /// Capability tags used for team task routing.
    capabilities: Vec<String>,
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
            teams: TeamRegistry::new(),
        }
    }

    /// Create a new agent with the given profile.
    /// Returns the AgentId of the newly created agent.
    pub async fn create_agent(
        &mut self,
        name: Option<String>,
        profile: AgentProfile,
    ) -> Result<AgentId> {
        self.create_agent_with_capabilities(name, profile, Vec::new(), None)
            .await
    }

    /// Create a new teammate using worker-style configuration.
    pub async fn spawn_teammate(&mut self, config: WorkerConfig) -> Result<AgentId> {
        self.create_agent_with_capabilities(
            Some(config.name),
            AgentProfile::Default,
            config.capabilities,
            config.model,
        )
        .await
    }

    /// Create a new team and return its identifier.
    pub fn create_team(&mut self, name: impl Into<String>, leader: TeamLeader) -> TeamId {
        self.teams.create_team(name, leader)
    }

    /// Ensure there is always a default team for task routing.
    pub fn ensure_default_team(&mut self, leader: TeamLeader) -> TeamId {
        if let Some(team_id) = self.teams.default_team_id() {
            return team_id;
        }
        self.teams.create_team("default", leader)
    }

    /// List all teams.
    pub fn list_teams(&self) -> Vec<AgentTeam> {
        self.teams.list_teams()
    }

    /// Add an agent to a team.
    pub fn add_agent_to_team(&mut self, team_id: TeamId, agent_id: AgentId) -> Result<()> {
        if !self.agents.iter().any(|agent| agent.id == agent_id) {
            return Err(AgentError::Command(format!("Unknown agent: {}", agent_id)));
        }
        self.teams.add_member(team_id, agent_id)
    }

    /// Execute a team task using the best available local agent.
    pub async fn execute_task(
        &mut self,
        team_id: Option<TeamId>,
        task: Task,
    ) -> Result<TaskResult> {
        let target_team_id = team_id.or_else(|| self.teams.default_team_id());
        let selected_index = self.select_agent_index(target_team_id, &task)?;
        let selected_id = self.agents[selected_index].id;
        let agent = self.agents[selected_index].agent.clone();

        if let Some(team_id) = target_team_id {
            self.teams
                .queue_task(team_id, task.clone(), TeamLeader::User)?;
            self.teams.claim_task(team_id, &task.id, selected_id)?;
        }

        self.agents[selected_index].status = AgentStatus::Running;
        self.active_index = selected_index;

        let start = Instant::now();
        let result = agent.run(&task.prompt).await;
        let duration_ms = start.elapsed().as_millis() as u64;
        let handle = &mut self.agents[selected_index];
        handle.status = AgentStatus::Idle;

        let task_result = match result {
            Ok(run_result) => {
                handle.task_count += 1;
                TaskResult {
                    task_id: task.id.clone(),
                    worker_id: selected_id,
                    success: true,
                    output: Some(run_result.text),
                    error: None,
                    duration_ms,
                    tool_calls: run_result.tool_calls,
                }
            }
            Err(error) => {
                handle.status = AgentStatus::Error;
                TaskResult {
                    task_id: task.id.clone(),
                    worker_id: selected_id,
                    success: false,
                    output: None,
                    error: Some(error.to_string()),
                    duration_ms,
                    tool_calls: Vec::new(),
                }
            }
        };

        if let Some(team_id) = target_team_id {
            self.teams
                .record_result(team_id, &task.id, selected_id, &task_result)?;
        }

        if task_result.success {
            Ok(task_result)
        } else {
            Err(AgentError::Command(
                task_result
                    .error
                    .clone()
                    .unwrap_or_else(|| "Task execution failed".to_string()),
            ))
        }
    }

    async fn create_agent_with_capabilities(
        &mut self,
        name: Option<String>,
        profile: AgentProfile,
        capabilities: Vec<String>,
        model_override: Option<String>,
    ) -> Result<AgentId> {
        let name = name.unwrap_or_else(|| {
            self.name_counter += 1;
            format!("{}-{}", profile.display_name(), self.name_counter)
        });

        let id = AgentId::new();
        let agent_config = if let Some(model) = model_override {
            let mut config = (*self.config).clone();
            config.model = model;
            Arc::new(config)
        } else {
            Arc::clone(&self.config)
        };
        let agent = Agent::builder(self.client.clone(), agent_config)
            .with_default_tools()
            .build();

        self.agents.push(AgentHandle {
            id,
            agent,
            name,
            profile: profile.clone(),
            capabilities,
            status: AgentStatus::Idle,
            task_count: 0,
        });

        if self.agents.len() == 1 {
            self.active_index = 0;
        }

        Ok(id)
    }

    /// List all active agents.
    pub fn list_agents(&self) -> Vec<AgentInfo> {
        self.agents
            .iter()
            .enumerate()
            .map(|(i, handle)| {
                AgentInfo {
                    id: handle.id,
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
    pub fn get_agent(&self, agent_id: &AgentId) -> Option<AgentInfo> {
        self.agents
            .iter()
            .find(|handle| &handle.id == agent_id)
            .map(|handle| AgentInfo {
                id: handle.id,
                name: handle.name.clone(),
                profile: handle.profile.clone(),
                status: handle.status,
                task_count: handle.task_count,
            })
    }

    /// Get the currently active agent.
    pub fn active_agent(&self) -> Option<&Agent<C>> {
        self.agents.get(self.active_index).map(|h| &h.agent)
    }

    /// Get the currently active agent ID.
    pub fn active_agent_id(&self) -> Option<AgentId> {
        self.agents.get(self.active_index).map(|handle| handle.id)
    }

    /// Switch to a different agent by ID.
    pub fn switch_to(&mut self, agent_id: &AgentId) -> Result<()> {
        if let Some(index) = self.agents.iter().position(|agent| &agent.id == agent_id) {
            self.active_index = index;
            Ok(())
        } else {
            Err(AgentError::Command(format!("Unknown agent: {}", agent_id)))
        }
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
    pub fn kill(&mut self, agent_id: &AgentId) -> Result<()> {
        if self.agents.len() == 1 {
            return Err(AgentError::Command(
                "Cannot kill the last agent".to_string(),
            ));
        }

        let index = self
            .agents
            .iter()
            .position(|agent| &agent.id == agent_id)
            .ok_or_else(|| AgentError::Command(format!("Unknown agent: {}", agent_id)))?;
        self.agents.remove(index);

        if self.active_index >= self.agents.len() {
            self.active_index = 0;
        }

        Ok(())
    }

    /// Get peer information for call_peer tool.
    /// Excludes the specified agent from the list.
    pub fn get_peers(&self, exclude_agent_id: &AgentId) -> Vec<crate::tools::peer::PeerInfo> {
        self.agents
            .iter()
            .filter(|agent| &agent.id != exclude_agent_id)
            .map(|agent| crate::tools::peer::PeerInfo {
                id: agent.id,
                name: agent.name.clone(),
                profile: agent.profile.to_string(),
                description: agent.capabilities.join(", "),
            })
            .collect()
    }

    /// Check if call_peer should be enabled (2+ agents).
    pub fn is_call_peer_enabled(&self) -> bool {
        self.agents.len() >= 2
    }

    fn select_agent_index(&self, team_id: Option<TeamId>, task: &Task) -> Result<usize> {
        let candidate_ids = team_id.and_then(|team_id| {
            self.teams
                .get_team(team_id)
                .map(|team| team.members.clone())
        });

        let mut selected_index = None;

        for (index, handle) in self.agents.iter().enumerate() {
            if let Some(candidate_ids) = &candidate_ids {
                if !candidate_ids.contains(&handle.id) {
                    continue;
                }
            }

            if task
                .required_capabilities
                .iter()
                .all(|capability| handle.capabilities.contains(capability))
            {
                selected_index = Some(index);
                if index == self.active_index {
                    break;
                }
            }
        }

        if selected_index.is_none() && task.required_capabilities.is_empty() {
            selected_index = if let Some(candidate_ids) = &candidate_ids {
                self.agents
                    .iter()
                    .enumerate()
                    .find(|(_, handle)| candidate_ids.contains(&handle.id))
                    .map(|(index, _)| index)
            } else if self.agents.is_empty() {
                None
            } else {
                Some(self.active_index.min(self.agents.len() - 1))
            };
        }

        selected_index.ok_or_else(|| {
            if task.required_capabilities.is_empty() {
                AgentError::Command("No agents available".to_string())
            } else {
                AgentError::Command(format!(
                    "No agent matched capabilities: {}",
                    task.required_capabilities.join(", ")
                ))
            }
        })
    }

    /// Get the total number of agents.
    pub fn agent_count(&self) -> usize {
        self.agents.len()
    }
}
