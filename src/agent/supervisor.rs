use std::collections::hash_map::RandomState;
use std::hash::BuildHasher;
use std::sync::Arc;

use tokio::sync::RwLock;

use crate::client::LLMClient;
use crate::core::id::AgentId;
use crate::core::Workspace;
use crate::error::Result;

use super::agent::Agent;
use super::agent_config::{AgentConfig, AgentStats, AgentStatus};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DispatchStrategy {
    RoundRobin,
    LeastLoaded,
    Random,
    CapabilityMatch,
}

pub struct SupervisorConfig {
    pub supervisor: AgentConfig,
    pub workers: Vec<AgentConfig>,
    pub strategy: DispatchStrategy,
    pub max_parallel: usize,
}

impl Default for SupervisorConfig {
    fn default() -> Self {
        Self {
            supervisor: AgentConfig::new("supervisor"),
            workers: Vec::new(),
            strategy: DispatchStrategy::RoundRobin,
            max_parallel: 4,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Task {
    pub id: String,
    pub prompt: String,
    pub priority: u8,
}

impl Task {
    pub fn new(id: impl Into<String>, prompt: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            prompt: prompt.into(),
            priority: 0,
        }
    }

    pub fn priority(mut self, priority: u8) -> Self {
        self.priority = priority;
        self
    }
}

#[derive(Debug, Clone)]
pub struct TaskResult {
    pub task_id: String,
    pub agent_id: AgentId,
    pub success: bool,
    pub output: Option<String>,
    pub error: Option<String>,
}

pub struct WorkerInfo {
    pub id: AgentId,
    pub config: AgentConfig,
    pub status: AgentStatus,
    pub stats: AgentStats,
    pub task_count: usize,
}

pub struct Supervisor<C: LLMClient> {
    workspace: Arc<RwLock<Workspace>>,
    client: C,
    config: SupervisorConfig,
    workers: Vec<WorkerInfo>,
    next_worker: usize,
}

impl<C: LLMClient + Clone + 'static> Supervisor<C> {
    pub fn new(workspace: Arc<RwLock<Workspace>>, client: C, config: SupervisorConfig) -> Self {
        Self {
            workspace,
            client,
            config,
            workers: Vec::new(),
            next_worker: 0,
        }
    }

    pub async fn spawn_workers(&mut self) -> Result<()> {
        for worker_config in &self.config.workers {
            let agent = Agent::new(
                self.client.clone(),
                worker_config.clone(),
                self.workspace.clone(),
            );
            let worker = WorkerInfo {
                id: agent.id(),
                config: worker_config.clone(),
                status: AgentStatus::Idle,
                stats: AgentStats::default(),
                task_count: 0,
            };
            self.workers.push(worker);
        }
        Ok(())
    }

    pub fn add_worker(&mut self, config: AgentConfig) -> AgentId {
        let agent = Agent::new(self.client.clone(), config.clone(), self.workspace.clone());
        let worker = WorkerInfo {
            id: agent.id(),
            config,
            status: AgentStatus::Idle,
            stats: AgentStats::default(),
            task_count: 0,
        };
        let id = worker.id;
        self.workers.push(worker);
        id
    }

    pub async fn dispatch(&mut self, task: Task) -> Result<TaskResult> {
        let worker_idx = self.select_worker();
        if worker_idx >= self.workers.len() {
            return Err(crate::error::AgentError::Api(
                "No workers available".to_string(),
            ));
        }

        let worker = &self.workers[worker_idx];
        let agent_id = worker.id;
        let config = worker.config.clone();

        let mut agent = Agent::new(self.client.clone(), config, self.workspace.clone());
        self.workers[worker_idx].task_count += 1;
        self.workers[worker_idx].status = AgentStatus::Thinking;

        let result = agent.run(&task.prompt).await;

        self.workers[worker_idx].status = AgentStatus::Idle;

        Ok(match result {
            Ok(run_result) => TaskResult {
                task_id: task.id,
                agent_id,
                success: true,
                output: Some(run_result.text),
                error: None,
            },
            Err(e) => TaskResult {
                task_id: task.id,
                agent_id,
                success: false,
                output: None,
                error: Some(e.to_string()),
            },
        })
    }

    pub async fn dispatch_batch(&mut self, tasks: Vec<Task>) -> Vec<TaskResult> {
        let mut results = Vec::new();
        for task in tasks {
            let result = self.dispatch(task).await;
            results.push(result.unwrap_or_else(|e| TaskResult {
                task_id: String::new(),
                agent_id: AgentId::new(),
                success: false,
                output: None,
                error: Some(e.to_string()),
            }));
        }
        results
    }

    pub async fn broadcast(&mut self, prompt: &str) -> Result<Vec<TaskResult>> {
        let mut results = Vec::new();

        for worker in &self.config.workers {
            let agent_id = worker.id.unwrap_or_else(AgentId::new);
            let mut agent = Agent::new(self.client.clone(), worker.clone(), self.workspace.clone());
            let result = agent.run(prompt).await;

            results.push(match result {
                Ok(run_result) => TaskResult {
                    task_id: "broadcast".to_string(),
                    agent_id,
                    success: true,
                    output: Some(run_result.text),
                    error: None,
                },
                Err(e) => TaskResult {
                    task_id: "broadcast".to_string(),
                    agent_id,
                    success: false,
                    output: None,
                    error: Some(e.to_string()),
                },
            });
        }

        Ok(results)
    }

    fn select_worker(&mut self) -> usize {
        if self.workers.is_empty() {
            return 0;
        }

        match self.config.strategy {
            DispatchStrategy::RoundRobin => {
                let idx = self.next_worker % self.workers.len();
                self.next_worker += 1;
                idx
            }
            DispatchStrategy::LeastLoaded => self
                .workers
                .iter()
                .enumerate()
                .min_by_key(|(_, w)| w.task_count)
                .map(|(i, _)| i)
                .unwrap_or(0),
            DispatchStrategy::Random => {
                let len = self.workers.len();
                if len == 0 {
                    return 0;
                }
                let hash = RandomState::new().hash_one(self.next_worker);
                hash as usize % len
            }
            DispatchStrategy::CapabilityMatch => {
                if self.next_worker >= self.workers.len() {
                    self.next_worker = 0;
                }
                let idx = self.next_worker;
                self.next_worker += 1;
                idx
            }
        }
    }

    pub fn workers_status(&self) -> Vec<(AgentId, AgentStatus, usize)> {
        self.workers
            .iter()
            .map(|w| (w.id, w.status, w.task_count))
            .collect()
    }

    pub fn worker_count(&self) -> usize {
        self.workers.len()
    }

    pub fn idle_worker_count(&self) -> usize {
        self.workers
            .iter()
            .filter(|w| w.status == AgentStatus::Idle)
            .count()
    }
}
