// @amadeus-header
// summary: Structured telemetry event recording with memory and JSONL sinks.
// layer: core
// status: active
// feature_flags: none
// provides:
// - module: crate
// - type: crate::TelemetryEntry
// - type: crate::TelemetryEvent
// - type: crate::TelemetryRecorder
// - type: crate::TelemetrySink
// - type: crate::MemorySink
// - type: crate::JsonlSink
// - type: crate::TelemetryError
// uses:
// - module: amadeus_ids
// - protocol: serde serialization
// - format: JSONL
// invariants:
// - Telemetry events remain append-only, serialization-stable records.
// side_effects:
// - Reads or writes filesystem state.
// tests:
// - cmd: cargo test -p telemetry
// @end-amadeus-header

use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use amadeus_ids::AgentId;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryEntry {
    pub timestamp: DateTime<Utc>,
    pub event: TelemetryEvent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TelemetryEvent {
    SessionStarted {
        session_id: String,
        model: String,
        prompt: Option<String>,
        subagent_depth: usize,
    },
    SessionCompleted {
        session_id: String,
        duration_ms: u64,
        tool_calls: usize,
        output_len: usize,
    },
    SessionFailed {
        session_id: String,
        error: String,
    },
    PromptSubmitted {
        session_id: String,
        prompt: String,
    },
    ToolStarted {
        session_id: String,
        tool_call_id: String,
        tool: String,
        input: Value,
        parent_id: Option<String>,
    },
    ToolCompleted {
        session_id: String,
        tool_call_id: String,
        tool: String,
        duration_ms: u64,
        is_error: bool,
    },
    ApprovalRequested {
        session_id: String,
        approval_id: String,
        tool: String,
        reason: String,
    },
    ApprovalResolved {
        session_id: String,
        approval_id: String,
        tool: String,
        decision: String,
    },
    WorkerSpawned {
        runtime_id: String,
        worker_id: AgentId,
        name: String,
        capabilities: Vec<String>,
    },
    WorkerStateChanged {
        runtime_id: String,
        worker_id: AgentId,
        state: String,
        active_tasks: usize,
    },
    TaskQueued {
        runtime_id: String,
        task_id: String,
    },
    TaskDispatched {
        runtime_id: String,
        task_id: String,
        worker_id: AgentId,
    },
    TaskCompleted {
        runtime_id: String,
        task_id: String,
        worker_id: AgentId,
        success: bool,
        duration_ms: u64,
    },
}

impl TelemetryEvent {
    pub fn into_entry(self) -> TelemetryEntry {
        TelemetryEntry {
            timestamp: Utc::now(),
            event: self,
        }
    }
}

#[derive(Debug, Error)]
pub enum TelemetryError {
    #[error("telemetry io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("telemetry serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("telemetry sink lock poisoned")]
    LockPoisoned,
}

pub trait TelemetrySink: Send + Sync {
    fn record(&self, entry: &TelemetryEntry) -> Result<(), TelemetryError>;
}

#[derive(Clone, Default)]
pub struct TelemetryRecorder {
    sinks: Vec<Arc<dyn TelemetrySink>>,
}

impl TelemetryRecorder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_sink(mut self, sink: Arc<dyn TelemetrySink>) -> Self {
        self.sinks.push(sink);
        self
    }

    pub fn add_sink(&mut self, sink: Arc<dyn TelemetrySink>) {
        self.sinks.push(sink);
    }

    pub fn record(&self, event: TelemetryEvent) -> Result<TelemetryEntry, TelemetryError> {
        let entry = event.into_entry();
        for sink in &self.sinks {
            sink.record(&entry)?;
        }
        Ok(entry)
    }

    pub fn is_enabled(&self) -> bool {
        !self.sinks.is_empty()
    }
}

#[derive(Default)]
pub struct MemorySink {
    entries: Mutex<Vec<TelemetryEntry>>,
}

impl MemorySink {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn entries(&self) -> Result<Vec<TelemetryEntry>, TelemetryError> {
        self.entries
            .lock()
            .map(|entries| entries.clone())
            .map_err(|_| TelemetryError::LockPoisoned)
    }
}

impl TelemetrySink for MemorySink {
    fn record(&self, entry: &TelemetryEntry) -> Result<(), TelemetryError> {
        self.entries
            .lock()
            .map_err(|_| TelemetryError::LockPoisoned)?
            .push(entry.clone());
        Ok(())
    }
}

pub struct JsonlSink {
    path: PathBuf,
    writer: Mutex<BufWriter<File>>,
}

impl JsonlSink {
    pub fn new(path: impl AsRef<Path>) -> Result<Self, TelemetryError> {
        let path = path.as_ref().to_path_buf();
        let file = OpenOptions::new().create(true).append(true).open(&path)?;
        Ok(Self {
            path,
            writer: Mutex::new(BufWriter::new(file)),
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl TelemetrySink for JsonlSink {
    fn record(&self, entry: &TelemetryEntry) -> Result<(), TelemetryError> {
        let mut writer = self
            .writer
            .lock()
            .map_err(|_| TelemetryError::LockPoisoned)?;
        serde_json::to_writer(&mut *writer, entry)?;
        writer.write_all(b"\n")?;
        writer.flush()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::sync::Arc;

    use tempfile::tempdir;

    use super::{JsonlSink, MemorySink, TelemetryEvent, TelemetryRecorder, TelemetrySink};

    #[test]
    fn memory_sink_records_entries() {
        let sink = Arc::new(MemorySink::new());
        let recorder = TelemetryRecorder::new().with_sink(sink.clone());

        recorder
            .record(TelemetryEvent::PromptSubmitted {
                session_id: "run-1".to_string(),
                prompt: "hello".to_string(),
            })
            .unwrap();

        let entries = sink.entries().unwrap();
        assert_eq!(entries.len(), 1);
        match &entries[0].event {
            TelemetryEvent::PromptSubmitted { prompt, .. } => assert_eq!(prompt, "hello"),
            _ => panic!("unexpected telemetry event"),
        }
    }

    #[test]
    fn jsonl_sink_writes_parseable_lines() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("telemetry.jsonl");
        let sink = JsonlSink::new(&path).unwrap();

        sink.record(
            &TelemetryEvent::SessionStarted {
                session_id: "run-1".to_string(),
                model: "test-model".to_string(),
                prompt: Some("hello".to_string()),
                subagent_depth: 0,
            }
            .into_entry(),
        )
        .unwrap();

        let content = fs::read_to_string(&path).unwrap();
        assert_eq!(content.lines().count(), 1);
        let parsed: serde_json::Value =
            serde_json::from_str(content.lines().next().unwrap()).unwrap();
        assert_eq!(parsed["event"]["type"], "session_started");
    }
}
