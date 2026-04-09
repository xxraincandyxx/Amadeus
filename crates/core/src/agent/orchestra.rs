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
// - module: crate::agent::manager
// - module: crate::agent::profile::AgentProfile
// - module: crate::agent::supervisor
// - module: crate::agent::worker
// - module: crate::client::LLMClient
// - module: crate::concurrency::LockManager
// - module: crate::core::id
// - module: crate::error
// - module: amadeus_runtime::orchestra
// invariants:
// - Orchestra naming remains the primary public surface while legacy modules stay deprecated.
// side_effects: none
// tests:
// - tests/agent_integration_test.rs
// - tests/p2p_test.rs
// - tests/e2e_product_flow.rs
// @end-amadeus-header

use std::ops::{Deref, DerefMut};
use std::sync::Arc;

pub use super::manager::{AgentInfo, AgentStatus};
pub use super::worker::{Task, TaskResult, WorkerConfig, WorkerInfo, WorkerStatus};
use crate::agent::config::Config;
use crate::agent::profile::AgentProfile;
use crate::client::LLMClient;
use crate::concurrency::LockManager;
use crate::core::id::{AgentId, TeamId};
use crate::error::Result;
pub use amadeus_runtime::{
    AgentOrchestra, OrchestraConfig, OrchestraLeader, OrchestraRegistry, OrchestraStatus,
    OrchestraStrategy, OrchestraTask, OrchestraTaskStatus,
};
use tokio::sync::Mutex;

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
        self.inner.create_team(name, leader)
    }

    /// Ensure there is always a default orchestra for task routing.
    pub fn ensure_default_orchestra(&mut self, leader: OrchestraLeader) -> TeamId {
        self.inner.ensure_default_team(leader)
    }

    /// List all orchestras.
    pub fn list_orchestras(&self) -> Vec<AgentOrchestra> {
        self.inner.list_teams()
    }

    /// Add an agent to an orchestra.
    pub fn add_agent_to_orchestra(
        &mut self,
        orchestra_id: TeamId,
        agent_id: AgentId,
    ) -> Result<()> {
        self.inner.add_agent_to_team(orchestra_id, agent_id)
    }

    /// Execute a task using the best available local agent in the selected orchestra.
    pub async fn execute_task(
        &mut self,
        orchestra_id: Option<TeamId>,
        task: Task,
    ) -> Result<TaskResult> {
        self.inner.execute_task(orchestra_id, task).await
    }
}

impl<C: LLMClient> Deref for AgentOrchestrator<C> {
    type Target = super::manager::AgentManager<C>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<C: LLMClient> DerefMut for AgentOrchestrator<C> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
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
        self.inner.spawn(configs).await
    }

    /// Spawn agents using a specific client implementation.
    pub async fn spawn_agents_with_client(
        &mut self,
        configs: Vec<WorkerConfig>,
        client: C,
    ) -> Result<Vec<AgentId>> {
        self.inner.spawn_with_client(configs, client).await
    }

    /// Get execution info for a specific agent in the orchestra runtime.
    pub async fn agent_info(&self, agent_id: AgentId) -> Option<WorkerInfo> {
        self.inner.worker(agent_id).await
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

impl<C: LLMClient> Deref for OrchestraRuntime<C> {
    type Target = super::supervisor::Supervisor<C>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}
