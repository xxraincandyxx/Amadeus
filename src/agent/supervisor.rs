//! # Supervisor Pattern
//!
//! Manages a pool of worker agents with configurable dispatch strategies.

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{mpsc, Mutex, RwLock};
use tracing::{debug, info, warn};

use crate::agent::config::Config;
use crate::agent::loop_agent::Agent;
use crate::client::LLMClient;
use crate::concurrency::LockManager;
use crate::core::id::AgentId;
use crate::error::{AgentError, Result};
use crate::tools::peer::PeerTool;

use super::worker::{HelpRequest, Task, TaskResult, WorkerConfig, WorkerInfo, WorkerStatus};

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
    info: Arc<RwLock<WorkerInfo>>,
    agent: Agent<C>,
}

/// Internal queue entry.
struct QueueEntry {
    task: Task,
    response_tx: mpsc::Sender<Result<TaskResult>>,
}

/// Supervisor manages a pool of worker agents.
pub struct Supervisor<C: LLMClient> {
    client: C,
    config: SupervisorConfig,
    sdk_config: Arc<Config>,
    workers: Arc<RwLock<HashMap<AgentId, WorkerEntry<C>>>>,
    lock_manager: Arc<Mutex<LockManager>>,
    next_worker_idx: Arc<Mutex<usize>>,
    help_tx: mpsc::Sender<HelpRequest>,
    help_rx: Mutex<mpsc::Receiver<HelpRequest>>,
    task_queue: Mutex<VecDeque<QueueEntry>>,
}

