// @amadeus-header
// summary: Agent team registry and shared task list primitives for multi-session coordination.
// layer: agent
// status: active
// feature_flags:
// - team
// provides:
// - module: crate::agent::team
// - type: crate::agent::team::TeamLeader
// - type: crate::agent::team::TeamStatus
// - type: crate::agent::team::TeamTaskStatus
// - type: crate::agent::team::TeamTask
// - type: crate::agent::team::AgentTeam
// - type: crate::agent::team::TeamRegistry
// uses:
// - module: crate::agent::worker
// - module: crate::core::id
// - module: crate::error
// - protocol: serde serialization
// invariants:
// - Team tasks remain append-only status records inside the shared task list.
// side_effects: none
// tests:
// - cmd: cargo test -p core team_registry_tracks_shared_tasks --features full
// @end-amadeus-header

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::agent::worker::{Task, TaskResult};
use crate::core::id::{AgentId, TeamId};
use crate::error::{AgentError, Result};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TeamLeader {
    User,
    Agent(AgentId),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TeamStatus {
    Active,
    Archived,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TeamTaskStatus {
    Pending,
    Claimed,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamTask {
    pub id: String,
    pub prompt: String,
    pub required_capabilities: Vec<String>,
    pub priority: u8,
    pub metadata: HashMap<String, Value>,
    pub created_by: TeamLeader,
    pub status: TeamTaskStatus,
    pub assignee: Option<AgentId>,
    pub output: Option<String>,
    pub error: Option<String>,
}

impl TeamTask {
    pub fn from_task(task: Task, created_by: TeamLeader) -> Self {
        Self {
            id: task.id,
            prompt: task.prompt,
            required_capabilities: task.required_capabilities,
            priority: task.priority,
            metadata: task.metadata,
            created_by,
            status: TeamTaskStatus::Pending,
            assignee: None,
            output: None,
            error: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentTeam {
    pub id: TeamId,
    pub name: String,
    pub leader: TeamLeader,
    pub status: TeamStatus,
    pub members: Vec<AgentId>,
    pub tasks: Vec<TeamTask>,
}

#[derive(Debug, Clone, Default)]
pub struct TeamRegistry {
    teams: Vec<AgentTeam>,
    default_team_id: Option<TeamId>,
}

impl TeamRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn create_team(&mut self, name: impl Into<String>, leader: TeamLeader) -> TeamId {
        let team = AgentTeam {
            id: TeamId::new(),
            name: name.into(),
            leader,
            status: TeamStatus::Active,
            members: Vec::new(),
            tasks: Vec::new(),
        };
        let team_id = team.id;
        if self.default_team_id.is_none() {
            self.default_team_id = Some(team_id);
        }
        self.teams.push(team);
        team_id
    }

    pub fn set_default_team(&mut self, team_id: TeamId) {
        self.default_team_id = Some(team_id);
    }

    pub fn default_team_id(&self) -> Option<TeamId> {
        self.default_team_id
    }

    pub fn list_teams(&self) -> Vec<AgentTeam> {
        self.teams.clone()
    }

    pub fn get_team(&self, team_id: TeamId) -> Option<&AgentTeam> {
        self.teams.iter().find(|team| team.id == team_id)
    }

    pub fn add_member(&mut self, team_id: TeamId, agent_id: AgentId) -> Result<()> {
        let team = self
            .teams
            .iter_mut()
            .find(|team| team.id == team_id)
            .ok_or_else(|| AgentError::Command(format!("Unknown team: {}", team_id)))?;

        if !team.members.contains(&agent_id) {
            team.members.push(agent_id);
        }

        Ok(())
    }

    pub fn queue_task(
        &mut self,
        team_id: TeamId,
        task: Task,
        created_by: TeamLeader,
    ) -> Result<()> {
        let team = self
            .teams
            .iter_mut()
            .find(|team| team.id == team_id)
            .ok_or_else(|| AgentError::Command(format!("Unknown team: {}", team_id)))?;

        team.tasks.push(TeamTask::from_task(task, created_by));
        Ok(())
    }

    pub fn claim_task(&mut self, team_id: TeamId, task_id: &str, agent_id: AgentId) -> Result<()> {
        let team = self
            .teams
            .iter_mut()
            .find(|team| team.id == team_id)
            .ok_or_else(|| AgentError::Command(format!("Unknown team: {}", team_id)))?;

        let task = team
            .tasks
            .iter_mut()
            .find(|task| task.id == task_id)
            .ok_or_else(|| AgentError::Command(format!("Unknown team task: {}", task_id)))?;

        task.status = TeamTaskStatus::Claimed;
        task.assignee = Some(agent_id);
        Ok(())
    }

    pub fn record_result(
        &mut self,
        team_id: TeamId,
        task_id: &str,
        agent_id: AgentId,
        result: &TaskResult,
    ) -> Result<()> {
        let team = self
            .teams
            .iter_mut()
            .find(|team| team.id == team_id)
            .ok_or_else(|| AgentError::Command(format!("Unknown team: {}", team_id)))?;

        let task = team
            .tasks
            .iter_mut()
            .find(|task| task.id == task_id)
            .ok_or_else(|| AgentError::Command(format!("Unknown team task: {}", task_id)))?;

        task.assignee = Some(agent_id);
        task.output = result.output.clone();
        task.error = result.error.clone();
        task.status = if result.success {
            TeamTaskStatus::Completed
        } else {
            TeamTaskStatus::Failed
        };

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn team_registry_tracks_shared_tasks() {
        let mut registry = TeamRegistry::new();
        let team_id = registry.create_team("default", TeamLeader::User);
        let agent_id = AgentId::new();
        let task = Task::new("task-1", "test prompt").requires(vec!["bash".to_string()]);

        registry.add_member(team_id, agent_id).unwrap();
        registry
            .queue_task(team_id, task, TeamLeader::User)
            .unwrap();
        registry.claim_task(team_id, "task-1", agent_id).unwrap();
        registry
            .record_result(
                team_id,
                "task-1",
                agent_id,
                &TaskResult {
                    task_id: "task-1".to_string(),
                    worker_id: agent_id,
                    success: true,
                    output: Some("done".to_string()),
                    error: None,
                    duration_ms: 1,
                    tool_calls: Vec::new(),
                },
            )
            .unwrap();

        let team = registry.get_team(team_id).unwrap();
        assert_eq!(team.members, vec![agent_id]);
        assert_eq!(team.tasks.len(), 1);
        assert_eq!(team.tasks[0].status, TeamTaskStatus::Completed);
        assert_eq!(team.tasks[0].output.as_deref(), Some("done"));
    }
}
