// @amadeus-header
// summary: Canonical orchestra surface unifying orchestration naming across core runtime APIs.
// layer: agent
// status: active
// feature_flags:
// - orchestra
// provides:
// - module: crate::agent::orchestra
// - type: crate::agent::orchestra::AgentInfo
// - type: crate::agent::orchestra::AgentStatus
// - type: crate::agent::orchestra::AgentOrchestrator
// - type: crate::agent::orchestra::OrchestraRuntime
// - type: crate::agent::orchestra::OrchestraLeader
// - type: crate::agent::orchestra::AgentOrchestra
// - type: crate::agent::orchestra::OrchestraRegistry
// - type: crate::agent::orchestra::OrchestraConfig
// - type: crate::agent::orchestra::OrchestraStrategy
// - type: crate::agent::orchestra::Task
// - type: crate::agent::orchestra::TaskResult
// - type: crate::agent::orchestra::WorkerConfig
// - type: crate::agent::orchestra::WorkerInfo
// - type: crate::agent::orchestra::WorkerStatus
// uses:
// - module: crate::agent::config::Config
// - module: crate::agent::loop_agent::Agent
// - module: crate::agent::profile::AgentProfile
// - module: crate::agent::worker
// - module: crate::client::LLMClient
// - module: crate::concurrency::LockManager
// - module: crate::core::id
// - module: crate::error
// - module: crate::telemetry
// - module: crate::tools::peer::PeerTool
// - module: amadeus_runtime
// - runtime: tokio async runtime
// invariants:
// - Orchestra naming remains the primary public surface while legacy modules stay deprecated.
// side_effects:
// - Spawns asynchronous tasks.
// - Sends or receives messages across async channels.
// tests:
// - tests/agent_integration_test.rs
// - tests/p2p_test.rs
// - tests/e2e_product_flow.rs
// @end-amadeus-header

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};

pub use super::worker::{Task, TaskResult, WorkerConfig, WorkerInfo, WorkerStatus};
use crate::agent::config::Config;
use crate::agent::loop_agent::Agent;
use crate::agent::profile::AgentProfile;
use crate::client::LLMClient;
use crate::concurrency::LockManager;
use crate::core::id::{AgentId, TeamId};
use crate::error::{AgentError, Result};
use crate::telemetry::{TelemetryEvent, TelemetryRecorder};
use crate::tools::peer::PeerTool;
use amadeus_runtime::{select_worker, select_worker_with_exclusions};
pub use amadeus_runtime::{
    AgentInfo, AgentOrchestra, AgentStatus, ArtifactRecord, MailboxEvent, MailboxEventKind,
    OrchestraConfig, OrchestraLeader, OrchestraRegistry, OrchestraStatus, OrchestraStrategy,
    OrchestraTask, OrchestraTaskStatus,
};
use tokio::sync::{mpsc, Mutex, RwLock};
use tracing::{debug, info, warn};

use super::worker::{finalize_worker_task, mark_worker_task_started, HelpRequest, RunOutcome};

pub(crate) struct OrchestratedAgent<C: LLMClient> {
    pub(crate) id: AgentId,
    pub(crate) agent: Agent<C>,
    pub(crate) name: String,
    pub(crate) profile: AgentProfile,
    pub(crate) capabilities: Vec<String>,
    pub(crate) status: AgentStatus,
    pub(crate) task_count: usize,
}

pub(crate) struct OrchestraRoster<C: LLMClient> {
    client: C,
    config: Arc<Config>,
    telemetry: Option<Arc<TelemetryRecorder>>,
    memory_registry: Option<crate::context::memory::MemoryRegistry>,
    pub(crate) agents: Vec<OrchestratedAgent<C>>,
    pub(crate) active_index: usize,
    name_counter: usize,
}

