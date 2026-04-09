// @amadeus-header
// summary: Worker agent execution and task lifecycle model types for runtime coordination.
// layer: core
// status: active
// feature_flags: none
// provides:
// - module: crate::worker
// - type: crate::worker::WorkerConfig
// - type: crate::worker::Task
// - type: crate::worker::HelpRequest
// - type: crate::worker::TaskResult
// - type: crate::worker::WorkerStatus
// - type: crate::worker::WorkerInfo
// uses:
// - module: amadeus_events
// - module: amadeus_ids
// - runtime: tokio async channels
// - protocol: serde serialization
// invariants:
// - Worker model semantics stay stable across transports.
// side_effects:
// - Sends or receives messages across async channels.
// tests:
// - cmd: cargo test -p runtime
// @end-amadeus-header

use std::collections::HashMap;

use amadeus_events::ToolCall;
use amadeus_ids::AgentId;
use serde::{Deserialize, Serialize};
use tokio::sync::oneshot;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerConfig {
    pub id: Option<AgentId>,
    pub name: String,
    pub capabilities: Vec<String>,
    pub max_concurrent: usize,
    pub model: Option<String>,
}

impl WorkerConfig {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: None,
            name: name.into(),
            capabilities: Vec::new(),
            max_concurrent: 1,
            model: None,
        }
    }

    pub fn id(mut self, id: Option<AgentId>) -> Self {
        self.id = id;
        self
    }

    pub fn capability(mut self, cap: impl Into<String>) -> Self {
        self.capabilities.push(cap.into());
        self
    }

    pub fn max_concurrent(mut self, max: usize) -> Self {
        self.max_concurrent = max;
        self
    }

    pub fn model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub prompt: String,
    pub required_capabilities: Vec<String>,
    pub priority: u8,
    pub metadata: HashMap<String, serde_json::Value>,
}

impl Task {
    pub fn new(id: impl Into<String>, prompt: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            prompt: prompt.into(),
            required_capabilities: Vec::new(),
            priority: 0,
            metadata: HashMap::new(),
        }
    }

    pub fn requires(mut self, caps: Vec<String>) -> Self {
        self.required_capabilities.extend(caps);
        self
    }

    pub fn priority(mut self, priority: u8) -> Self {
        self.priority = priority;
        self
    }

    pub fn meta(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }
}

#[derive(Debug)]
pub struct HelpRequest {
    pub task: Task,
    pub response_tx: oneshot::Sender<TaskResult>,
    pub requester_id: AgentId,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResult {
    pub task_id: String,
    pub worker_id: AgentId,
    pub success: bool,
    pub output: Option<String>,
    pub error: Option<String>,
    pub duration_ms: u64,
    pub tool_calls: Vec<ToolCall>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorkerStatus {
    Idle,
    Busy,
    Error,
    Offline,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerInfo {
    pub id: AgentId,
    pub name: String,
    pub capabilities: Vec<String>,
    pub status: WorkerStatus,
    pub active_tasks: usize,
    pub max_concurrent: usize,
    pub completed_tasks: usize,
    pub total_errors: usize,
}

impl WorkerInfo {
    pub fn has_capabilities(&self, required: &[String]) -> bool {
        required.iter().all(|cap| self.capabilities.contains(cap))
    }

    pub fn is_available(&self) -> bool {
        matches!(self.status, WorkerStatus::Idle | WorkerStatus::Busy)
            && self.active_tasks < self.max_concurrent
    }
}
