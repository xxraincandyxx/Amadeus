// @amadeus-header
// summary: Agent subsystem code for manager.
// layer: agent
// status: active
// feature_flags:
// - orchestra
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
// - module: amadeus_runtime::agent
// invariants:
// - Listed interfaces stay aligned with the implementation in this file.
// side_effects: none
// tests:
// - tests/agent_integration_test.rs
// @end-amadeus-header

//! Legacy orchestration registry implementation backing the orchestra surface.

use std::sync::Arc;
use std::time::Instant;

use crate::agent::config::Config;
use crate::agent::loop_agent::Agent;
use crate::agent::profile::AgentProfile;
use crate::agent::team::{AgentTeam, TeamLeader, TeamRegistry};
use crate::agent::worker::{Task, TaskResult, WorkerConfig};
use crate::client::LLMClient;
use crate::core::id::{AgentId, TeamId};
use crate::error::{AgentError, Result};
pub use amadeus_runtime::{
    find_agent_index, get_agent_info, list_agent_info, next_agent_index,
    normalize_active_index_after_removal, previous_agent_index, select_agent, AgentInfo,
    AgentRouteCandidate, AgentStatus,
};

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
    fn roster_entries(&self) -> Vec<AgentInfo> {
        self.agents
            .iter()
            .map(|handle| AgentInfo {
                id: handle.id,
                name: handle.name.clone(),
                profile: handle.profile.clone(),
                status: handle.status,
                task_count: handle.task_count,
            })
            .collect()
    }

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

    /// Create a new orchestra and return its identifier.
    pub fn create_orchestra(&mut self, name: impl Into<String>, leader: TeamLeader) -> TeamId {
        self.teams.create_team(name, leader)
    }

    /// Create a new team and return its identifier.
    #[deprecated(note = "use create_orchestra")]
    pub fn create_team(&mut self, name: impl Into<String>, leader: TeamLeader) -> TeamId {
        self.create_orchestra(name, leader)
    }

    /// Ensure there is always a default orchestra for task routing.
    pub fn ensure_default_orchestra(&mut self, leader: TeamLeader) -> TeamId {
        if let Some(team_id) = self.teams.default_team_id() {
            return team_id;
        }
        self.teams.create_team("default", leader)
    }

    /// Ensure there is always a default team for task routing.
    #[deprecated(note = "use ensure_default_orchestra")]
    pub fn ensure_default_team(&mut self, leader: TeamLeader) -> TeamId {
        self.ensure_default_orchestra(leader)
    }

    /// List all orchestras.
    pub fn list_orchestras(&self) -> Vec<AgentTeam> {
        self.teams.list_teams()
    }

    /// List all teams.
    #[deprecated(note = "use list_orchestras")]
    pub fn list_teams(&self) -> Vec<AgentTeam> {
        self.list_orchestras()
    }

    /// Add an agent to an orchestra.
    pub fn add_agent_to_orchestra(
        &mut self,
        orchestra_id: TeamId,
        agent_id: AgentId,
    ) -> Result<()> {
        if !self.agents.iter().any(|agent| agent.id == agent_id) {
            return Err(AgentError::Command(format!("Unknown agent: {}", agent_id)));
        }
        self.teams.add_member(orchestra_id, agent_id)?;
        Ok(())
    }

    /// Add an agent to a team.
    #[deprecated(note = "use add_agent_to_orchestra")]
    pub fn add_agent_to_team(&mut self, team_id: TeamId, agent_id: AgentId) -> Result<()> {
        self.add_agent_to_orchestra(team_id, agent_id)
    }

    /// Execute an orchestra task using the best available local agent.
    pub async fn execute_task(
        &mut self,
        orchestra_id: Option<TeamId>,
        task: Task,
    ) -> Result<TaskResult> {
        let target_team_id = orchestra_id.or_else(|| self.teams.default_team_id());
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
        list_agent_info(&self.roster_entries(), self.active_agent_id())
    }

    /// Get info for a specific agent.
    pub fn get_agent(&self, agent_id: &AgentId) -> Option<AgentInfo> {
        get_agent_info(&self.roster_entries(), *agent_id)
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
        if let Some(index) = find_agent_index(&self.roster_entries(), *agent_id) {
            self.active_index = index;
            Ok(())
        } else {
            Err(AgentError::Command(format!("Unknown agent: {}", agent_id)))
        }
    }

    /// Switch to the next agent.
    pub fn switch_next(&mut self) {
        if let Some(index) = next_agent_index(self.agents.len(), self.active_index) {
            self.active_index = index;
        }
    }

    /// Switch to the previous agent.
    pub fn switch_prev(&mut self) {
        if let Some(index) = previous_agent_index(self.agents.len(), self.active_index) {
            self.active_index = index;
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
        self.active_index =
            normalize_active_index_after_removal(self.agents.len(), self.active_index);

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
        let allowed_ids = team_id.and_then(|team_id| {
            self.teams
                .get_team(team_id)
                .map(|team| team.members.clone())
        });
        let candidates = self
            .agents
            .iter()
            .map(|handle| AgentRouteCandidate {
                id: handle.id,
                capabilities: handle.capabilities.clone(),
            })
            .collect::<Vec<_>>();
        let selected_id = select_agent(
            &candidates,
            self.active_agent_id(),
            allowed_ids.as_deref(),
            task,
        );

        selected_id
            .and_then(|agent_id| self.agents.iter().position(|handle| handle.id == agent_id))
            .ok_or_else(|| {
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
