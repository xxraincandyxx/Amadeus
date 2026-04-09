// @amadeus-header
// summary: Canonical orchestra surface unifying orchestration naming across core runtime APIs.
// layer: agent
// status: active
// feature_flags:
// - orchestra
// provides:
// - module: crate::agent::orchestra
// - type: crate::agent::orchestra::AgentInfo
// - type: crate::agent::orchestra::AgentStatus
// - type: crate::agent::orchestra::AgentOrchestrator
// - type: crate::agent::orchestra::OrchestraRuntime
// - type: crate::agent::orchestra::OrchestraLeader
// - type: crate::agent::orchestra::AgentOrchestra
// - type: crate::agent::orchestra::OrchestraRegistry
// - type: crate::agent::orchestra::OrchestraConfig
// - type: crate::agent::orchestra::OrchestraStrategy
// - type: crate::agent::orchestra::Task
// - type: crate::agent::orchestra::TaskResult
// - type: crate::agent::orchestra::WorkerConfig
// - type: crate::agent::orchestra::WorkerInfo
// - type: crate::agent::orchestra::WorkerStatus
// uses:
// - module: crate::agent::config::Config
// - module: crate::agent::loop_agent::Agent
// - module: crate::agent::profile::AgentProfile
// - module: crate::agent::supervisor
// - module: crate::agent::worker
// - module: crate::client::LLMClient
// - module: crate::concurrency::LockManager
// - module: crate::core::id
// - module: crate::error
// - module: amadeus_runtime
// invariants:
// - Orchestra naming remains the primary public surface while legacy modules stay deprecated.
// side_effects: none
// tests:
// - tests/agent_integration_test.rs
// - tests/p2p_test.rs
// - tests/e2e_product_flow.rs
// @end-amadeus-header

use std::sync::Arc;

pub use super::worker::{Task, TaskResult, WorkerConfig, WorkerInfo, WorkerStatus};
use crate::agent::config::Config;
use crate::agent::loop_agent::Agent;
use crate::agent::profile::AgentProfile;
use crate::client::LLMClient;
use crate::concurrency::LockManager;
use crate::core::id::{AgentId, TeamId};
use crate::error::{AgentError, Result};
pub use amadeus_runtime::{
    AgentInfo, AgentOrchestra, AgentStatus, OrchestraConfig, OrchestraLeader, OrchestraRegistry,
    OrchestraStatus, OrchestraStrategy, OrchestraTask, OrchestraTaskStatus,
};
use tokio::sync::Mutex;

pub(crate) struct OrchestratedAgent<C: LLMClient> {
    pub(crate) id: AgentId,
    pub(crate) agent: Agent<C>,
    pub(crate) name: String,
    pub(crate) profile: AgentProfile,
    pub(crate) capabilities: Vec<String>,
    pub(crate) status: AgentStatus,
    pub(crate) task_count: usize,
}

pub(crate) struct OrchestraRoster<C: LLMClient> {
    client: C,
    config: Arc<Config>,
    pub(crate) agents: Vec<OrchestratedAgent<C>>,
    pub(crate) active_index: usize,
    name_counter: usize,
}

