use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::id::{AgentId, CommitId, TxId};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Event {
    AgentSpawned {
        id: AgentId,
        role: String,
        config: serde_json::Value,
    },
    AgentTerminated {
        id: AgentId,
        reason: TerminationReason,
    },
    AgentHeartbeat {
        id: AgentId,
        status: AgentStatus,
    },
    AgentThinking {
        id: AgentId,
        content: String,
    },
    AgentAction {
        id: AgentId,
        action: Action,
    },
    MessageSent {
        from: AgentId,
        to: AgentId,
        content: String,
        channel: Option<String>,
    },
    MessageBroadcast {
        from: AgentId,
        content: String,
        channel: String,
    },
    StateUpdated {
        key: String,
        old: Option<serde_json::Value>,
        new: serde_json::Value,
        author: AgentId,
    },
    ToolCallStart {
        agent: AgentId,
        tool: String,
        args: serde_json::Value,
    },
    ToolCallComplete {
        agent: AgentId,
        tool: String,
        result: serde_json::Value,
        duration_ms: u64,
    },
    ToolCallError {
        agent: AgentId,
        tool: String,
        error: String,
    },
    BranchCreated {
        name: String,
        from: String,
        from_commit: CommitId,
    },
    BranchDeleted {
        name: String,
    },
    CommitCreated {
        id: CommitId,
        message: String,
        author: AgentId,
    },
    MergeCompleted {
        from: String,
        into: String,
        result: MergeResult,
    },
    ResetCompleted {
        to: CommitId,
        mode: super::branch::ResetMode,
    },
    LockAcquired {
        resource: String,
        holder: AgentId,
        mode: LockMode,
    },
    LockReleased {
        resource: String,
        holder: AgentId,
    },
    TransactionStarted {
        tx_id: TxId,
        initiator: AgentId,
    },
    TransactionCommitted {
        tx_id: TxId,
    },
    TransactionRolledBack {
        tx_id: TxId,
        reason: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TerminationReason {
    Completed,
    Stopped,
    Error,
    Timeout,
    Killed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentStatus {
    Idle,
    Thinking,
    ExecutingTool,
    Waiting,
    Paused,
    Stopped,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Action {
    ToolCall {
        name: String,
        args: serde_json::Value,
    },
    MessageSend {
        to: AgentId,
        content: String,
    },
    StateWrite {
        key: String,
        value: serde_json::Value,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MergeResult {
    Success,
    Conflict { conflicts: u32 },
    Aborted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LockMode {
    Shared,
    Exclusive,
}

#[derive(Debug, Clone)]
pub struct EventEntry {
    pub index: usize,
    pub timestamp: DateTime<Utc>,
    pub event: Event,
}

#[derive(Debug, Clone, Default)]
pub struct EventFilter {
    pub agent: Option<AgentId>,
    pub event_type: Option<String>,
    pub since: Option<DateTime<Utc>>,
    pub until: Option<DateTime<Utc>>,
    pub limit: Option<usize>,
}

pub struct EventLog {
    events: Vec<EventEntry>,
    agent_index: HashMap<AgentId, Vec<usize>>,
    type_index: HashMap<String, Vec<usize>>,
}

impl EventLog {
    pub fn new() -> Self {
        Self {
            events: Vec::new(),
            agent_index: HashMap::new(),
            type_index: HashMap::new(),
        }
    }

    pub fn append(&mut self, event: Event) -> usize {
        let index = self.events.len();
        let timestamp = Utc::now();

        if let Some(agent_id) = Self::extract_agent_id(&event) {
            self.agent_index.entry(agent_id).or_default().push(index);
        }

        let event_type = Self::event_type(&event);
        self.type_index.entry(event_type).or_default().push(index);

        self.events.push(EventEntry {
            index,
            timestamp,
            event,
        });

        index
    }

    pub fn len(&self) -> usize {
        self.events.len()
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    pub fn get(&self, index: usize) -> Option<&EventEntry> {
        self.events.get(index)
    }

    pub fn query(&self, filter: EventFilter) -> Vec<&EventEntry> {
        let mut results: Vec<&EventEntry> = if let Some(agent_id) = &filter.agent {
            self.agent_index
                .get(agent_id)
                .map(|indices| indices.iter().filter_map(|i| self.events.get(*i)).collect())
                .unwrap_or_default()
        } else {
            self.events.iter().collect()
        };

        if let Some(since) = filter.since {
            results.retain(|e| e.timestamp >= since);
        }

        if let Some(until) = filter.until {
            results.retain(|e| e.timestamp <= until);
        }

        if let Some(event_type) = &filter.event_type {
            results.retain(|e| Self::event_type(&e.event) == *event_type);
        }

        if let Some(limit) = filter.limit {
            results.truncate(limit);
        }

        results
    }

    pub fn replay_from(&self, index: usize) -> Vec<&Event> {
        self.events.iter().skip(index).map(|e| &e.event).collect()
    }

    pub fn last(&self) -> Option<&EventEntry> {
        self.events.last()
    }

    pub fn all(&self) -> &[EventEntry] {
        &self.events
    }

    fn extract_agent_id(event: &Event) -> Option<AgentId> {
        match event {
            Event::AgentSpawned { id, .. } => Some(*id),
            Event::AgentTerminated { id, .. } => Some(*id),
            Event::AgentHeartbeat { id, .. } => Some(*id),
            Event::AgentThinking { id, .. } => Some(*id),
            Event::AgentAction { id, .. } => Some(*id),
            Event::MessageSent { from, .. } => Some(*from),
            Event::MessageBroadcast { from, .. } => Some(*from),
            Event::StateUpdated { author, .. } => Some(*author),
            Event::ToolCallStart { agent, .. } => Some(*agent),
            Event::ToolCallComplete { agent, .. } => Some(*agent),
            Event::ToolCallError { agent, .. } => Some(*agent),
            Event::CommitCreated { author, .. } => Some(*author),
            Event::LockAcquired { holder, .. } => Some(*holder),
            Event::LockReleased { holder, .. } => Some(*holder),
            Event::TransactionStarted { initiator, .. } => Some(*initiator),
            _ => None,
        }
    }

    fn event_type(event: &Event) -> String {
        match event {
            Event::AgentSpawned { .. } => "AgentSpawned".to_string(),
            Event::AgentTerminated { .. } => "AgentTerminated".to_string(),
            Event::AgentHeartbeat { .. } => "AgentHeartbeat".to_string(),
            Event::AgentThinking { .. } => "AgentThinking".to_string(),
            Event::AgentAction { .. } => "AgentAction".to_string(),
            Event::MessageSent { .. } => "MessageSent".to_string(),
            Event::MessageBroadcast { .. } => "MessageBroadcast".to_string(),
            Event::StateUpdated { .. } => "StateUpdated".to_string(),
            Event::ToolCallStart { .. } => "ToolCallStart".to_string(),
            Event::ToolCallComplete { .. } => "ToolCallComplete".to_string(),
            Event::ToolCallError { .. } => "ToolCallError".to_string(),
            Event::BranchCreated { .. } => "BranchCreated".to_string(),
            Event::BranchDeleted { .. } => "BranchDeleted".to_string(),
            Event::CommitCreated { .. } => "CommitCreated".to_string(),
            Event::MergeCompleted { .. } => "MergeCompleted".to_string(),
            Event::ResetCompleted { .. } => "ResetCompleted".to_string(),
            Event::LockAcquired { .. } => "LockAcquired".to_string(),
            Event::LockReleased { .. } => "LockReleased".to_string(),
            Event::TransactionStarted { .. } => "TransactionStarted".to_string(),
            Event::TransactionCommitted { .. } => "TransactionCommitted".to_string(),
            Event::TransactionRolledBack { .. } => "TransactionRolledBack".to_string(),
        }
    }
}

impl Default for EventLog {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_log_append() {
        let mut log = EventLog::new();
        let agent_id = AgentId::new();

        let idx = log.append(Event::AgentSpawned {
            id: agent_id,
            role: "test".to_string(),
            config: serde_json::json!({"provider": "anthropic"}),
        });

        assert_eq!(idx, 0);
        assert_eq!(log.len(), 1);
    }

    #[test]
    fn test_event_log_query_by_agent() {
        let mut log = EventLog::new();
        let agent_id = AgentId::new();

        log.append(Event::AgentSpawned {
            id: agent_id,
            role: "test".to_string(),
            config: serde_json::json!({}),
        });

        log.append(Event::AgentHeartbeat {
            id: agent_id,
            status: AgentStatus::Idle,
        });

        let other_agent = AgentId::new();
        log.append(Event::AgentSpawned {
            id: other_agent,
            role: "other".to_string(),
            config: serde_json::json!({}),
        });

        let filter = EventFilter {
            agent: Some(agent_id),
            ..Default::default()
        };

        let results = log.query(filter);
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_event_log_replay() {
        let mut log = EventLog::new();

        log.append(Event::BranchCreated {
            name: "feature".to_string(),
            from: "main".to_string(),
            from_commit: CommitId::new(),
        });

        log.append(Event::CommitCreated {
            id: CommitId::new(),
            message: "test".to_string(),
            author: AgentId::system(),
        });

        let events = log.replay_from(1);
        assert_eq!(events.len(), 1);
    }
}
