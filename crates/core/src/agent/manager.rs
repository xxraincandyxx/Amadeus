// @amadeus-header
// summary: Deprecated manager shim forwarding to the orchestra registry surface.
// layer: agent
// status: deprecated
// feature_flags:
// - orchestra
// provides:
// - module: crate::agent::manager
// - type: crate::agent::manager::AgentStatus
// - type: crate::agent::manager::AgentInfo
// - type: crate::agent::manager::AgentManager
// uses:
// - module: crate::agent::config::Config
// - module: crate::agent::orchestra
// - module: crate::agent::profile::AgentProfile
// - module: crate::agent::team
// - module: crate::agent::worker
// - module: crate::client::LLMClient
// - module: crate::core::id
// - module: crate::error
// invariants:
// - Legacy manager callers continue to hit orchestra-owned registry behavior.
// side_effects: none
// tests:
// - tests/agent_integration_test.rs
// - tests/p2p_test.rs
// @end-amadeus-header

use std::sync::Arc;

pub use amadeus_runtime::{AgentInfo, AgentStatus};

use crate::agent::config::Config;
use crate::agent::orchestra::AgentOrchestrator;
use crate::agent::profile::AgentProfile;
use crate::agent::team::{AgentTeam, TeamLeader};
use crate::agent::worker::{Task, TaskResult, WorkerConfig};
use crate::client::LLMClient;
use crate::core::id::{AgentId, TeamId};
use crate::error::Result;

#[deprecated(note = "use crate::agent::orchestra::AgentOrchestrator")]
pub struct AgentManager<C: LLMClient> {
    inner: AgentOrchestrator<C>,
}

#[allow(deprecated)]
impl<C: LLMClient + Clone + 'static> AgentManager<C> {
    pub fn new(client: C, config: Arc<Config>) -> Self {
        Self {
            inner: AgentOrchestrator::new(client, config),
        }
    }

    pub async fn create_agent(
        &mut self,
        name: Option<String>,
        profile: AgentProfile,
    ) -> Result<AgentId> {
        self.inner.create_agent(name, profile).await
    }

    pub async fn spawn_teammate(&mut self, config: WorkerConfig) -> Result<AgentId> {
        self.inner.spawn_agent(config).await
    }

    pub fn create_orchestra(&mut self, name: impl Into<String>, leader: TeamLeader) -> TeamId {
        self.inner.create_orchestra(name, leader)
    }

    #[deprecated(note = "use create_orchestra")]
    pub fn create_team(&mut self, name: impl Into<String>, leader: TeamLeader) -> TeamId {
        self.create_orchestra(name, leader)
    }

    pub fn ensure_default_orchestra(&mut self, leader: TeamLeader) -> TeamId {
        self.inner.ensure_default_orchestra(leader)
    }

    #[deprecated(note = "use ensure_default_orchestra")]
    pub fn ensure_default_team(&mut self, leader: TeamLeader) -> TeamId {
        self.ensure_default_orchestra(leader)
    }

    pub fn list_orchestras(&self) -> Vec<AgentTeam> {
        self.inner.list_orchestras()
    }

    #[deprecated(note = "use list_orchestras")]
    pub fn list_teams(&self) -> Vec<AgentTeam> {
        self.list_orchestras()
    }

    pub fn add_agent_to_orchestra(
        &mut self,
        orchestra_id: TeamId,
        agent_id: AgentId,
    ) -> Result<()> {
        self.inner.add_agent_to_orchestra(orchestra_id, agent_id)
    }

    #[deprecated(note = "use add_agent_to_orchestra")]
    pub fn add_agent_to_team(&mut self, team_id: TeamId, agent_id: AgentId) -> Result<()> {
        self.add_agent_to_orchestra(team_id, agent_id)
    }

    pub async fn execute_task(
        &mut self,
        orchestra_id: Option<TeamId>,
        task: Task,
    ) -> Result<TaskResult> {
        self.inner.execute_task(orchestra_id, task).await
    }

    pub fn list_agents(&self) -> Vec<AgentInfo> {
        self.inner.list_agents()
    }

    pub fn get_agent(&self, agent_id: &AgentId) -> Option<AgentInfo> {
        self.inner.get_agent(agent_id)
    }

    pub fn active_agent_id(&self) -> Option<AgentId> {
        self.inner.active_agent_id()
    }

    pub fn switch_to(&mut self, agent_id: &AgentId) -> Result<()> {
        self.inner.switch_to(agent_id)
    }

    pub fn switch_next(&mut self) {
        self.inner.switch_next();
    }

    pub fn switch_prev(&mut self) {
        self.inner.switch_prev();
    }

    pub fn kill(&mut self, agent_id: &AgentId) -> Result<()> {
        self.inner.kill(agent_id)
    }

    pub fn get_peers(&self, exclude_agent_id: &AgentId) -> Vec<crate::tools::peer::PeerInfo> {
        self.inner.get_peers(exclude_agent_id)
    }

    pub fn is_call_peer_enabled(&self) -> bool {
        self.inner.is_call_peer_enabled()
    }

    pub fn agent_count(&self) -> usize {
        self.inner.agent_count()
    }
}