impl<C: LLMClient + Clone + 'static> OrchestraRoster<C> {
    pub(crate) fn new(
        client: C,
        config: Arc<Config>,
        telemetry: Option<Arc<TelemetryRecorder>>,
    ) -> Self {
        Self {
            client,
            config,
            telemetry,
            memory_registry: None,
            agents: Vec::new(),
            active_index: 0,
            name_counter: 0,
        }
    }

    pub(crate) fn with_memory_registry(
        mut self,
        registry: crate::context::memory::MemoryRegistry,
    ) -> Self {
        self.memory_registry = Some(registry);
        self
    }

    pub(crate) async fn create_agent(
        &mut self,
        name: Option<String>,
        profile: AgentProfile,
    ) -> Result<AgentId> {
        self.create_agent_with_capabilities(name, profile, Vec::new(), None)
            .await
    }

    pub(crate) async fn spawn_agent(&mut self, config: WorkerConfig) -> Result<AgentId> {
        self.create_agent_with_capabilities(
            Some(config.name),
            AgentProfile::Default,
            config.capabilities,
            config.model,
        )
        .await
    }

    pub(crate) fn list_agents(&self) -> Vec<AgentInfo> {
        amadeus_runtime::list_agent_info(&self.roster_entries(), self.active_agent_id())
    }

    pub(crate) fn get_agent(&self, agent_id: &AgentId) -> Option<AgentInfo> {
        amadeus_runtime::get_agent_info(&self.roster_entries(), *agent_id)
    }

    pub(crate) fn active_agent_id(&self) -> Option<AgentId> {
        self.agents.get(self.active_index).map(|handle| handle.id)
    }

    pub(crate) fn switch_to(&mut self, agent_id: &AgentId) -> Result<()> {
        if let Some(index) = amadeus_runtime::find_agent_index(&self.roster_entries(), *agent_id) {
            self.active_index = index;
            Ok(())
        } else {
            Err(AgentError::Command(format!("Unknown agent: {}", agent_id)))
        }
    }

    pub(crate) fn kill(&mut self, agent_id: &AgentId) -> Result<()> {
        if self.agents.len() == 1 {
            return Err(AgentError::Command(
                "Cannot kill the last agent".to_string(),
            ));
        }

        let index = self
            .agents
            .iter()
            .position(|agent| &agent.id == agent_id)
            .ok_or_else(|| AgentError::Command(format!("Unknown agent: {}", agent_id)))?;
        self.agents.remove(index);
        self.active_index = amadeus_runtime::normalize_active_index_after_removal(
            self.agents.len(),
            self.active_index,
        );
        Ok(())
    }

    pub(crate) fn contains(&self, agent_id: AgentId) -> bool {
        self.agents.iter().any(|agent| agent.id == agent_id)
    }

    pub(crate) fn select_agent_index(
        &self,
        allowed_ids: Option<&[AgentId]>,
        task: &Task,
    ) -> Result<usize> {
        let candidates = self
            .agents
            .iter()
            .map(|handle| amadeus_runtime::AgentRouteCandidate {
                id: handle.id,
                capabilities: handle.capabilities.clone(),
            })
            .collect::<Vec<_>>();
        let selected_id =
            amadeus_runtime::select_agent(&candidates, self.active_agent_id(), allowed_ids, task);

        selected_id
            .and_then(|agent_id| self.agents.iter().position(|handle| handle.id == agent_id))
            .ok_or_else(|| {
                if task.required_capabilities.is_empty() {
                    AgentError::Command("No agents available".to_string())
                } else {
                    AgentError::Command(format!(
                        "No agent matched capabilities: {}",
                        task.required_capabilities.join(", ")
                    ))
                }
            })
    }

    pub(crate) fn agent_id_at(&self, index: usize) -> AgentId {
        self.agents[index].id
    }

    pub(crate) fn agent_clone_at(&self, index: usize) -> Agent<C> {
        self.agents[index].agent.clone()
    }

    pub(crate) fn mark_running(&mut self, index: usize) {
        self.agents[index].status = AgentStatus::Running;
        self.active_index = index;
    }

    pub(crate) fn mark_idle(&mut self, index: usize) {
        self.agents[index].status = AgentStatus::Idle;
    }

    pub(crate) fn mark_error(&mut self, index: usize) {
        self.agents[index].status = AgentStatus::Error;
    }

    pub(crate) fn increment_task_count(&mut self, index: usize) {
        self.agents[index].task_count += 1;
    }

    pub(crate) fn peer_info(
        &self,
        exclude_agent_id: &AgentId,
    ) -> Vec<crate::tools::peer::PeerInfo> {
        self.agents
            .iter()
            .filter(|agent| &agent.id != exclude_agent_id)
            .map(|agent| crate::tools::peer::PeerInfo {
                id: agent.id,
                name: agent.name.clone(),
                profile: agent.profile.to_string(),
                description: agent.capabilities.join(", "),
            })
            .collect()
    }

    pub(crate) fn call_peer_enabled(&self) -> bool {
        self.agents.len() >= 2
    }

    pub(crate) fn agent_count(&self) -> usize {
        self.agents.len()
    }

    pub(crate) fn switch_next(&mut self) {
        if let Some(index) = amadeus_runtime::next_agent_index(self.agents.len(), self.active_index)
        {
            self.active_index = index;
        }
    }

    pub(crate) fn switch_prev(&mut self) {
        if let Some(index) =
            amadeus_runtime::previous_agent_index(self.agents.len(), self.active_index)
        {
            self.active_index = index;
        }
    }

    fn roster_entries(&self) -> Vec<AgentInfo> {
        self.agents
            .iter()
            .map(|handle| AgentInfo {
                id: handle.id,
                name: handle.name.clone(),
                profile: handle.profile.clone(),
                status: handle.status,
                task_count: handle.task_count,
            })
            .collect()
    }

    async fn create_agent_with_capabilities(
        &mut self,
        name: Option<String>,
        profile: AgentProfile,
        capabilities: Vec<String>,
        model_override: Option<String>,
    ) -> Result<AgentId> {
        let name = name.unwrap_or_else(|| {
            self.name_counter += 1;
            format!("{}-{}", profile.display_name(), self.name_counter)
        });
        let telemetry_name = name.clone();
        let telemetry_capabilities = capabilities.clone();

        let id = AgentId::new();
        let agent_config = if let Some(model) = model_override {
            let mut config = (*self.config).clone();
            config.model = model;
            Arc::new(config)
        } else {
            Arc::clone(&self.config)
        };
        let mut builder = Agent::builder(self.client.clone(), agent_config)
            .with_default_tools()
            .with_optional_telemetry(self.telemetry.clone());
        if let Some(ref mem) = self.memory_registry {
            builder = builder.with_memory_registry(mem.clone());
        }
        let agent = builder.build();

        self.agents.push(OrchestratedAgent {
            id,
            agent,
            name,
            profile: profile.clone(),
            capabilities,
            status: AgentStatus::Idle,
            task_count: 0,
        });

        AgentOrchestrator::<C>::emit_telemetry(
            &self.telemetry,
            TelemetryEvent::WorkerSpawned {
                runtime_id: "local-roster".to_string(),
                worker_id: id,
                name: telemetry_name,
                capabilities: telemetry_capabilities,
            },
        );

        if self.agents.len() == 1 {
            self.active_index = 0;
        }

        Ok(id)
    }
}