impl<C: LLMClient + Clone + 'static> OrchestraRoster<C> {
    pub(crate) fn new(client: C, config: Arc<Config>) -> Self {
        Self {
            client,
            config,
            agents: Vec::new(),
            active_index: 0,
            name_counter: 0,
        }
    }

    pub(crate) async fn create_agent(
        &mut self,
        name: Option<String>,
        profile: AgentProfile,
    ) -> Result<AgentId> {
        self.create_agent_with_capabilities(name, profile, Vec::new(), None)
            .await
    }

    pub(crate) async fn spawn_agent(&mut self, config: WorkerConfig) -> Result<AgentId> {
        self.create_agent_with_capabilities(
            Some(config.name),
            AgentProfile::Default,
            config.capabilities,
            config.model,
        )
        .await
    }

    pub(crate) fn list_agents(&self) -> Vec<AgentInfo> {
        amadeus_runtime::list_agent_info(&self.roster_entries(), self.active_agent_id())
    }

    pub(crate) fn get_agent(&self, agent_id: &AgentId) -> Option<AgentInfo> {
        amadeus_runtime::get_agent_info(&self.roster_entries(), *agent_id)
    }

    pub(crate) fn active_agent_id(&self) -> Option<AgentId> {
        self.agents.get(self.active_index).map(|handle| handle.id)
    }

    pub(crate) fn switch_to(&mut self, agent_id: &AgentId) -> Result<()> {
        if let Some(index) = amadeus_runtime::find_agent_index(&self.roster_entries(), *agent_id) {
            self.active_index = index;
            Ok(())
        } else {
            Err(AgentError::Command(format!("Unknown agent: {}", agent_id)))
        }
    }

    pub(crate) fn kill(&mut self, agent_id: &AgentId) -> Result<()> {
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
        self.active_index = amadeus_runtime::normalize_active_index_after_removal(
            self.agents.len(),
            self.active_index,
        );
        Ok(())
    }

    pub(crate) fn contains(&self, agent_id: AgentId) -> bool {
        self.agents.iter().any(|agent| agent.id == agent_id)
    }

    pub(crate) fn select_agent_index(
        &self,
        allowed_ids: Option<&[AgentId]>,
        task: &Task,
    ) -> Result<usize> {
        let candidates = self
            .agents
            .iter()
            .map(|handle| amadeus_runtime::AgentRouteCandidate {
                id: handle.id,
                capabilities: handle.capabilities.clone(),
            })
            .collect::<Vec<_>>();
        let selected_id =
            amadeus_runtime::select_agent(&candidates, self.active_agent_id(), allowed_ids, task);

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

    pub(crate) fn agent_id_at(&self, index: usize) -> AgentId {
        self.agents[index].id
    }

    pub(crate) fn agent_clone_at(&self, index: usize) -> Agent<C> {
        self.agents[index].agent.clone()
    }

    pub(crate) fn mark_running(&mut self, index: usize) {
        self.agents[index].status = AgentStatus::Running;
        self.active_index = index;
    }

    pub(crate) fn mark_idle(&mut self, index: usize) {
        self.agents[index].status = AgentStatus::Idle;
    }

    pub(crate) fn mark_error(&mut self, index: usize) {
        self.agents[index].status = AgentStatus::Error;
    }

    pub(crate) fn increment_task_count(&mut self, index: usize) {
        self.agents[index].task_count += 1;
    }

    pub(crate) fn peer_info(
        &self,
        exclude_agent_id: &AgentId,
    ) -> Vec<crate::tools::peer::PeerInfo> {
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

    pub(crate) fn call_peer_enabled(&self) -> bool {
        self.agents.len() >= 2
    }

    pub(crate) fn agent_count(&self) -> usize {
        self.agents.len()
    }

    pub(crate) fn switch_next(&mut self) {
        if let Some(index) = amadeus_runtime::next_agent_index(self.agents.len(), self.active_index)
        {
            self.active_index = index;
        }
    }

    pub(crate) fn switch_prev(&mut self) {
        if let Some(index) =
            amadeus_runtime::previous_agent_index(self.agents.len(), self.active_index)
        {
            self.active_index = index;
        }
    }

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

        self.agents.push(OrchestratedAgent {
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
}

/// Canonical orchestra-aware agent registry and routing surface.
pub struct AgentOrchestrator<C: LLMClient> {
    inner: super::manager::AgentManager<C>,
}

impl<C: LLMClient + Clone + 'static> AgentOrchestrator<C> {
    /// Create a new orchestrator.
    pub fn new(client: C, config: Arc<Config>) -> Self {
        Self {
            inner: super::manager::AgentManager::new(client, config),
        }
    }

    /// Create a new agent using the given profile.
    pub async fn create_agent(
        &mut self,
        name: Option<String>,
        profile: AgentProfile,
    ) -> Result<AgentId> {
        self.inner.create_agent(name, profile).await
    }

    /// Spawn a new agent using worker-style runtime configuration.
    pub async fn spawn_agent(&mut self, config: WorkerConfig) -> Result<AgentId> {
        self.inner.spawn_teammate(config).await
    }

    /// Create a new orchestra and return its identifier.
    pub fn create_orchestra(&mut self, name: impl Into<String>, leader: OrchestraLeader) -> TeamId {
        self.inner.create_orchestra(name, leader)
    }

    /// Ensure there is always a default orchestra for task routing.
    pub fn ensure_default_orchestra(&mut self, leader: OrchestraLeader) -> TeamId {
        self.inner.ensure_default_orchestra(leader)
    }

    /// List all orchestras.
    pub fn list_orchestras(&self) -> Vec<AgentOrchestra> {
        self.inner.list_orchestras()
    }

    /// Add an agent to an orchestra.
    pub fn add_agent_to_orchestra(
        &mut self,
        orchestra_id: TeamId,
        agent_id: AgentId,
    ) -> Result<()> {
        self.inner.add_agent_to_orchestra(orchestra_id, agent_id)
    }

    /// Execute a task using the best available local agent in the selected orchestra.
    pub async fn execute_task(
        &mut self,
        orchestra_id: Option<TeamId>,
        task: Task,
    ) -> Result<TaskResult> {
        self.inner.execute_task(orchestra_id, task).await
    }

    /// List all active agents.
    pub fn list_agents(&self) -> Vec<AgentInfo> {
        self.inner.list_agents()
    }

    /// Get info for a specific agent.
    pub fn get_agent(&self, agent_id: &AgentId) -> Option<AgentInfo> {
        self.inner.get_agent(agent_id)
    }

    /// Get the currently active agent ID.
    pub fn active_agent_id(&self) -> Option<AgentId> {
        self.inner.active_agent_id()
    }

    /// Switch the active agent.
    pub fn switch_to(&mut self, agent_id: &AgentId) -> Result<()> {
        self.inner.switch_to(agent_id)
    }

    /// Remove an agent from the orchestrator.
    pub fn kill(&mut self, agent_id: &AgentId) -> Result<()> {
        self.inner.kill(agent_id)
    }
}

/// Canonical queued runtime for background orchestra execution.
pub struct OrchestraRuntime<C: LLMClient> {
    inner: super::supervisor::Supervisor<C>,
}

impl<C: LLMClient + Clone + 'static> OrchestraRuntime<C> {
    /// Create a new orchestra runtime.
    pub fn new(client: C, config: OrchestraConfig, sdk_config: Arc<Config>) -> Self {
        Self {
            inner: super::supervisor::Supervisor::new(client, config, sdk_config),
        }
    }

    /// Get the lock manager for resource coordination.
    pub fn lock_manager(&self) -> Arc<Mutex<LockManager>> {
        self.inner.lock_manager()
    }

    /// Get the base LLM client.
    pub fn client(&self) -> &C {
        self.inner.client()
    }

    /// Get the base SDK configuration.
    pub fn config(&self) -> &Arc<Config> {
        self.inner.config()
    }

    /// Spawn agents into the orchestra runtime.
    pub async fn spawn_agents(&mut self, configs: Vec<WorkerConfig>) -> Result<Vec<AgentId>> {
        self.inner.spawn_agents(configs).await
    }

    /// Spawn agents using a specific client implementation.
    pub async fn spawn_agents_with_client(
        &mut self,
        configs: Vec<WorkerConfig>,
        client: C,
    ) -> Result<Vec<AgentId>> {
        self.inner.spawn_agents_with_client(configs, client).await
    }

    /// Get execution info for a specific agent in the orchestra runtime.
    pub async fn agent_info(&self, agent_id: AgentId) -> Option<WorkerInfo> {
        self.inner.agent_info(agent_id).await
    }

    /// Run the orchestra background loop to process delegated work.
    pub async fn run(&self) -> Result<()> {
        self.inner.run().await
    }

    /// Execute a task through the queued orchestra runtime.
    pub async fn execute(&self, task: Task) -> Result<TaskResult> {
        self.inner.execute(task).await
    }
}
