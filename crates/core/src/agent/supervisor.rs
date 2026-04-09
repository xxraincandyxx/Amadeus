// @amadeus-header
// summary: Deprecated supervisor shim forwarding to the orchestra runtime.
// layer: agent
// status: deprecated
// feature_flags: none
// provides:
// - module: crate::agent::supervisor
// - type: crate::agent::supervisor::DispatchStrategy
// - type: crate::agent::supervisor::SupervisorConfig
// - type: crate::agent::supervisor::Supervisor
// uses:
// - module: crate::agent::config::Config
// - module: crate::agent::orchestra
// - module: crate::agent::worker
// - module: crate::client::LLMClient
// - module: crate::concurrency::LockManager
// - module: crate::core::id
// - module: crate::error
// invariants:
// - Legacy supervisor callers continue to hit orchestra-owned runtime behavior.
// side_effects:
// - Spawns asynchronous tasks through the orchestra runtime.
// tests:
// - tests/p2p_test.rs
// - tests/e2e_product_flow.rs
// @end-amadeus-header

use std::sync::Arc;

pub use super::orchestra::{
    OrchestraConfig as SupervisorConfig, OrchestraStrategy as DispatchStrategy,
};
use super::worker::{Task, TaskResult, WorkerConfig, WorkerInfo};
use crate::agent::config::Config;
use crate::agent::orchestra::OrchestraRuntime;
use crate::client::LLMClient;
use crate::concurrency::LockManager;
use crate::core::id::AgentId;
use crate::error::Result;
use tokio::sync::Mutex;

#[deprecated(note = "use crate::agent::orchestra::OrchestraRuntime")]
pub struct Supervisor<C: LLMClient> {
    inner: OrchestraRuntime<C>,
}

#[allow(deprecated)]
impl<C: LLMClient + Clone + 'static> Supervisor<C> {
    pub fn new(client: C, config: SupervisorConfig, sdk_config: Arc<Config>) -> Self {
        Self {
            inner: OrchestraRuntime::new(client, config, sdk_config),
        }
    }

    pub fn lock_manager(&self) -> Arc<Mutex<LockManager>> {
        self.inner.lock_manager()
    }

    pub fn client(&self) -> &C {
        self.inner.client()
    }

    pub fn config(&self) -> &Arc<Config> {
        self.inner.config()
    }

    pub async fn spawn_agents(&mut self, configs: Vec<WorkerConfig>) -> Result<Vec<AgentId>> {
        self.inner.spawn_agents(configs).await
    }

    #[deprecated(note = "use spawn_agents")]
    pub async fn spawn(&mut self, configs: Vec<WorkerConfig>) -> Result<Vec<AgentId>> {
        self.spawn_agents(configs).await
    }

    pub async fn spawn_agents_with_client(
        &mut self,
        configs: Vec<WorkerConfig>,
        client: C,
    ) -> Result<Vec<AgentId>> {
        self.inner.spawn_agents_with_client(configs, client).await
    }

    #[deprecated(note = "use spawn_agents_with_client")]
    pub async fn spawn_with_client(
        &mut self,
        configs: Vec<WorkerConfig>,
        client: C,
    ) -> Result<Vec<AgentId>> {
        self.spawn_agents_with_client(configs, client).await
    }

    pub async fn agent_info(&self, id: AgentId) -> Option<WorkerInfo> {
        self.inner.agent_info(id).await
    }

    #[deprecated(note = "use agent_info")]
    pub async fn worker(&self, id: AgentId) -> Option<WorkerInfo> {
        self.agent_info(id).await
    }

    pub async fn run(&self) -> Result<()> {
        self.inner.run().await
    }

    pub async fn execute(&self, task: Task) -> Result<TaskResult> {
        self.inner.execute(task).await
    }
}