struct WorkerEntry<C: LLMClient> {
    info: Arc<RwLock<WorkerInfo>>,
    agent: Agent<C>,
}

struct QueueEntry {
    task: Task,
    team_id: TeamId,
    response_tx: mpsc::Sender<Result<TaskResult>>,
}

/// Canonical orchestra-aware agent registry and routing surface.
pub struct AgentOrchestrator<C: LLMClient> {
    roster: OrchestraRoster<C>,
    orchestras: OrchestraRegistry,
    telemetry: Option<Arc<TelemetryRecorder>>,
}

impl<C: LLMClient + Clone + 'static> AgentOrchestrator<C> {
    /// Create a new orchestrator.
    pub fn new(client: C, config: Arc<Config>) -> Self {
        Self {
            roster: OrchestraRoster::new(client, config, None),
            orchestras: OrchestraRegistry::new(),
            telemetry: None,
        }
    }

    fn emit_telemetry(recorder: &Option<Arc<TelemetryRecorder>>, event: TelemetryEvent) {
        if let Some(recorder) = recorder {
            if let Err(error) = recorder.record(event) {
                warn!(error = %error, "Failed to record telemetry event");
            }
        }
    }

    /// Attach a telemetry recorder to the orchestrator and its roster-created agents.
    pub fn with_telemetry(mut self, telemetry: Arc<TelemetryRecorder>) -> Self {
        self.telemetry = Some(Arc::clone(&telemetry));
        self.roster.telemetry = Some(telemetry);
        self
    }

    /// Attach a memory registry for injection into newly created agents.
    pub fn with_memory_registry(
        mut self,
        registry: crate::context::memory::MemoryRegistry,
    ) -> Self {
        self.roster = self.roster.with_memory_registry(registry);
        self
    }

    /// Create a new agent using the given profile.
    pub async fn create_agent(
        &mut self,
        name: Option<String>,
        profile: AgentProfile,
    ) -> Result<AgentId> {
        self.roster.create_agent(name, profile).await
    }

    /// Spawn a new agent using worker-style runtime configuration.
    pub async fn spawn_agent(&mut self, config: WorkerConfig) -> Result<AgentId> {
        self.roster.spawn_agent(config).await
    }

    /// Create a new orchestra and return its identifier.
    pub fn create_orchestra(&mut self, name: impl Into<String>, leader: OrchestraLeader) -> TeamId {
        self.orchestras.create_team(name, leader)
    }

    /// Ensure there is always a default orchestra for task routing.
    pub fn ensure_default_orchestra(&mut self, leader: OrchestraLeader) -> TeamId {
        if let Some(orchestra_id) = self.orchestras.default_team_id() {
            orchestra_id
        } else {
            self.orchestras.create_team("default", leader)
        }
    }

    /// List all orchestras.
    pub fn list_orchestras(&self) -> Vec<AgentOrchestra> {
        self.orchestras.list_teams()
    }

    /// Add an agent to an orchestra.
    pub fn add_agent_to_orchestra(
        &mut self,
        orchestra_id: TeamId,
        agent_id: AgentId,
    ) -> Result<()> {
        if !self.roster.contains(agent_id) {
            return Err(AgentError::Command(format!("Unknown agent: {}", agent_id)));
        }
        self.orchestras.add_member(orchestra_id, agent_id)?;
        Ok(())
    }

    /// Execute a task using the best available local agent in the selected orchestra.
    pub async fn execute_task(
        &mut self,
        orchestra_id: Option<TeamId>,
        task: Task,
    ) -> Result<TaskResult> {
        let task_id = task.id.clone();
        let target_orchestra_id = orchestra_id.or_else(|| self.orchestras.default_team_id());
        let allowed_ids = target_orchestra_id.and_then(|team_id| {
            self.orchestras
                .get_team(team_id)
                .map(|team| team.members.clone())
        });
        let selected_index = self
            .roster
            .select_agent_index(allowed_ids.as_deref(), &task)?;
        let selected_id = self.roster.agent_id_at(selected_index);
        let agent = self.roster.agent_clone_at(selected_index);

        if let Some(team_id) = target_orchestra_id {
            self.orchestras
                .queue_task(team_id, task.clone(), OrchestraLeader::User)?;
            self.orchestras.claim_task(team_id, &task.id, selected_id)?;
        }

        self.roster.mark_running(selected_index);
        Self::emit_telemetry(
            &self.telemetry,
            TelemetryEvent::TaskDispatched {
                runtime_id: "local-orchestrator".to_string(),
                task_id: task_id.clone(),
                worker_id: selected_id,
            },
        );

        let start = Instant::now();
        let result = agent.run(&task.prompt).await;
        let duration_ms = start.elapsed().as_millis() as u64;
        self.roster.mark_idle(selected_index);

        let task_result = match result {
            Ok(run_result) => {
                self.roster.increment_task_count(selected_index);
                TaskResult {
                    task_id: task.id.clone(),
                    worker_id: selected_id,
                    success: true,
                    output: Some(run_result.text),
                    error: None,
                    duration_ms,
                    tool_calls: run_result.tool_calls,
                }
            }
            Err(error) => {
                self.roster.mark_error(selected_index);
                TaskResult {
                    task_id: task.id.clone(),
                    worker_id: selected_id,
                    success: false,
                    output: None,
                    error: Some(error.to_string()),
                    duration_ms,
                    tool_calls: Vec::new(),
                }
            }
        };

        if let Some(team_id) = target_orchestra_id {
            self.orchestras
                .record_result(team_id, &task.id, selected_id, &task_result)?;
        }

        Self::emit_telemetry(
            &self.telemetry,
            TelemetryEvent::TaskCompleted {
                runtime_id: "local-orchestrator".to_string(),
                task_id,
                worker_id: selected_id,
                success: task_result.success,
                duration_ms,
            },
        );

        if task_result.success {
            Ok(task_result)
        } else {
            Err(AgentError::Command(
                task_result
                    .error
                    .clone()
                    .unwrap_or_else(|| "Task execution failed".to_string()),
            ))
        }
    }

    /// List all active agents.
    pub fn list_agents(&self) -> Vec<AgentInfo> {
        self.roster.list_agents()
    }

    /// Get info for a specific agent.
    pub fn get_agent(&self, agent_id: &AgentId) -> Option<AgentInfo> {
        self.roster.get_agent(agent_id)
    }

    /// Get the currently active agent ID.
    pub fn active_agent_id(&self) -> Option<AgentId> {
        self.roster.active_agent_id()
    }

    /// Switch the active agent.
    pub fn switch_to(&mut self, agent_id: &AgentId) -> Result<()> {
        self.roster.switch_to(agent_id)
    }

    /// Remove an agent from the orchestrator.
    pub fn kill(&mut self, agent_id: &AgentId) -> Result<()> {
        self.roster.kill(agent_id)
    }

    /// Switch to the next agent.
    pub fn switch_next(&mut self) {
        self.roster.switch_next();
    }

    /// Switch to the previous agent.
    pub fn switch_prev(&mut self) {
        self.roster.switch_prev();
    }

    /// Get peer information for the call-peer tool.
    pub fn get_peers(&self, exclude_agent_id: &AgentId) -> Vec<crate::tools::peer::PeerInfo> {
        self.roster.peer_info(exclude_agent_id)
    }

    /// Check if call-peer should be enabled.
    pub fn is_call_peer_enabled(&self) -> bool {
        self.roster.call_peer_enabled()
    }

    /// Get the total number of agents.
    pub fn agent_count(&self) -> usize {
        self.roster.agent_count()
    }
}