impl<C: LLMClient + Clone + 'static> Supervisor<C> {
    /// Create a new supervisor.
    pub fn new(client: C, config: SupervisorConfig, sdk_config: Arc<Config>) -> Self {
        let (help_tx, help_rx) = mpsc::channel(100);
        Self {
            client,
            config,
            sdk_config,
            workers: Arc::new(RwLock::new(HashMap::new())),
            lock_manager: Arc::new(Mutex::new(LockManager::new())),
            next_worker_idx: Arc::new(Mutex::new(0)),
            help_tx,
            help_rx: Mutex::new(help_rx),
            task_queue: Mutex::new(VecDeque::new()),
        }
    }

    /// Get the lock manager for resource coordination.
    pub fn lock_manager(&self) -> Arc<Mutex<LockManager>> {
        Arc::clone(&self.lock_manager)
    }

    /// Spawn worker agents.
    pub async fn spawn(&mut self, configs: Vec<WorkerConfig>) -> Result<Vec<AgentId>> {
        self.spawn_with_client(configs, self.client.clone()).await
    }

    /// Spawn worker agents with a specific client.
    pub async fn spawn_with_client(&mut self, configs: Vec<WorkerConfig>, client: C) -> Result<Vec<AgentId>> {
        let mut ids = Vec::new();
        let mut workers = self.workers.write().await;

        for worker_config in configs {
            let id = worker_config.id.unwrap_or_else(AgentId::new);

            let worker_sdk_config = if let Some(model) = worker_config.model {
                let mut cfg = (*self.sdk_config).clone();
                cfg.model = model;
                Arc::new(cfg)
            } else {
                Arc::clone(&self.sdk_config)
            };

            // Initialize agent with PeerTool
            let agent = crate::agent::loop_agent::AgentBuilder::new(client.clone(), worker_sdk_config)
                .with_default_tools()
                .register_tool(Box::new(PeerTool::new(id, self.help_tx.clone())))
                .build();

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

            workers.insert(
                id,
                WorkerEntry {
                    info: Arc::new(RwLock::new(info)),
                    agent,
                },
            );

            ids.push(id);
        }

        info!(workers = ids.len(), "Workers spawned");
        Ok(ids)
    }

    /// Get worker info.
    pub async fn worker(&self, id: AgentId) -> Option<WorkerInfo> {
        let workers = self.workers.read().await;
        if let Some(w) = workers.get(&id) {
            Some(w.info.read().await.clone())
        } else {
            None
        }
    }

    /// Run the supervisor background loop to process help requests.
    pub async fn run(&self) -> Result<()> {
        info!("Supervisor loop started");
        
        let mut interval = tokio::time::interval(Duration::from_millis(100));
        
        loop {
            tokio::select! {
                // Handle help requests from peers
                help_req_opt = async {
                    let mut help_rx = self.help_rx.lock().await;
                    help_rx.recv().await
                } => {
                    if let Some(help_req) = help_req_opt {
                        let workers_map = Arc::clone(&self.workers);
                        let strategy = self.config.strategy;
                        let timeout_dur = self.config.task_timeout;
                        let next_idx_mutex = Arc::clone(&self.next_worker_idx);

                        tokio::spawn(async move {
                            let worker_selection = {
                                let workers_guard = workers_map.read().await;
                                Self::select_worker_internal(&workers_guard, &help_req.task, strategy, next_idx_mutex).await
                            };

                            match worker_selection {
                                Ok(id) => {
                                    let workers_guard = workers_map.read().await;
                                    if let Some(entry) = workers_guard.get(&id) {
                                        let result = Self::dispatch_internal(id, entry, help_req.task, timeout_dur).await;
                                        let _ = help_req.response_tx.send(result.unwrap_or_else(|e| TaskResult {
                                            task_id: "error".to_string(),
                                            worker_id: id,
                                            success: false,
                                            output: None,
                                            error: Some(e.to_string()),
                                            duration_ms: 0,
                                            tool_calls: Vec::new(),
                                        }));
                                    }
                                }
                                Err(e) => {
                                    warn!("Failed to find worker for help request: {}", e);
                                    let _ = help_req.response_tx.send(TaskResult {
                                        task_id: help_req.task.id,
                                        worker_id: AgentId::new(),
                                        success: false,
                                        output: None,
                                        error: Some(format!("No available worker for help request: {}", e)),
                                        duration_ms: 0,
                                        tool_calls: Vec::new(),
                                    });
                                }
                            }
                        });
                    }
                }

                // Check queue periodically
                _ = interval.tick() => {
                    self.process_queue().await;
                }
            }
        }
    }

    async fn process_queue(&self) {
        let mut queue = self.task_queue.lock().await;
        if queue.is_empty() {
            return;
        }

        let mut next_queue = VecDeque::new();
        while let Some(entry) = queue.pop_front() {
            let workers_map = Arc::clone(&self.workers);
            let strategy = self.config.strategy;
            let next_idx_mutex = Arc::clone(&self.next_worker_idx);
            
            let worker_selection = {
                let workers_guard = workers_map.read().await;
                Self::select_worker_internal(&workers_guard, &entry.task, strategy, next_idx_mutex).await
            };

            if let Ok(id) = worker_selection {
                let timeout_dur = self.config.task_timeout;
                tokio::spawn(async move {
                    let workers_guard = workers_map.read().await;
                    if let Some(worker_entry) = workers_guard.get(&id) {
                        let result = Self::dispatch_internal(id, worker_entry, entry.task, timeout_dur).await;
                        let _ = entry.response_tx.send(result).await;
                    }
                });
            } else {
                next_queue.push_back(entry);
            }
        }
        *queue = next_queue;
    }

    /// Execute a task (buffered version).
    pub async fn execute(&self, task: Task) -> Result<TaskResult> {
        let (tx, mut rx) = mpsc::channel(1);
        {
            let mut queue = self.task_queue.lock().await;
            if queue.len() >= self.config.max_pending_tasks {
                return Err(AgentError::Config("Task queue is full".to_string()));
            }
            queue.push_back(QueueEntry { task, response_tx: tx });
        }

        rx.recv().await.ok_or_else(|| AgentError::Command("Task response channel closed".to_string()))?
    }

    async fn select_worker_internal(
        workers: &HashMap<AgentId, WorkerEntry<C>>,
        task: &Task,
        strategy: DispatchStrategy,
        next_idx_mutex: Arc<Mutex<usize>>,
    ) -> Result<AgentId> {
        let mut candidates = Vec::new();
        for (id, entry) in workers {
            let info = entry.info.read().await;
            if info.is_available() {
                candidates.push((*id, info.clone()));
            }
        }

        let worker_id = match strategy {
            DispatchStrategy::RoundRobin => {
                if candidates.is_empty() {
                    None
                } else {
                    let mut next_idx = next_idx_mutex.lock().await;
                    let idx = *next_idx % candidates.len();
                    *next_idx += 1;
                    Some(candidates[idx].0)
                }
            }
            DispatchStrategy::LeastLoaded => {
                candidates
                    .iter()
                    .min_by_key(|(_, info)| info.active_tasks)
                    .map(|(id, _)| *id)
            }
            DispatchStrategy::CapabilityMatch => {
                candidates
                    .iter()
                    .filter(|(_, info)| info.has_capabilities(&task.required_capabilities))
                    .min_by_key(|(_, info)| info.active_tasks)
                    .map(|(id, _)| *id)
            }
        };

        worker_id.ok_or_else(|| AgentError::Config("No available worker".to_string()))
    }

    async fn dispatch_internal(
        worker_id: AgentId,
        entry: &WorkerEntry<C>,
        task: Task,
        task_timeout: Duration,
    ) -> Result<TaskResult> {
        let task_id = task.id.clone();
        let prompt = task.prompt.clone();

        {
            let mut info = entry.info.write().await;
            info.status = WorkerStatus::Busy;
            info.active_tasks += 1;
        }

        let agent = entry.agent.clone();
        debug!(worker_id = %worker_id, task_id = %task_id, "Dispatching task");

        let result = tokio::time::timeout(task_timeout, agent.run(&prompt)).await;

        let task_res = match result {
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
        };

        {
            let mut info = entry.info.write().await;
            info.active_tasks = info.active_tasks.saturating_sub(1);

            if task_res.success {
                info.completed_tasks += 1;
            } else {
                info.total_errors += 1;
            }

            info.status = if info.active_tasks > 0 {
                WorkerStatus::Busy
            } else {
                WorkerStatus::Idle
            };
        }

        Ok(task_res)
    }
}
