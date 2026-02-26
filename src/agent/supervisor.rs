//! # Supervisor Pattern
//!
//! Manages a pool of worker agents with configurable dispatch strategies.
//!
//! ## Dispatch Strategies
//!
//! - **RoundRobin**: Rotate through workers in order.
//! - **LeastLoaded**: Pick worker with fewest active tasks.
//! - **CapabilityMatch**: Match task requirements to worker capabilities.
//!
//! ## Example
//!
//! ```rust,ignore
//! use amadeus::{Supervisor, SupervisorConfig, DispatchStrategy, WorkerConfig, Task};
//!
//! let config = SupervisorConfig {
//!     strategy: DispatchStrategy::CapabilityMatch,
//!     ..Default::default()
//! };
//!
//! let mut supervisor = Supervisor::new(client, config);
//!
//! supervisor.spawn(vec![
//!     WorkerConfig::new("coder").capability("code").capability("refactor"),
//!     WorkerConfig::new("reviewer").capability("review").capability("test"),
//! ]).await?;
//!
//! let task = Task::new("task-1", "Review the auth module")
//!     .requires("review");
//!
//! let result = supervisor.execute(task).await?;
//! ```

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{Mutex, RwLock};
use tracing::{debug, info, warn};

use crate::agent::config::Config;
use crate::agent::loop_agent::Agent;
use crate::client::LLMClient;
use crate::concurrency::LockManager;
use crate::core::id::AgentId;
use crate::error::Result;

use super::worker::{Task, TaskResult, WorkerConfig, WorkerInfo, WorkerStatus};

/// Strategy for dispatching tasks to workers.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum DispatchStrategy {
    /// Rotate through workers in order.
    #[default]
    RoundRobin,
    /// Pick worker with fewest active tasks.
    LeastLoaded,
    /// Match task requirements to worker capabilities.
    CapabilityMatch,
}

/// Configuration for the supervisor.
#[derive(Debug, Clone)]
pub struct SupervisorConfig {
    /// Dispatch strategy.
    pub strategy: DispatchStrategy,
    /// Maximum pending tasks in queue.
    pub max_pending_tasks: usize,
    /// Task execution timeout.
    pub task_timeout: Duration,
    /// Whether to retry failed tasks.
    pub retry_failed_tasks: bool,
    /// Maximum retry attempts.
    pub max_retries: u8,
}

impl Default for SupervisorConfig {
    fn default() -> Self {
        Self {
            strategy: DispatchStrategy::default(),
            max_pending_tasks: 100,
            task_timeout: Duration::from_secs(300),
            retry_failed_tasks: true,
            max_retries: 3,
        }
    }
}

/// Internal worker entry.
struct WorkerEntry<C: LLMClient> {
    info: WorkerInfo,
    agent: Agent<C>,
    active_tasks: HashMap<String, tokio::task::JoinHandle<Result<TaskResult>>>,
}

/// Supervisor manages a pool of worker agents.
///
/// Provides:
/// - Worker lifecycle management
/// - Task dispatch with multiple strategies
/// - Resource locking via LockManager
pub struct Supervisor<C: LLMClient> {
    client: C,
    config: SupervisorConfig,
    sdk_config: Arc<Config>,
    workers: HashMap<AgentId, WorkerEntry<C>>,
    task_queue: VecDeque<Task>,
    lock_manager: Arc<Mutex<LockManager>>,
    next_worker_idx: usize,
}

