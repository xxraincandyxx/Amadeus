// @amadeus-header
// summary: Versioned append-only transcript model for durable agent session events.
// layer: agent
// status: active
// feature_flags: none
// provides:
// - module: crate::transcript
// - type: crate::transcript::TranscriptLog
// - type: crate::transcript::TranscriptEvent
// - type: crate::transcript::TranscriptStore
// uses:
// - module: crate::agent::events
// - protocol: serde serialization
// - format: JSON values
// invariants:
// - Transcript logs remain append-only and compatible with session summaries.
// side_effects:
// - Reads or writes filesystem state when using TranscriptStore.
// tests:
// - cmd: cargo test -p core transcript --features full
// @end-amadeus-header

//! Durable transcript primitives for agent sessions.

use std::path::{Path, PathBuf};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{AgentError, Result};

pub const TRANSCRIPT_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TranscriptLog {
    pub version: u32,
    pub session_id: String,
    pub events: Vec<TranscriptEvent>,
}

impl TranscriptLog {
    pub fn new(session_id: impl Into<String>) -> Self {
        Self {
            version: TRANSCRIPT_VERSION,
            session_id: session_id.into(),
            events: Vec::new(),
        }
    }

    pub fn append(&mut self, event: TranscriptEvent) {
        self.events.push(event);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TranscriptEvent {
    pub timestamp: String,
    pub kind: TranscriptEventKind,
    pub payload: Value,
}

impl TranscriptEvent {
    pub fn new(kind: TranscriptEventKind, payload: Value) -> Self {
        Self {
            timestamp: Utc::now().to_rfc3339(),
            kind,
            payload,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TranscriptEventKind {
    TurnStarted,
    ProviderEvent,
    ToolCall,
    Approval,
    PermissionDecision,
    McpEvent,
    Compaction,
    Error,
    TurnCompleted,
}

#[derive(Debug, Clone)]
pub struct TranscriptStore {
    dir: PathBuf,
}

impl TranscriptStore {
    pub fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    pub fn save(&self, log: &TranscriptLog) -> Result<PathBuf> {
        std::fs::create_dir_all(&self.dir).map_err(AgentError::Io)?;
        let path = self.dir.join(format!("{}.transcript.json", log.session_id));
        let json = serde_json::to_vec_pretty(log).map_err(AgentError::Serde)?;
        std::fs::write(&path, json).map_err(AgentError::Io)?;
        Ok(path)
    }

    pub fn load(path: &Path) -> Result<TranscriptLog> {
        let content = std::fs::read_to_string(path).map_err(AgentError::Io)?;
        serde_json::from_str(&content).map_err(AgentError::Serde)
    }
}

#[cfg(test)]
mod tests {
    use super::{TranscriptEvent, TranscriptEventKind, TranscriptLog, TranscriptStore};

    #[test]
    fn transcript_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let mut log = TranscriptLog::new("session-1");
        log.append(TranscriptEvent::new(
            TranscriptEventKind::TurnStarted,
            serde_json::json!({"prompt":"hello"}),
        ));

        let path = TranscriptStore::new(dir.path().to_path_buf())
            .save(&log)
            .unwrap();
        let loaded = TranscriptStore::load(&path).unwrap();

        assert_eq!(loaded, log);
    }
}
