// @amadeus-header
// summary: Legacy orchestration registry wrapper delegating roster logic to the orchestra module.
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
// - module: crate::agent::orchestra
// - module: crate::agent::profile::AgentProfile
// - module: crate::agent::team
// - module: crate::agent::worker
// - module: crate::client::LLMClient
// - module: crate::core::id
// - module: crate::error
// - module: amadeus_runtime::agent
// invariants:
// - Team coordination remains behavior-compatible while roster ownership lives in orchestra.
// side_effects: none
// tests:
// - tests/agent_integration_test.rs
// @end-amadeus-header

use std::sync::Arc;
use std::time::Instant;

use crate::agent::config::Config;
use crate::agent::orchestra::OrchestraRoster;
use crate::agent::profile::AgentProfile;
use crate::agent::team::{AgentTeam, TeamLeader, TeamRegistry};
use crate::agent::worker::{Task, TaskResult, WorkerConfig};
use crate::client::LLMClient;
use crate::core::id::{AgentId, TeamId};
use crate::error::{AgentError, Result};
pub use amadeus_runtime::{AgentInfo, AgentStatus};

pub struct AgentManager<C: LLMClient> {
    roster: OrchestraRoster<C>,
    teams: TeamRegistry,
}

impl<C: LLMClient + Clone + 'static> AgentManager<C> {
    pub fn new(client: C, config: Arc<Config>) -> Self {
        Self {
            roster: OrchestraRoster::new(client, config),
            teams: TeamRegistry::new(),
        }
    }

    pub async fn create_agent(
        &mut self,
        name: Option<String>,
        profile: AgentProfile,
    ) -> Result<AgentId> {
        self.roster.create_agent(name, profile).await
    }

    pub async fn spawn_teammate(&mut self, config: WorkerConfig) -> Result<AgentId> {
        self.roster.spawn_agent(config).await
    }

    pub fn create_orchestra(&mut self, name: impl Into<String>, leader: TeamLeader) -> TeamId {
        self.teams.create_team(name, leader)
    }

    #[deprecated(note = "use create_orchestra")]
    pub fn create_team(&mut self, name: impl Into<String>, leader: TeamLeader) -> TeamId {
        self.create_orchestra(name, leader)
    }

    pub fn ensure_default_orchestra(&mut self, leader: TeamLeader) -> TeamId {
        if let Some(team_id) = self.teams.default_team_id() {
            return team_id;
        }
        self.teams.create_team("default", leader)
    }

    #[deprecated(note = "use ensure_default_orchestra")]
    pub fn ensure_default_team(&mut self, leader: TeamLeader) -> TeamId {
        self.ensure_default_orchestra(leader)
    }

    pub fn list_orchestras(&self) -> Vec<AgentTeam> {
        self.teams.list_teams()
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
        if !self.roster.contains(agent_id) {
            return Err(AgentError::Command(format!("Unknown agent: {}", agent_id)));
        }
        self.teams.add_member(orchestra_id, agent_id)?;
        Ok(())
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
        let target_orchestra_id = orchestra_id.or_else(|| self.teams.default_team_id());
        let allowed_ids = target_orchestra_id.and_then(|team_id| {
            self.teams
                .get_team(team_id)
                .map(|team| team.members.clone())
        });
        let selected_index = self
            .roster
            .select_agent_index(allowed_ids.as_deref(), &task)?;
        let selected_id = self.roster.agent_id_at(selected_index);
        let agent = self.roster.agent_clone_at(selected_index);

        if let Some(team_id) = target_orchestra_id {
            self.teams
                .queue_task(team_id, task.clone(), TeamLeader::User)?;
            self.teams.claim_task(team_id, &task.id, selected_id)?;
        }

        self.roster.mark_running(selected_index);

        let start = Instant::now();
        let result = agent.run(&task.prompt).await;
        let duration_ms = start.elapsed().as_millis() as u64;
        self.roster.mark_idle(selected_index);

        let task_result = match result {
            Ok(run_result) => {
                self.roster.increment_task_count(selected_index);
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
                self.roster.mark_error(selected_index);
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

        if let Some(team_id) = target_orchestra_id {
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

    pub fn list_agents(&self) -> Vec<AgentInfo> {
        self.roster.list_agents()
    }

    pub fn get_agent(&self, agent_id: &AgentId) -> Option<AgentInfo> {
        self.roster.get_agent(agent_id)
    }

    pub fn active_agent_id(&self) -> Option<AgentId> {
        self.roster.active_agent_id()
    }

    pub fn switch_to(&mut self, agent_id: &AgentId) -> Result<()> {
        self.roster.switch_to(agent_id)
    }

    pub fn switch_next(&mut self) {
        self.roster.switch_next();
    }

    pub fn switch_prev(&mut self) {
        self.roster.switch_prev();
    }

    pub fn kill(&mut self, agent_id: &AgentId) -> Result<()> {
        self.roster.kill(agent_id)
    }

    pub fn get_peers(&self, exclude_agent_id: &AgentId) -> Vec<crate::tools::peer::PeerInfo> {
        self.roster.peer_info(exclude_agent_id)
    }

    pub fn is_call_peer_enabled(&self) -> bool {
        self.roster.call_peer_enabled()
    }

    pub fn agent_count(&self) -> usize {
        self.roster.agent_count()
    }
}
