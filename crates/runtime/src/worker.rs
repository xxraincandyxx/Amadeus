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
// - function: crate::worker::mark_worker_task_started
// - function: crate::worker::finalize_worker_task
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

pub fn mark_worker_task_started(info: &mut WorkerInfo) {
    info.status = WorkerStatus::Busy;
    info.active_tasks += 1;
}

pub fn finalize_worker_task(
    info: &mut WorkerInfo,
    task_id: String,
    worker_id: AgentId,
    outcome: std::result::Result<RunOutcome, String>,
) -> TaskResult {
    info.active_tasks = info.active_tasks.saturating_sub(1);

    let task_result = match outcome {
        Ok(run_outcome) => {
            info.completed_tasks += 1;
            TaskResult {
                task_id,
                worker_id,
                success: true,
                output: Some(run_outcome.text),
                error: None,
                duration_ms: run_outcome.duration_ms,
                tool_calls: run_outcome.tool_calls,
            }
        }
        Err(error) => {
            info.total_errors += 1;
            TaskResult {
                task_id,
                worker_id,
                success: false,
                output: None,
                error: Some(error),
                duration_ms: 0,
                tool_calls: Vec::new(),
            }
        }
    };

    info.status = if info.active_tasks > 0 {
        WorkerStatus::Busy
    } else {
        WorkerStatus::Idle
    };

    task_result
}

#[derive(Debug, Clone)]
pub struct RunOutcome {
    pub text: String,
    pub duration_ms: u64,
    pub tool_calls: Vec<ToolCall>,
}

#[cfg(test)]
mod tests {
    use amadeus_ids::AgentId;

    use super::{
        finalize_worker_task, mark_worker_task_started, RunOutcome, WorkerInfo, WorkerStatus,
    };

    fn worker_info() -> WorkerInfo {
        WorkerInfo {
            id: AgentId::new(),
            name: "worker".to_string(),
            capabilities: Vec::new(),
            status: WorkerStatus::Idle,
            active_tasks: 0,
            max_concurrent: 2,
            completed_tasks: 0,
            total_errors: 0,
        }
    }

    #[test]
    fn mark_worker_task_started_sets_busy_state() {
        let mut info = worker_info();

        mark_worker_task_started(&mut info);

        assert_eq!(info.status, WorkerStatus::Busy);
        assert_eq!(info.active_tasks, 1);
    }

    #[test]
    fn finalize_worker_task_records_success() {
        let worker_id = AgentId::new();
        let mut info = worker_info();
        mark_worker_task_started(&mut info);

        let result = finalize_worker_task(
            &mut info,
            "task-1".to_string(),
            worker_id,
            Ok(RunOutcome {
                text: "done".to_string(),
                duration_ms: 42,
                tool_calls: Vec::new(),
            }),
        );

        assert!(result.success);
        assert_eq!(result.output.as_deref(), Some("done"));
        assert_eq!(info.completed_tasks, 1);
        assert_eq!(info.status, WorkerStatus::Idle);
    }

    #[test]
    fn finalize_worker_task_records_failure() {
        let worker_id = AgentId::new();
        let mut info = worker_info();
        mark_worker_task_started(&mut info);

        let result = finalize_worker_task(
            &mut info,
            "task-1".to_string(),
            worker_id,
            Err("boom".to_string()),
        );

        assert!(!result.success);
        assert_eq!(result.error.as_deref(), Some("boom"));
        assert_eq!(info.total_errors, 1);
        assert_eq!(info.status, WorkerStatus::Idle);
    }
}