/// Canonical queued runtime for background orchestra execution.
pub struct OrchestraRuntime<C: LLMClient> {
    runtime_id: String,
    client: C,
    config: OrchestraConfig,
    sdk_config: Arc<Config>,
    telemetry: Option<Arc<TelemetryRecorder>>,
    orchestras: Arc<Mutex<OrchestraRegistry>>,
    default_orchestra_id: TeamId,
    workers: Arc<RwLock<HashMap<AgentId, WorkerEntry<C>>>>,
    lock_manager: Arc<Mutex<LockManager>>,
    next_worker_idx: Arc<Mutex<usize>>,
    help_tx: mpsc::Sender<HelpRequest>,
    help_rx: Mutex<mpsc::Receiver<HelpRequest>>,
    task_queue: Arc<Mutex<VecDeque<QueueEntry>>>,
}

impl<C: LLMClient + Clone + 'static> OrchestraRuntime<C> {
    /// Create a new orchestra runtime.
    pub fn new(client: C, config: OrchestraConfig, sdk_config: Arc<Config>) -> Self {
        let (help_tx, help_rx) = mpsc::channel(100);
        let mut orchestras = OrchestraRegistry::new();
        let default_orchestra_id = orchestras.create_team("default", OrchestraLeader::User);
        Self {
            runtime_id: uuid::Uuid::new_v4().to_string(),
            client,
            config,
            sdk_config,
            telemetry: None,
            orchestras: Arc::new(Mutex::new(orchestras)),
            default_orchestra_id,
            workers: Arc::new(RwLock::new(HashMap::new())),
            lock_manager: Arc::new(Mutex::new(LockManager::new())),
            next_worker_idx: Arc::new(Mutex::new(0)),
            help_tx,
            help_rx: Mutex::new(help_rx),
            task_queue: Arc::new(Mutex::new(VecDeque::new())),
        }
    }

    fn emit_telemetry(&self, event: TelemetryEvent) {
        if let Some(recorder) = &self.telemetry {
            if let Err(error) = recorder.record(event) {
                warn!(error = %error, "Failed to record telemetry event");
            }
        }
    }

    /// Attach a telemetry recorder to the runtime and spawned agents.
    pub fn with_telemetry(mut self, telemetry: Arc<TelemetryRecorder>) -> Self {
        self.telemetry = Some(telemetry);
        self
    }

    /// Get the lock manager for resource coordination.
    pub fn lock_manager(&self) -> Arc<Mutex<LockManager>> {
        Arc::clone(&self.lock_manager)
    }

    /// Get the base LLM client.
    pub fn client(&self) -> &C {
        &self.client
    }

    /// Get the base SDK configuration.
    pub fn config(&self) -> &Arc<Config> {
        &self.sdk_config
    }

    /// List orchestras tracked by the runtime.
    pub async fn list_orchestras(&self) -> Vec<AgentOrchestra> {
        self.orchestras.lock().await.list_teams()
    }

    /// Spawn agents into the orchestra runtime.
    pub async fn spawn_agents(&mut self, configs: Vec<WorkerConfig>) -> Result<Vec<AgentId>> {
        self.spawn_agents_with_client(configs, self.client.clone())
            .await
    }

    /// Spawn agents using a specific client implementation.
    pub async fn spawn_agents_with_client(
        &mut self,
        configs: Vec<WorkerConfig>,
        client: C,
    ) -> Result<Vec<AgentId>> {
        let mut ids = Vec::new();
        let mut workers = self.workers.write().await;

        for worker_config in configs {
            let id = worker_config.id.unwrap_or_else(AgentId::new);
            let name = worker_config.name.clone();
            let capabilities = worker_config.capabilities.clone();

            let worker_sdk_config = if let Some(model) = worker_config.model {
                let mut cfg = (*self.sdk_config).clone();
                cfg.model = model;
                Arc::new(cfg)
            } else {
                Arc::clone(&self.sdk_config)
            };

            let agent =
                crate::agent::loop_agent::AgentBuilder::new(client.clone(), worker_sdk_config)
                    .with_default_tools()
                    .with_optional_telemetry(self.telemetry.clone())
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

            self.emit_telemetry(TelemetryEvent::WorkerSpawned {
                runtime_id: self.runtime_id.clone(),
                worker_id: id,
                name,
                capabilities,
            });

            self.orchestras
                .lock()
                .await
                .add_member(self.default_orchestra_id, id)
                .map_err(|error| AgentError::Command(error.to_string()))?;
            ids.push(id);
        }

        info!(workers = ids.len(), "Workers spawned");
        Ok(ids)
    }

    /// Get execution info for a specific agent in the orchestra runtime.
    pub async fn agent_info(&self, agent_id: AgentId) -> Option<WorkerInfo> {
        let workers = self.workers.read().await;
        if let Some(worker) = workers.get(&agent_id) {
            Some(worker.info.read().await.clone())
        } else {
            None
        }
    }

    /// Run the orchestra background loop to process delegated work.
    pub async fn run(&self) -> Result<()> {
        info!("Orchestra runtime loop started");

        let mut interval = tokio::time::interval(Duration::from_millis(100));

        loop {
            tokio::select! {
                help_req_opt = async {
                    let mut help_rx = self.help_rx.lock().await;
                    help_rx.recv().await
                } => {
                    if let Some(help_req) = help_req_opt {
                        let workers_map = Arc::clone(&self.workers);
                        let orchestras = Arc::clone(&self.orchestras);
                        let default_orchestra_id = self.default_orchestra_id;
                        let strategy = self.config.strategy;
                        let timeout_dur = self.config.task_timeout;
                        let retry_failed_tasks = self.config.retry_failed_tasks;
                        let max_retries = self.config.max_retries;
                        let next_idx_mutex = Arc::clone(&self.next_worker_idx);
                        let task_queue = Arc::clone(&self.task_queue);
                        let runtime_id = self.runtime_id.clone();
                        let telemetry = self.telemetry.clone();

                        tokio::spawn(async move {
                            let team_id = help_req.team_id.unwrap_or(default_orchestra_id);
                            {
                                let mut registry = orchestras.lock().await;
                                let created_by = OrchestraLeader::Agent(help_req.requester_id);
                                if let Err(error) =
                                    registry.queue_task(team_id, help_req.task.clone(), created_by)
                                {
                                    let _ = help_req.response_tx.send(TaskResult {
                                        task_id: help_req.task.id,
                                        worker_id: AgentId::new(),
                                        success: false,
                                        output: None,
                                        error: Some(error.to_string()),
                                        duration_ms: 0,
                                        tool_calls: Vec::new(),
                                    });
                                    return;
                                }

                                let _ = registry.record_mailbox_event(
                                    team_id,
                                    MailboxEvent::new(
                                        format!("msg-{}", uuid::Uuid::new_v4()),
                                        MailboxEventKind::DirectMessage,
                                        Some(help_req.task.id.clone()),
                                        Some(help_req.requester_id),
                                        None,
                                        help_req.task.prompt.clone(),
                                    ),
                                );
                            }

                            let excluded_ids = if help_req.exclude_requester {
                                vec![help_req.requester_id]
                            } else {
                                Vec::new()
                            };
                            let worker_selection = {
                                let workers_guard = workers_map.read().await;
                                Self::select_worker(
                                    &workers_guard,
                                    &help_req.task,
                                    strategy,
                                    next_idx_mutex,
                                    &excluded_ids,
                                )
                                .await
                            };

                            match worker_selection {
                                Ok(id) => {
                                    if let Err(error) = orchestras
                                        .lock()
                                        .await
                                        .claim_task(team_id, &help_req.task.id, id)
                                    {
                                        let _ = help_req.response_tx.send(TaskResult {
                                            task_id: help_req.task.id,
                                            worker_id: id,
                                            success: false,
                                            output: None,
                                            error: Some(error.to_string()),
                                            duration_ms: 0,
                                            tool_calls: Vec::new(),
                                        });
                                        return;
                                    }

                                    let workers_guard = workers_map.read().await;
                                    if let Some(entry) = workers_guard.get(&id) {
                                        Self::reserve_worker(
                                            runtime_id.clone(),
                                            telemetry.clone(),
                                            id,
                                            entry,
                                        )
                                        .await;
                                        let result = Self::dispatch_internal(
                                            runtime_id.clone(),
                                            telemetry.clone(),
                                            Arc::clone(&orchestras),
                                            team_id,
                                            id,
                                            entry,
                                            help_req.task,
                                            timeout_dur,
                                            retry_failed_tasks,
                                            max_retries,
                                        ).await;
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
                                Err(error) => {
                                    warn!("Failed to find worker for help request: {}", error);
                                    let _ = orchestras.lock().await.record_mailbox_event(
                                        team_id,
                                        MailboxEvent::new(
                                            format!("msg-{}", uuid::Uuid::new_v4()),
                                            MailboxEventKind::StatusUpdate,
                                            Some(help_req.task.id.clone()),
                                            None,
                                            Some(help_req.requester_id),
                                            format!("Peer request failed to route: {}", error),
                                        ),
                                    );
                                    let mut queue = task_queue.lock().await;
                                    queue.retain(|entry| entry.task.id != help_req.task.id);
                                    let _ = help_req.response_tx.send(TaskResult {
                                        task_id: help_req.task.id,
                                        worker_id: AgentId::new(),
                                        success: false,
                                        output: None,
                                        error: Some(format!("No available worker for help request: {}", error)),
                                        duration_ms: 0,
                                        tool_calls: Vec::new(),
                                    });
                                }
                            }
                        });
                    }
                }

                _ = interval.tick() => {
                    self.process_queue().await;
                }
            }
        }
    }

    /// Execute a task through the queued orchestra runtime.
    pub async fn execute(&self, task: Task) -> Result<TaskResult> {
        let (tx, mut rx) = mpsc::channel(1);
        {
            let mut queue = self.task_queue.lock().await;
            if queue.len() >= self.config.max_pending_tasks {
                return Err(AgentError::Config("Task queue is full".to_string()));
            }
            self.orchestras
                .lock()
                .await
                .queue_task(
                    self.default_orchestra_id,
                    task.clone(),
                    OrchestraLeader::User,
                )
                .map_err(|error| AgentError::Command(error.to_string()))?;
            self.emit_telemetry(TelemetryEvent::TaskQueued {
                runtime_id: self.runtime_id.clone(),
                task_id: task.id.clone(),
            });
            queue.push_back(QueueEntry {
                task,
                team_id: self.default_orchestra_id,
                response_tx: tx,
            });
        }

        rx.recv()
            .await
            .ok_or_else(|| AgentError::Command("Task response channel closed".to_string()))?
    }

    async fn process_queue(&self) {
        let mut queue = self.task_queue.lock().await;
        if queue.is_empty() {
            return;
        }

        let mut queued = queue.drain(..).collect::<Vec<_>>();
        queued.sort_by(|left, right| {
            right
                .task
                .priority
                .cmp(&left.task.priority)
                .then_with(|| left.task.id.cmp(&right.task.id))
        });
        let mut next_queue = VecDeque::new();
        for entry in queued {
            let workers_map = Arc::clone(&self.workers);
            let orchestras = Arc::clone(&self.orchestras);
            let strategy = self.config.strategy;
            let retry_failed_tasks = self.config.retry_failed_tasks;
            let max_retries = self.config.max_retries;
            let next_idx_mutex = Arc::clone(&self.next_worker_idx);
            let runtime_id = self.runtime_id.clone();
            let telemetry = self.telemetry.clone();

            let worker_selection = {
                let workers_guard = workers_map.read().await;
                Self::select_worker(&workers_guard, &entry.task, strategy, next_idx_mutex, &[])
                    .await
            };

            if let Ok(id) = worker_selection {
                if let Err(error) =
                    orchestras
                        .lock()
                        .await
                        .claim_task(entry.team_id, &entry.task.id, id)
                {
                    warn!(task_id = %entry.task.id, error = %error, "Failed to claim task");
                    next_queue.push_back(entry);
                    continue;
                }
                let timeout_dur = self.config.task_timeout;
                self.emit_telemetry(TelemetryEvent::TaskDispatched {
                    runtime_id: self.runtime_id.clone(),
                    task_id: entry.task.id.clone(),
                    worker_id: id,
                });
                tokio::spawn(async move {
                    let workers_guard = workers_map.read().await;
                    if let Some(worker_entry) = workers_guard.get(&id) {
                        Self::reserve_worker(
                            runtime_id.clone(),
                            telemetry.clone(),
                            id,
                            worker_entry,
                        )
                        .await;
                        let result = Self::dispatch_internal(
                            runtime_id,
                            telemetry,
                            orchestras,
                            entry.team_id,
                            id,
                            worker_entry,
                            entry.task,
                            timeout_dur,
                            retry_failed_tasks,
                            max_retries,
                        )
                        .await;
                        let _ = entry.response_tx.send(result).await;
                    }
                });
            } else {
                next_queue.push_back(entry);
            }
        }
        *queue = next_queue;
    }

    async fn select_worker(
        workers: &HashMap<AgentId, WorkerEntry<C>>,
        task: &Task,
        strategy: OrchestraStrategy,
        next_idx_mutex: Arc<Mutex<usize>>,
        excluded_ids: &[AgentId],
    ) -> Result<AgentId> {
        let mut candidates = Vec::new();
        for entry in workers.values() {
            let info = entry.info.read().await;
            candidates.push(info.clone());
        }

        let worker_id = {
            let mut next_idx = next_idx_mutex.lock().await;
            if excluded_ids.is_empty() {
                select_worker(&candidates, task, strategy, &mut next_idx)
            } else {
                select_worker_with_exclusions(
                    &candidates,
                    task,
                    strategy,
                    &mut next_idx,
                    excluded_ids,
                )
            }
        };

        worker_id.ok_or_else(|| AgentError::Config("No available worker".to_string()))
    }

    #[allow(clippy::too_many_arguments)]
    async fn dispatch_internal(
        runtime_id: String,
        telemetry: Option<Arc<TelemetryRecorder>>,
        orchestras: Arc<Mutex<OrchestraRegistry>>,
        team_id: TeamId,
        worker_id: AgentId,
        entry: &WorkerEntry<C>,
        task: Task,
        task_timeout: Duration,
        retry_failed_tasks: bool,
        max_retries: u8,
    ) -> Result<TaskResult> {
        let task_id = task.id.clone();
        let prompt = task.prompt.clone();

        let agent = entry.agent.clone();
        debug!(worker_id = %worker_id, task_id = %task_id, "Dispatching task");

        let total_attempts = if retry_failed_tasks {
            task.max_attempts
                .unwrap_or_else(|| max_retries.saturating_add(1))
                .max(1)
        } else {
            1
        };
        let mut total_duration_ms = 0_u64;
        let mut outcome = Err("Task execution failed".to_string());

        for attempt in 1..=total_attempts {
            if attempt > 1 {
                orchestras
                    .lock()
                    .await
                    .record_attempt(team_id, &task_id)
                    .map_err(|error| AgentError::Command(error.to_string()))?;
                let _ = orchestras.lock().await.record_mailbox_event(
                    team_id,
                    MailboxEvent::new(
                        format!("msg-{}", uuid::Uuid::new_v4()),
                        MailboxEventKind::StatusUpdate,
                        Some(task_id.clone()),
                        Some(worker_id),
                        None,
                        format!("Retrying task attempt {}/{}", attempt, total_attempts),
                    ),
                );
            }

            let start = Instant::now();
            let result = tokio::time::timeout(task_timeout, agent.run(&prompt)).await;
            total_duration_ms += start.elapsed().as_millis() as u64;

            match result {
                Ok(Ok(run_result)) => {
                    outcome = Ok(RunOutcome {
                        text: run_result.text,
                        duration_ms: total_duration_ms,
                        tool_calls: run_result.tool_calls,
                    });
                    break;
                }
                Ok(Err(error)) => {
                    outcome = Err(error.to_string());
                }
                Err(_) => {
                    outcome = Err("Task timed out".to_string());
                }
            }

            if attempt < total_attempts {
                let _ = orchestras.lock().await.mark_retry_ready(
                    team_id,
                    &task_id,
                    worker_id,
                    outcome
                        .as_ref()
                        .err()
                        .cloned()
                        .unwrap_or_else(|| "Retry requested".to_string()),
                );
            }
        }

        let mut info = entry.info.write().await;
        let mut task_result = finalize_worker_task(&mut info, task_id.clone(), worker_id, outcome);
        task_result.duration_ms = total_duration_ms;
        let state = if task_result.success { "idle" } else { "error" }.to_string();
        AgentOrchestrator::<C>::emit_telemetry(
            &telemetry,
            TelemetryEvent::WorkerStateChanged {
                runtime_id: runtime_id.clone(),
                worker_id,
                state,
                active_tasks: info.active_tasks,
            },
        );
        AgentOrchestrator::<C>::emit_telemetry(
            &telemetry,
            TelemetryEvent::TaskCompleted {
                runtime_id,
                task_id: task_result.task_id.clone(),
                worker_id,
                success: task_result.success,
                duration_ms: task_result.duration_ms,
            },
        );
        drop(info);

        {
            let mut registry = orchestras.lock().await;
            registry
                .record_result(team_id, &task_id, worker_id, &task_result)
                .map_err(|error| AgentError::Command(error.to_string()))?;
            if task_result.success {
                if let Some(output) = &task_result.output {
                    let _ = registry.add_artifact(
                        team_id,
                        &task_id,
                        ArtifactRecord {
                            label: "output".to_string(),
                            value: output.clone(),
                        },
                    );
                }
            }
            let _ = registry.record_mailbox_event(
                team_id,
                MailboxEvent::new(
                    format!("msg-{}", uuid::Uuid::new_v4()),
                    MailboxEventKind::StatusUpdate,
                    Some(task_id),
                    Some(worker_id),
                    None,
                    if task_result.success {
                        "Task completed".to_string()
                    } else {
                        format!(
                            "Task failed: {}",
                            task_result
                                .error
                                .clone()
                                .unwrap_or_else(|| "unknown error".to_string())
                        )
                    },
                ),
            );
        }
        Ok(task_result)
    }

    async fn reserve_worker(
        runtime_id: String,
        telemetry: Option<Arc<TelemetryRecorder>>,
        worker_id: AgentId,
        entry: &WorkerEntry<C>,
    ) {
        let mut info = entry.info.write().await;
        mark_worker_task_started(&mut info);
        AgentOrchestrator::<C>::emit_telemetry(
            &telemetry,
            TelemetryEvent::WorkerStateChanged {
                runtime_id,
                worker_id,
                state: "running".to_string(),
                active_tasks: info.active_tasks,
            },
        );
    }
}
