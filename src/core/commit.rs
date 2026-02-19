use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::id::{AgentId, CommitId};
use super::state::StateSnapshot;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Commit {
    pub id: CommitId,
    pub parent: Option<CommitId>,
    pub state: StateSnapshot,
    pub message: String,
    pub author: AgentId,
    pub timestamp: DateTime<Utc>,
    pub trigger: CommitTrigger,
}

impl Commit {
    pub fn new(
        parent: Option<CommitId>,
        state: StateSnapshot,
        message: String,
        author: AgentId,
        trigger: CommitTrigger,
    ) -> Self {
        Self {
            id: CommitId::new(),
            parent,
            state,
            message,
            author,
            timestamp: Utc::now(),
            trigger,
        }
    }

    pub fn is_root(&self) -> bool {
        self.parent.is_none()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CommitTrigger {
    UserRequest,
    AgentCheckpoint { agent: AgentId, step: String },
    AutoSave,
    ToolExecution { tool: String, phase: Phase },
    RecoveryPoint,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Phase {
    Before,
    After,
}

#[derive(Debug, Clone, Default)]
pub struct CommitBuilder {
    parent: Option<CommitId>,
    state: Option<StateSnapshot>,
    message: Option<String>,
    author: Option<AgentId>,
    trigger: Option<CommitTrigger>,
}

impl CommitBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn parent(mut self, parent: impl Into<Option<CommitId>>) -> Self {
        self.parent = parent.into();
        self
    }

    pub fn state(mut self, state: StateSnapshot) -> Self {
        self.state = Some(state);
        self
    }

    pub fn message(mut self, message: impl Into<String>) -> Self {
        self.message = Some(message.into());
        self
    }

    pub fn author(mut self, author: AgentId) -> Self {
        self.author = Some(author);
        self
    }

    pub fn trigger(mut self, trigger: CommitTrigger) -> Self {
        self.trigger = Some(trigger);
        self
    }

    pub fn build(self) -> Result<Commit, String> {
        let state = self.state.ok_or("state is required")?;
        let message = self.message.ok_or("message is required")?;
        let author = self.author.ok_or("author is required")?;
        let trigger = self.trigger.unwrap_or(CommitTrigger::UserRequest);

        Ok(Commit::new(self.parent, state, message, author, trigger))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn test_snapshot() -> StateSnapshot {
        let mut data = BTreeMap::new();
        data.insert("key".to_string(), serde_json::json!("value"));
        StateSnapshot::new(1, data)
    }

    #[test]
    fn test_commit_creation() {
        let state = test_snapshot();
        let commit = Commit::new(
            None,
            state,
            "Initial commit".to_string(),
            AgentId::system(),
            CommitTrigger::UserRequest,
        );

        assert!(commit.is_root());
        assert!(commit.parent.is_none());
    }

    #[test]
    fn test_commit_with_parent() {
        let parent_commit = Commit::new(
            None,
            test_snapshot(),
            "Parent".to_string(),
            AgentId::system(),
            CommitTrigger::UserRequest,
        );

        let child_commit = Commit::new(
            Some(parent_commit.id),
            test_snapshot(),
            "Child".to_string(),
            AgentId::system(),
            CommitTrigger::AutoSave,
        );

        assert!(!child_commit.is_root());
        assert_eq!(child_commit.parent, Some(parent_commit.id));
    }

    #[test]
    fn test_commit_builder() {
        let commit = CommitBuilder::new()
            .state(test_snapshot())
            .message("Test commit")
            .author(AgentId::system())
            .trigger(CommitTrigger::AutoSave)
            .build()
            .unwrap();

        assert_eq!(commit.message, "Test commit");
    }

    #[test]
    fn test_commit_builder_missing_field() {
        let result = CommitBuilder::new()
            .message("Test")
            .author(AgentId::system())
            .build();

        assert!(result.is_err());
    }
}
