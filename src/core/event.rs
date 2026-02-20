//! Event types for the SDK

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::id::AgentId;

/// A log entry for events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventEntry {
    pub timestamp: DateTime<Utc>,
    pub event: Event,
}

/// Core SDK events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Event {
    AgentSpawned {
        id: AgentId,
        role: String,
    },
    AgentTerminated {
        id: AgentId,
        reason: String,
    },
    AgentThinking {
        id: AgentId,
        content: String,
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
    MessageSent {
        from: AgentId,
        to: AgentId,
        content: String,
    },
}

impl Event {
    pub fn timestamp(&self) -> DateTime<Utc> {
        Utc::now()
    }
    
    pub fn to_entry(&self) -> EventEntry {
        EventEntry {
            timestamp: self.timestamp(),
            event: self.clone(),
        }
    }
}