impl<C: LLMClient + Clone + 'static> Supervisor<C> {
    /// Create a new supervisor.
    pub fn new(client: C, config: SupervisorConfig, sdk_config: Arc<Config>) -> Self {
        Self {
            client,
            config,
            sdk_config,
            workers: HashMap::new(),
            task_queue: VecDeque::new(),
            lock_manager: Arc::new(Mutex::new(LockManager::new())),
            next_worker_idx: 0,
        }
    }

    /// Get the lock manager for resource coordination.
    pub fn lock_manager(&self) -> Arc<Mutex<LockManager>> {
        Arc::clone(&self.lock_manager)
    }

    /// Spawn worker agents.
    ///
    /// Returns the IDs of the spawned workers.
    pub async fn spawn(&mut self, configs: Vec<WorkerConfig>) -> Result<Vec<AgentId>> {
        let mut ids = Vec::new();

        for worker_config in configs {
            let id = worker_config.id.unwrap_or_else(AgentId::new);

            let worker_sdk_config = if let Some(model) = worker_config.model {
                let mut cfg = (*self.sdk_config).clone();
                cfg.model = model;
                Arc::new(cfg)
            } else {
                Arc::clone(&self.sdk_config)
            };

            let agent = Agent::new(self.client.clone(), worker_sdk_config);

            let info = WorkerInfo {
                id,
                name: worker_config.name,
                capabilities: worker_config.capabilities,
                status: WorkerStatus::Idle,
                active_tasks: 0,
                max_concurrent: worker_config.max_concurrent,
                completed_tasks: 0,
                total_errors: 0,
            };

            self.workers.insert(
                id,
                WorkerEntry {
                    info,
                    agent,
                    active_tasks: HashMap::new(),
                },
            );

            ids.push(id);
        }

        info!(workers = ids.len(), "Workers spawned");
        Ok(ids)
    }

    /// Get worker info.
    pub fn worker(&self, id: AgentId) -> Option<&WorkerInfo> {
        self.workers.get(&id).map(|w| &w.info)
    }

    /// Get all worker infos.
    pub fn workers(&self) -> Vec<&WorkerInfo> {
        self.workers.values().map(|w| &w.info).collect()
    }

    /// Get count of workers.
    pub fn worker_count(&self) -> usize {
        self.workers.len()
    }

    /// Submit a task to the queue (non-blocking).
    pub fn submit(&mut self, task: Task) -> Result<()> {
        if self.task_queue.len() >= self.config.max_pending_tasks {
            return Err(crate::error::AgentError::Config(
                "Task queue is full".to_string(),
            ));
        }

        self.task_queue.push_back(task);
        Ok(())
    }

    /// Execute a task immediately (blocking).
    ///
    /// Dispatches to an available worker and waits for result.
    pub async fn execute(&mut self, task: Task) -> Result<TaskResult> {
        let worker_id = self.select_worker(&task)?;

        self.dispatch_to_worker(worker_id, task).await
    }

    /// Process pending tasks.
    ///
    /// Returns count of tasks dispatched.
    pub async fn process_pending(&mut self) -> Result<usize> {
        let mut dispatched = 0;

        while let Some(task) = self.task_queue.pop_front() {
            if let Ok(worker_id) = self.select_worker(&task) {
                let task_id = task.id.clone();
                let result = self.dispatch_to_worker(worker_id, task).await;

                match result {
                    Ok(r) => {
                        if r.success {
                            dispatched += 1;
                        }
                    }
                    Err(e) => {
                        warn!(task_id = %task_id, error = %e, "Task failed");
                    }
                }
            } else {
                self.task_queue.push_front(task);
                break;
            }
        }

        Ok(dispatched)
    }

    /// Get pending task count.
    pub fn pending_count(&self) -> usize {
        self.task_queue.len()
    }

    /// Shutdown all workers.
    ///
    /// Cancels active tasks and clears the queue.
    pub async fn shutdown(&mut self) {
        for (_, worker) in self.workers.iter_mut() {
            for (_, handle) in worker.active_tasks.drain() {
                handle.abort();
            }
            worker.info.status = WorkerStatus::Offline;
        }
        self.task_queue.clear();
        info!("Supervisor shutdown complete");
    }

    /// Select a worker for a task based on strategy.
    fn select_worker(&mut self, task: &Task) -> Result<AgentId> {
        let worker_id = match self.config.strategy {
            DispatchStrategy::RoundRobin => self.select_round_robin(),
            DispatchStrategy::LeastLoaded => self.select_least_loaded(),
            DispatchStrategy::CapabilityMatch => self.select_by_capability(task),
        };

        worker_id.ok_or_else(|| crate::error::AgentError::Config("No available worker".to_string()))
    }

    /// Round-robin selection.
    fn select_round_robin(&mut self) -> Option<AgentId> {
        let available: Vec<_> = self
            .workers
            .iter()
            .filter(|(_, w)| w.info.is_available())
            .collect();

        if available.is_empty() {
            return None;
        }

        let idx = self.next_worker_idx % available.len();
        self.next_worker_idx += 1;
        Some(*available[idx].0)
    }

    /// Least-loaded selection.
    fn select_least_loaded(&self) -> Option<AgentId> {
        self.workers
            .iter()
            .filter(|(_, w)| w.info.is_available())
            .min_by_key(|(_, w)| w.info.active_tasks)
            .map(|(id, _)| *id)
    }

    /// Capability-based selection.
    fn select_by_capability(&self, task: &Task) -> Option<AgentId> {
        self.workers
            .iter()
            .filter(|(_, w)| {
                w.info.is_available() && w.info.has_capabilities(&task.required_capabilities)
            })
            .min_by_key(|(_, w)| w.info.active_tasks)
            .map(|(id, _)| *id)
    }

    /// Dispatch a task to a specific worker.
    async fn dispatch_to_worker(&mut self, worker_id: AgentId, task: Task) -> Result<TaskResult> {
        let worker = self
            .workers
            .get_mut(&worker_id)
            .ok_or_else(|| crate::error::AgentError::Config("Worker not found".to_string()))?;

        let task_id = task.id.clone();
        let prompt = task.prompt.clone();

        worker.info.status = WorkerStatus::Busy;
        worker.info.active_tasks += 1;

        let history = Arc::new(RwLock::new(Vec::new()));
        let agent = worker.agent.clone();
        let task_timeout = self.config.task_timeout;

        debug!(worker_id = %worker_id, task_id = %task_id, "Dispatching task");

        let handle = tokio::spawn(async move {
            let result = tokio::time::timeout(task_timeout, agent.run(&prompt, history)).await;

            match result {
                Ok(Ok(run_result)) => TaskResult {
                    task_id,
                    worker_id,
                    success: true,
                    output: Some(run_result.text),
                    error: None,
                    duration_ms: 0,
                    tool_calls: run_result.tool_calls,
                },
                Ok(Err(e)) => TaskResult {
                    task_id,
                    worker_id,
                    success: false,
                    output: None,
                    error: Some(e.to_string()),
                    duration_ms: 0,
                    tool_calls: Vec::new(),
                },
                Err(_) => TaskResult {
                    task_id,
                    worker_id,
                    success: false,
                    output: None,
                    error: Some("Task timed out".to_string()),
                    duration_ms: 0,
                    tool_calls: Vec::new(),
                },
            }
        });

        let result = handle.await?;

        let worker = self.workers.get_mut(&worker_id).unwrap();
        worker.info.active_tasks = worker.info.active_tasks.saturating_sub(1);

        if result.success {
            worker.info.completed_tasks += 1;
        } else {
            worker.info.total_errors += 1;
        }

        worker.info.status = if worker.info.active_tasks > 0 {
            WorkerStatus::Busy
        } else {
            WorkerStatus::Idle
        };

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dispatch_strategy_default() {
        let strategy = DispatchStrategy::default();
        assert!(matches!(strategy, DispatchStrategy::RoundRobin));
    }

    #[test]
    fn test_supervisor_config_default() {
        let config = SupervisorConfig::default();
        assert_eq!(config.max_pending_tasks, 100);
        assert!(config.retry_failed_tasks);
        assert_eq!(config.max_retries, 3);
    }

    #[test]
    fn test_worker_config_builder() {
        let config = WorkerConfig::new("test-worker")
            .capability("code")
            .capability("review")
            .max_concurrent(4);

        assert_eq!(config.name, "test-worker");
        assert_eq!(config.capabilities, vec!["code", "review"]);
        assert_eq!(config.max_concurrent, 4);
    }

    #[test]
    fn test_task_builder() {
        let task = Task::new("task-1", "Do something")
            .requires("code")
            .priority(10);

        assert_eq!(task.id, "task-1");
        assert_eq!(task.prompt, "Do something");
        assert_eq!(task.required_capabilities, vec!["code"]);
        assert_eq!(task.priority, 10);
    }

    #[test]
    fn test_worker_info_has_capabilities() {
        let info = WorkerInfo {
            id: AgentId::new(),
            name: "test".to_string(),
            capabilities: vec!["code".to_string(), "review".to_string()],
            status: WorkerStatus::Idle,
            active_tasks: 0,
            max_concurrent: 1,
            completed_tasks: 0,
            total_errors: 0,
        };

        assert!(info.has_capabilities(&["code".to_string()]));
        assert!(info.has_capabilities(&["code".to_string(), "review".to_string()]));
        assert!(!info.has_capabilities(&["test".to_string()]));
    }

    #[test]
    fn test_worker_info_is_available() {
        let mut info = WorkerInfo {
            id: AgentId::new(),
            name: "test".to_string(),
            capabilities: vec![],
            status: WorkerStatus::Idle,
            active_tasks: 0,
            max_concurrent: 2,
            completed_tasks: 0,
            total_errors: 0,
        };

        assert!(info.is_available());

        info.active_tasks = 2;
        assert!(!info.is_available());

        info.status = WorkerStatus::Error;
        info.active_tasks = 0;
        assert!(!info.is_available());
    }
}
