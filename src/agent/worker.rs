//! # Worker Types
//!
//! Types for worker agents in a supervisor pattern.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::agent::events::ToolCall;
use crate::core::id::AgentId;

/// Configuration for a worker agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerConfig {
    /// Optional explicit ID (generated if None).
    pub id: Option<AgentId>,
    /// Human-readable name for the worker.
    pub name: String,
    /// Capabilities as tags (e.g., ["code", "review", "test"]).
    pub capabilities: Vec<String>,
    /// Maximum concurrent tasks this worker can handle.
    pub max_concurrent: usize,
    /// Optional model override for this worker.
    pub model: Option<String>,
}

impl WorkerConfig {
    /// Create a new worker config with the given name.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: None,
            name: name.into(),
            capabilities: Vec::new(),
            max_concurrent: 1,
            model: None,
        }
    }

    /// Add a capability tag.
    pub fn capability(mut self, cap: impl Into<String>) -> Self {
        self.capabilities.push(cap.into());
        self
    }

    /// Set maximum concurrent tasks.
    pub fn max_concurrent(mut self, max: usize) -> Self {
        self.max_concurrent = max;
        self
    }

    /// Set model override.
    pub fn model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }
}

/// A task to be dispatched to a worker.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    /// Unique task identifier.
    pub id: String,
    /// The prompt/instruction for the task.
    pub prompt: String,
    /// Required capabilities (matched against worker.capabilities).
    pub required_capabilities: Vec<String>,
    /// Priority (0-255, higher = more urgent).
    pub priority: u8,
    /// Additional metadata.
    pub metadata: HashMap<String, serde_json::Value>,
}

impl Task {
    /// Create a new task.
    pub fn new(id: impl Into<String>, prompt: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            prompt: prompt.into(),
            required_capabilities: Vec::new(),
            priority: 0,
            metadata: HashMap::new(),
        }
    }

    /// Add a required capability.
    pub fn requires(mut self, cap: impl Into<String>) -> Self {
        self.required_capabilities.push(cap.into());
        self
    }

    /// Set priority.
    pub fn priority(mut self, priority: u8) -> Self {
        self.priority = priority;
        self
    }

    /// Add metadata.
    pub fn meta(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }
}

/// Result of task execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResult {
    /// Task ID that was executed.
    pub task_id: String,
    /// Worker that executed the task.
    pub worker_id: AgentId,
    /// Whether execution succeeded.
    pub success: bool,
    /// Output text (if successful).
    pub output: Option<String>,
    /// Error message (if failed).
    pub error: Option<String>,
    /// Execution duration in milliseconds.
    pub duration_ms: u64,
    /// Tool calls made during execution.
    pub tool_calls: Vec<ToolCall>,
}

/// Runtime status of a worker.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorkerStatus {
    /// Worker is available for tasks.
    Idle,
    /// Worker is executing tasks.
    Busy,
    /// Worker encountered an error.
    Error,
    /// Worker is offline/unavailable.
    Offline,
}

/// Runtime information about a worker.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerInfo {
    /// Worker ID.
    pub id: AgentId,
    /// Worker name.
    pub name: String,
    /// Worker capabilities.
    pub capabilities: Vec<String>,
    /// Current status.
    pub status: WorkerStatus,
    /// Number of active tasks.
    pub active_tasks: usize,
    /// Maximum concurrent tasks.
    pub max_concurrent: usize,
    /// Total completed tasks.
    pub completed_tasks: usize,
    /// Total errors encountered.
    pub total_errors: usize,
}

impl WorkerInfo {
    /// Check if worker has all required capabilities.
    pub fn has_capabilities(&self, required: &[String]) -> bool {
        required.iter().all(|cap| self.capabilities.contains(cap))
    }

    /// Check if worker can accept more tasks.
    pub fn is_available(&self) -> bool {
        matches!(self.status, WorkerStatus::Idle | WorkerStatus::Busy)
            && self.active_tasks < self.max_concurrent
    }
}
