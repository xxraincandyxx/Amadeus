// @amadeus-header
// summary: Agent team registry, mailbox, and shared task workflow primitives for runtime coordination.
// layer: core
// status: active
// feature_flags: none
// provides:
// - module: crate::team
// - type: crate::team::ArtifactRecord
// - type: crate::team::MailboxEvent
// - type: crate::team::MailboxEventKind
// - type: crate::team::TeamLeader
// - type: crate::team::TeamStatus
// - type: crate::team::TeamTaskStatus
// - type: crate::team::TeamTask
// - type: crate::team::AgentTeam
// - type: crate::team::TeamRegistry
// uses:
// - module: crate::worker
// - module: amadeus_ids
// - module: crate::RuntimeError
// - protocol: serde serialization
// invariants:
// - Team tasks retain dependency, attempt, and assignee state in the shared registry.
// - Mailbox events are append-only coordination records scoped to a team.
// side_effects: none
// tests:
// - cmd: cargo test -p runtime
// @end-amadeus-header

use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use amadeus_ids::{AgentId, TeamId};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::worker::{Task, TaskResult};
use crate::{Result, RuntimeError};

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

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
    Ready,
    Blocked,
    InProgress,
    Review,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MailboxEventKind {
    DirectMessage,
    ReviewRequest,
    ArtifactPublished,
    StatusUpdate,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ArtifactRecord {
    pub label: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MailboxEvent {
    pub id: String,
    pub kind: MailboxEventKind,
    pub task_id: Option<String>,
    pub from: Option<AgentId>,
    pub to: Option<AgentId>,
    pub content: String,
    pub created_at_ms: u64,
}

impl MailboxEvent {
    pub fn new(
        id: impl Into<String>,
        kind: MailboxEventKind,
        task_id: Option<String>,
        from: Option<AgentId>,
        to: Option<AgentId>,
        content: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            kind,
            task_id,
            from,
            to,
            content: content.into(),
            created_at_ms: now_ms(),
        }
    }
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
    pub dependencies: Vec<String>,
    pub owned_files: Vec<String>,
    pub artifacts: Vec<ArtifactRecord>,
    pub attempt_count: u32,
    pub last_claimed_at_ms: Option<u64>,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
}

impl TeamTask {
    pub fn from_task(task: Task, created_by: TeamLeader) -> Self {
        let now = now_ms();
        let status = if task.dependencies.is_empty() {
            TeamTaskStatus::Ready
        } else {
            TeamTaskStatus::Blocked
        };

        Self {
            id: task.id,
            prompt: task.prompt,
            required_capabilities: task.required_capabilities,
            priority: task.priority,
            metadata: task.metadata,
            created_by,
            status,
            assignee: None,
            output: None,
            error: None,
            dependencies: task.dependencies,
            owned_files: task.owned_files,
            artifacts: Vec::new(),
            attempt_count: 0,
            last_claimed_at_ms: None,
            created_at_ms: now,
            updated_at_ms: now,
        }
    }

    pub fn dependencies_satisfied(&self, tasks: &[TeamTask]) -> bool {
        self.dependencies.iter().all(|dependency| {
            tasks.iter().any(|task| {
                task.id == *dependency && matches!(task.status, TeamTaskStatus::Completed)
            })
        })
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
    pub mailbox: Vec<MailboxEvent>,
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
            mailbox: Vec::new(),
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
            .ok_or_else(|| RuntimeError::Command(format!("Unknown team: {}", team_id)))?;

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
            .ok_or_else(|| RuntimeError::Command(format!("Unknown team: {}", team_id)))?;

        team.tasks.push(TeamTask::from_task(task, created_by));
        self.refresh_team_statuses(team_id)?;
        Ok(())
    }

    pub fn claim_task(&mut self, team_id: TeamId, task_id: &str, agent_id: AgentId) -> Result<()> {
        let team = self
            .teams
            .iter_mut()
            .find(|team| team.id == team_id)
            .ok_or_else(|| RuntimeError::Command(format!("Unknown team: {}", team_id)))?;

        let dependency_ready = {
            let task = team
                .tasks
                .iter()
                .find(|task| task.id == task_id)
                .ok_or_else(|| RuntimeError::Command(format!("Unknown team task: {}", task_id)))?;
            task.dependencies_satisfied(&team.tasks)
        };

        let task = team
            .tasks
            .iter_mut()
            .find(|task| task.id == task_id)
            .ok_or_else(|| RuntimeError::Command(format!("Unknown team task: {}", task_id)))?;

        if !dependency_ready {
            task.status = TeamTaskStatus::Blocked;
            task.updated_at_ms = now_ms();
            return Err(RuntimeError::Command(format!(
                "Task dependencies are not satisfied: {}",
                task_id
            )));
        }

        task.assignee = Some(agent_id);
        self.record_attempt(team_id, task_id)?;
        Ok(())
    }

    pub fn record_attempt(&mut self, team_id: TeamId, task_id: &str) -> Result<()> {
        let team = self
            .teams
            .iter_mut()
            .find(|team| team.id == team_id)
            .ok_or_else(|| RuntimeError::Command(format!("Unknown team: {}", team_id)))?;

        let task = team
            .tasks
            .iter_mut()
            .find(|task| task.id == task_id)
            .ok_or_else(|| RuntimeError::Command(format!("Unknown team task: {}", task_id)))?;

        task.status = TeamTaskStatus::InProgress;
        task.attempt_count += 1;
        task.last_claimed_at_ms = Some(now_ms());
        task.updated_at_ms = now_ms();
        Ok(())
    }

    pub fn mark_review(&mut self, team_id: TeamId, task_id: &str, agent_id: AgentId) -> Result<()> {
        let team = self
            .teams
            .iter_mut()
            .find(|team| team.id == team_id)
            .ok_or_else(|| RuntimeError::Command(format!("Unknown team: {}", team_id)))?;

        let task = team
            .tasks
            .iter_mut()
            .find(|task| task.id == task_id)
            .ok_or_else(|| RuntimeError::Command(format!("Unknown team task: {}", task_id)))?;

        task.assignee = Some(agent_id);
        task.status = TeamTaskStatus::Review;
        task.updated_at_ms = now_ms();
        Ok(())
    }

    pub fn mark_retry_ready(
        &mut self,
        team_id: TeamId,
        task_id: &str,
        agent_id: AgentId,
        error: impl Into<String>,
    ) -> Result<()> {
        let team = self
            .teams
            .iter_mut()
            .find(|team| team.id == team_id)
            .ok_or_else(|| RuntimeError::Command(format!("Unknown team: {}", team_id)))?;

        let task = team
            .tasks
            .iter_mut()
            .find(|task| task.id == task_id)
            .ok_or_else(|| RuntimeError::Command(format!("Unknown team task: {}", task_id)))?;

        task.assignee = Some(agent_id);
        task.error = Some(error.into());
        task.status = TeamTaskStatus::Ready;
        task.updated_at_ms = now_ms();
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
            .ok_or_else(|| RuntimeError::Command(format!("Unknown team: {}", team_id)))?;

        let task = team
            .tasks
            .iter_mut()
            .find(|task| task.id == task_id)
            .ok_or_else(|| RuntimeError::Command(format!("Unknown team task: {}", task_id)))?;

        task.assignee = Some(agent_id);
        task.output = result.output.clone();
        task.error = result.error.clone();
        task.status = if result.success {
            TeamTaskStatus::Completed
        } else {
            TeamTaskStatus::Failed
        };
        task.updated_at_ms = now_ms();

        self.refresh_team_statuses(team_id)?;
        Ok(())
    }

    pub fn add_artifact(
        &mut self,
        team_id: TeamId,
        task_id: &str,
        artifact: ArtifactRecord,
    ) -> Result<()> {
        let team = self
            .teams
            .iter_mut()
            .find(|team| team.id == team_id)
            .ok_or_else(|| RuntimeError::Command(format!("Unknown team: {}", team_id)))?;

        let task = team
            .tasks
            .iter_mut()
            .find(|task| task.id == task_id)
            .ok_or_else(|| RuntimeError::Command(format!("Unknown team task: {}", task_id)))?;

        task.artifacts.push(artifact);
        task.updated_at_ms = now_ms();
        Ok(())
    }

    pub fn record_mailbox_event(&mut self, team_id: TeamId, event: MailboxEvent) -> Result<()> {
        let team = self
            .teams
            .iter_mut()
            .find(|team| team.id == team_id)
            .ok_or_else(|| RuntimeError::Command(format!("Unknown team: {}", team_id)))?;

        team.mailbox.push(event);
        Ok(())
    }

    pub fn refresh_team_statuses(&mut self, team_id: TeamId) -> Result<()> {
        let team = self
            .teams
            .iter_mut()
            .find(|team| team.id == team_id)
            .ok_or_else(|| RuntimeError::Command(format!("Unknown team: {}", team_id)))?;

        let completed_ids = team
            .tasks
            .iter()
            .filter(|task| matches!(task.status, TeamTaskStatus::Completed))
            .map(|task| task.id.clone())
            .collect::<Vec<_>>();

        for task in &mut team.tasks {
            if matches!(
                task.status,
                TeamTaskStatus::Completed
                    | TeamTaskStatus::Failed
                    | TeamTaskStatus::InProgress
                    | TeamTaskStatus::Review
            ) {
                continue;
            }

            let ready = task
                .dependencies
                .iter()
                .all(|dependency| completed_ids.iter().any(|id| id == dependency));
            task.status = if ready {
                TeamTaskStatus::Ready
            } else {
                TeamTaskStatus::Blocked
            };
            task.updated_at_ms = now_ms();
        }

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
        assert_eq!(team.tasks[0].attempt_count, 1);
    }

    #[test]
    fn claim_task_blocks_until_dependencies_finish() {
        let mut registry = TeamRegistry::new();
        let team_id = registry.create_team("default", TeamLeader::User);
        let agent_id = AgentId::new();
        let dependency = Task::new("task-1", "dependency");
        let dependent = Task::new("task-2", "dependent").depends_on(vec!["task-1".to_string()]);

        registry
            .queue_task(team_id, dependency, TeamLeader::User)
            .unwrap();
        registry
            .queue_task(team_id, dependent, TeamLeader::User)
            .unwrap();

        assert!(registry.claim_task(team_id, "task-2", agent_id).is_err());

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

        registry.claim_task(team_id, "task-2", agent_id).unwrap();
        let team = registry.get_team(team_id).unwrap();
        let task = team.tasks.iter().find(|task| task.id == "task-2").unwrap();
        assert_eq!(task.status, TeamTaskStatus::InProgress);
    }

    #[test]
    fn mailbox_events_are_recorded() {
        let mut registry = TeamRegistry::new();
        let team_id = registry.create_team("default", TeamLeader::User);
        let worker_id = AgentId::new();

        registry
            .record_mailbox_event(
                team_id,
                MailboxEvent::new(
                    "evt-1",
                    MailboxEventKind::ReviewRequest,
                    Some("task-1".to_string()),
                    Some(worker_id),
                    None,
                    "Please review the patch",
                ),
            )
            .unwrap();

        let team = registry.get_team(team_id).unwrap();
        assert_eq!(team.mailbox.len(), 1);
        assert_eq!(team.mailbox[0].kind, MailboxEventKind::ReviewRequest);
    }
}
