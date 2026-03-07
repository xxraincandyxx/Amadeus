use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::{Arc, RwLock as StdRwLock};
use std::time::Instant;

use chrono::Local;
use flate2::write::GzEncoder;
use flate2::Compression;
use futures::{Stream, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, info, instrument, warn};

use crate::agent::compaction::ContextCompactor;
use crate::agent::config::Config;
use crate::agent::events::{AgentEvent, ApprovalDecision, ApprovalRequest, RunResult, ToolCall};
use crate::agent::messages::{ContentBlock, Message};
use crate::client::{LLMClient, StreamEvent};
use crate::error::{AgentError, Result};
use crate::hooks::{HookAction, HookRegistry};
use crate::policy::Policy;
use crate::tools::registry::ToolRegistry;
use crate::tools::{TodoItem, TodoManager};

#[derive(Debug, Clone)]
struct ToolExecutionRecord {
    id: String,
    name: String,
    input: serde_json::Value,
    output: String,
    is_error: bool,
}

impl ToolExecutionRecord {
    fn new(
        id: String,
        name: String,
        input: serde_json::Value,
        output: String,
        is_error: bool,
    ) -> Self {
        Self {
            id,
            name,
            input,
            output,
            is_error,
        }
    }

    fn completion_event(&self) -> AgentEvent {
        AgentEvent::ToolComplete {
            id: self.id.clone(),
            name: self.name.clone(),
            input: self.input.clone(),
            output: self.output.clone(),
            is_error: self.is_error,
        }
    }

    fn tool_call(&self) -> ToolCall {
        ToolCall {
            name: self.name.clone(),
            input: self.input.clone(),
            output: self.output.clone(),
            is_error: self.is_error,
        }
    }

    fn tool_use_block(&self) -> ContentBlock {
        ContentBlock::ToolUse {
            id: self.id.clone(),
            name: self.name.clone(),
            input: self.input.clone(),
        }
    }

    fn tool_result_block(&self) -> ContentBlock {
        ContentBlock::ToolResult {
            tool_use_id: self.id.clone(),
            content: self.output.clone(),
        }
    }
}

fn record_tool_execution(
    total_result: &mut RunResult,
    tool_uses: &mut Vec<ContentBlock>,
    tool_results: &mut Vec<ContentBlock>,
    record: &ToolExecutionRecord,
) {
    total_result.tool_calls.push(record.tool_call());
    tool_uses.push(record.tool_use_block());
    tool_results.push(record.tool_result_block());
}

/// A log of a single conversation session.
#[derive(Debug, Serialize, Deserialize)]
pub struct SessionLog {
    pub timestamp: String,
    pub model: String,
    pub system_prompt: String,
    pub history: Vec<Message>,
    #[serde(default)]
    pub todos: Vec<TodoItem>,
    pub stats: SessionStats,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct SessionStats {
    pub total_tokens: u32,
    pub tool_calls: usize,
    pub duration_ms: u64,
}

/// A builder for creating an Agent.
pub struct AgentBuilder<C: LLMClient> {
    client: C,
    config: Arc<Config>,
    tools: ToolRegistry,
    history: Option<Arc<RwLock<Vec<Message>>>>,
    todo_manager: Arc<StdRwLock<TodoManager>>,
    hooks: HookRegistry,
    policy: Policy,
}

impl<C: LLMClient + Clone + 'static> AgentBuilder<C> {
    pub fn new(client: C, config: Arc<Config>) -> Self {
        Self {
            client,
            config,
            tools: ToolRegistry::new(),
            history: None,
            todo_manager: Arc::new(StdRwLock::new(TodoManager::new())),
            hooks: HookRegistry::new(),
            policy: Policy::default(),
        }
    }

    /// Add default tools (bash, file operations, glob, grep, web_fetch) to the agent.
    pub fn with_default_tools(mut self) -> Self {
        self.tools =
            ToolRegistry::with_defaults_and_todo(&self.config, Arc::clone(&self.todo_manager));
        self
    }

    /// Register a custom tool.
    pub fn register_tool(mut self, tool: Box<dyn crate::tools::tool_trait::Tool>) -> Self {
        self.tools = self.tools.register(tool);
        self
    }

    /// Set a custom tool registry.
    pub fn with_tools(mut self, tools: ToolRegistry) -> Self {
        self.tools = tools;
        self
    }

    /// Set an initial conversation history.
    pub fn with_history(mut self, history: Arc<RwLock<Vec<Message>>>) -> Self {
        self.history = Some(history);
        self
    }

    /// Set initial todo state.
    pub fn with_todos(self, todos: Vec<TodoItem>) -> Self {
        if let Ok(mut manager) = self.todo_manager.write() {
            manager.replace_items(todos);
        }
        self
    }

    /// Set a hook registry for the agent.
    pub fn with_hooks(mut self, hooks: HookRegistry) -> Self {
        self.hooks = hooks;
        self
    }

    /// Set a policy for tool approval.
    pub fn with_policy(mut self, policy: Policy) -> Self {
        self.policy = policy;
        self
    }

    pub fn build(self) -> Agent<C> {
        let history = self
            .history
            .unwrap_or_else(|| Arc::new(RwLock::new(Vec::new())));

        Agent {
            client: self.client,
            tools: self.tools,
            config: self.config,
            history,
            todo_manager: self.todo_manager,
            hooks: self.hooks,
            policy: Arc::new(RwLock::new(self.policy)),
        }
    }
}

/// The main agent that orchestrates LLM interaction and tool usage.
#[derive(Clone)]
pub struct Agent<C: LLMClient> {
    client: C,
    tools: ToolRegistry,
    config: Arc<Config>,
    history: Arc<RwLock<Vec<Message>>>,
    todo_manager: Arc<StdRwLock<TodoManager>>,
    hooks: HookRegistry,
    policy: Arc<RwLock<Policy>>,
}

/// Approval channels for bidirectional communication with UI.
pub struct ApprovalChannels {
    /// Channel to send approval requests to UI
    pub request_tx: mpsc::Sender<ApprovalRequest>,
    /// Channel to receive approval decisions from UI
    pub decision_rx: mpsc::Receiver<(String, ApprovalDecision)>,
}

/// Handle for UI to communicate with agent approval system.
pub struct ApprovalHandle {
    /// Channel to receive approval requests from agent
    pub request_rx: mpsc::Receiver<ApprovalRequest>,
    /// Channel to send approval decisions to agent
    pub decision_tx: mpsc::Sender<(String, ApprovalDecision)>,
}

/// Create a pair of approval channels for agent-ui communication.
pub fn create_approval_channels() -> (ApprovalChannels, ApprovalHandle) {
    let (req_tx, req_rx) = mpsc::channel(8);
    let (dec_tx, dec_rx) = mpsc::channel(8);

    (
        ApprovalChannels {
            request_tx: req_tx,
            decision_rx: dec_rx,
        },
        ApprovalHandle {
            request_rx: req_rx,
            decision_tx: dec_tx,
        },
    )
}

impl<C: LLMClient + Clone + 'static> Agent<C> {
    /// Create a new agent with default tools.
    pub fn new(client: C, config: Arc<Config>) -> Self {
        AgentBuilder::new(client, config)
            .with_default_tools()
            .build()
    }

    /// Create an AgentBuilder for custom configuration.
    pub fn builder(client: C, config: Arc<Config>) -> AgentBuilder<C> {
        AgentBuilder::new(client, config)
    }

    pub fn registry(&self) -> &ToolRegistry {
        &self.tools
    }

    pub fn history(&self) -> Arc<RwLock<Vec<Message>>> {
        Arc::clone(&self.history)
    }

    pub fn config(&self) -> Arc<Config> {
        Arc::clone(&self.config)
    }

    /// Get a clone of the LLM client.
    pub fn client(&self) -> C {
        self.client.clone()
    }

    /// Get a clone of the policy for reading.
    pub fn policy(&self) -> Policy {
        let policy = self.policy.blocking_read();
        policy.clone()
    }

    /// Update the policy at runtime.
    pub async fn update_policy<F>(&self, f: F)
    where
        F: FnOnce(&mut Policy),
    {
        let mut policy = self.policy.write().await;
        f(&mut policy);
    }

    /// Add a tool to the auto-approve list.
    pub async fn auto_approve_tool(&self, tool: &str) {
        let mut policy = self.policy.write().await;
        policy.add_auto_approve(tool);
    }

    /// Run with a specific skill.
    ///
    /// The skill's prompt template will be rendered with the user input
    /// as context, and if the skill has tool restrictions, only those
    /// tools will be available.
    pub async fn run_with_skill(
        &self,
        skill: &crate::skills::Skill,
        user_input: &str,
    ) -> Result<RunResult> {
        let prompt = skill.render(user_input);

        // If skill has tool restrictions, filter the tool registry
        let original_tools = self.tools.clone();
        let filtered_tools = if let Some(ref allowed) = skill.allowed_tools {
            original_tools.filter_by_name(allowed)
        } else {
            original_tools
        };

        // Temporarily use filtered tools
        let agent = Agent {
            client: self.client.clone(),
            tools: filtered_tools,
            config: Arc::clone(&self.config),
            history: Arc::clone(&self.history),
            todo_manager: Arc::clone(&self.todo_manager),
            hooks: self.hooks.clone(),
            policy: Arc::clone(&self.policy),
        };

        // Run with the rendered prompt
        agent.run(&prompt).await
    }

    /// Save the current session history to a log file.
    pub async fn save_session(&self, stats: SessionStats) -> Result<Option<PathBuf>> {
        let log_dir = match &self.config.session_log_dir {
            Some(dir) => dir,
            None => return Ok(None),
        };

        if !log_dir.exists() {
            fs::create_dir_all(log_dir).map_err(AgentError::Io)?;
        }

        let timestamp = Local::now().format("%Y%m%d_%H%M%S").to_string();
        let filename = format!("session_{}.json", timestamp);
        let mut path = log_dir.join(&filename);

        let history = self.history.read().await;
        let todos = self
            .todo_manager
            .read()
            .map_err(|_| AgentError::InvalidResponse("Todo state lock poisoned".to_string()))?;
        let session_log = SessionLog {
            timestamp: Local::now().to_rfc3339(),
            model: self.config.model.clone(),
            system_prompt: self.config.system_prompt(),
            history: history.clone(),
            todos: todos.cloned_items(),
            stats,
        };

        let json = serde_json::to_vec_pretty(&session_log).map_err(|e| {
            AgentError::InvalidResponse(format!("Failed to serialize session log: {}", e))
        })?;

        if self.config.session_log_compress {
            path.set_extension("json.gz");
            let file = File::create(&path).map_err(AgentError::Io)?;
            let mut encoder = GzEncoder::new(file, Compression::default());
            encoder.write_all(&json).map_err(AgentError::Io)?;
            encoder.finish().map_err(AgentError::Io)?;
        } else {
            let mut file = File::create(&path).map_err(AgentError::Io)?;
            file.write_all(&json).map_err(AgentError::Io)?;
        }

        info!(path = %path.display(), "Session log saved");
        Ok(Some(path))
    }

    /// List available session logs in the log directory.
    pub fn list_sessions(&self) -> Result<Vec<(PathBuf, SessionLog)>> {
        let log_dir = match &self.config.session_log_dir {
            Some(dir) => dir.clone(),
            None => return Ok(Vec::new()),
        };

        if !log_dir.exists() {
            return Ok(Vec::new());
        }

        let mut sessions = Vec::new();
        let entries = fs::read_dir(&log_dir).map_err(AgentError::Io)?;

        for entry in entries.flatten() {
            let path = entry.path();
            let extension = path.extension().and_then(|e| e.to_str());

            let content = if extension == Some("gz") {
                // Decompress gzipped file
                use flate2::read::GzDecoder;
                use std::io::Read;
                let file = File::open(&path).map_err(AgentError::Io)?;
                let mut decoder = GzDecoder::new(file);
                let mut json = String::new();
                decoder.read_to_string(&mut json).map_err(AgentError::Io)?;
                json
            } else if extension == Some("json") {
                fs::read_to_string(&path).map_err(AgentError::Io)?
            } else {
                continue;
            };

            if let Ok(session) = serde_json::from_str::<SessionLog>(&content) {
                sessions.push((path, session));
            }
        }

        // Sort by timestamp (newest first)
        sessions.sort_by(|a, b| b.1.timestamp.cmp(&a.1.timestamp));
        Ok(sessions)
    }

    /// Load a session log from a file.
    pub fn load_session(path: &PathBuf) -> Result<SessionLog> {
        let extension = path.extension().and_then(|e| e.to_str());

        let content = if extension == Some("gz") {
            use flate2::read::GzDecoder;
            use std::io::Read;
            let file = File::open(path).map_err(AgentError::Io)?;
            let mut decoder = GzDecoder::new(file);
            let mut json = String::new();
            decoder.read_to_string(&mut json).map_err(AgentError::Io)?;
            json
        } else {
            fs::read_to_string(path).map_err(AgentError::Io)?
        };

        serde_json::from_str(&content)
            .map_err(|e| AgentError::InvalidResponse(format!("Failed to parse session log: {}", e)))
    }

    /// Restore history from a session log.
    pub async fn restore_session(&self, session: &SessionLog) {
        let mut history = self.history.write().await;
        *history = session.history.clone();
        let message_count = history.len();
        drop(history);

        if let Ok(mut todos) = self.todo_manager.write() {
            todos.replace_items(session.todos.clone());
        }
        info!(messages = message_count, "Session restored");
    }

    pub fn todos(&self) -> Arc<StdRwLock<TodoManager>> {
        Arc::clone(&self.todo_manager)
    }

    /// Run a single turn with a prompt and return the result.
    #[instrument(skip(self), fields(prompt = %prompt))]
    pub async fn run(&self, prompt: &str) -> Result<RunResult> {
        debug!("Starting agent run");
        let start = Instant::now();

        {
            let mut history_guard = self.history.write().await;
            history_guard.push(Message::user(prompt));
        }

        let mut stream = self.run_stream();
        while let Some(event) = stream.next().await {
            match event? {
                AgentEvent::Done { result } => {
                    let duration_ms = start.elapsed().as_millis() as u64;
                    info!(
                        duration_ms = duration_ms,
                        tool_calls = result.tool_calls.len(),
                        text_len = result.text.len(),
                        "Agent run completed"
                    );

                    // Save session log if configured
                    let stats = SessionStats {
                        total_tokens: 0, // Token counting not yet implemented in all clients
                        tool_calls: result.tool_calls.len(),
                        duration_ms,
                    };
                    let _ = self.save_session(stats).await;

                    return Ok(result);
                }
                AgentEvent::Error { message } => {
                    warn!(error = %message, "Agent run failed");
                    return Err(AgentError::Api(message));
                }
                _ => {}
            }
        }
        Err(AgentError::StreamEndedUnexpectedly)
    }

    /// Run the agent loop as a stream of events.
    /// Note: This version auto-denies any tool that requires approval.
    /// Use `run_stream_with_approval()` for interactive approval flow.
    pub fn run_stream(&self) -> Pin<Box<dyn Stream<Item = Result<AgentEvent>> + Send>> {
        // Run without approval channels - will auto-deny approvals
        self.run_stream_with_approval(None)
    }

    /// Run the agent loop with optional approval channels.
    /// If channels are provided, approval requests will be sent through them.
    /// If not provided, tools requiring approval will be auto-denied.
    pub fn run_stream_with_approval(
        &self,
        mut approval_channels: Option<ApprovalChannels>,
    ) -> Pin<Box<dyn Stream<Item = Result<AgentEvent>> + Send>> {
        let agent = self.clone();
        let client = self.client.clone();
        let tools = self.tools.clone();
        let config = Arc::clone(&self.config);
        let history = Arc::clone(&self.history);
        let hooks = self.hooks.clone();
        let policy = Arc::clone(&self.policy);
        let system = config.system_prompt();
        let tool_schemas: Vec<serde_json::Value> = tools.schemas().into_iter().cloned().collect();

        Box::pin(async_stream::stream! {
            let start = Instant::now();
            let mut total_result = RunResult::default();
            let mut should_continue = true;
            let mut turn_count = 0;

            // Create compactor if auto_compact is enabled
            let compactor = if config.auto_compact {
                Some(ContextCompactor::new(config.to_compaction_config()))
            } else {
                None
            };

            while should_continue {
                turn_count += 1;
                debug!(turn = turn_count, "Starting agent turn");

                // Check if compaction is needed before each turn
                if let Some(ref compactor) = compactor {
                    let history_guard = history.read().await;
                    if compactor.needs_compaction(&history_guard, config.context_window_size) {
                        let context_usage = compactor.context_usage_percent(&history_guard, config.context_window_size);
                        drop(history_guard);

                        info!(
                            context_usage = context_usage,
                            "Context threshold reached, performing compaction"
                        );

                        let mut history_guard = history.write().await;
                        match compactor.compact(&mut history_guard, &client, config.context_window_size).await {
                            Ok(result) => {
                                debug!(
                                    original = result.original_count,
                                    compacted = result.compacted_count,
                                    tokens_saved = result.tokens_saved,
                                    "Compaction complete"
                                );
                                yield Ok(AgentEvent::Compaction {
                                    original_count: result.original_count,
                                    compacted_count: result.compacted_count,
                                    tokens_saved: result.tokens_saved,
                                    messages_summarized: result.messages_summarized,
                                });
                            }
                            Err(e) => {
                                warn!(error = %e, "Compaction failed, continuing with full history");
                            }
                        }
                    }
                }

                let mut stream = {
                    let history_guard = history.read().await;
                    client
                        .create_message_stream(&system, &history_guard, &tool_schemas, 8000)
                        .await?
                };

                let mut tool_uses: Vec<ContentBlock> = Vec::new();
                let mut tool_results: Vec<ContentBlock> = Vec::new();
                let mut current_tool: Option<(String, String, String)> = None;
                let mut turn_text = String::new();
                let mut has_activity_in_turn = false;
                let mut turn_stop_reason = String::new();

                while let Some(event) = stream.next().await {
                    match event? {
                        StreamEvent::TextDelta(text) => {
                            debug!(turn = turn_count, text = %text, "Received TextDelta");
                            has_activity_in_turn = true;
                            turn_text.push_str(&text);
                            yield Ok(AgentEvent::TextDelta { delta: text });
                        }

                        StreamEvent::ThinkingDelta(thinking) => {
                            debug!(turn = turn_count, thinking_len = thinking.len(), "Received ThinkingDelta");
                            has_activity_in_turn = true;
                            yield Ok(AgentEvent::ThinkingDelta { delta: thinking });
                        }

                        StreamEvent::ToolCallStart { id, name } => {
                            debug!(turn = turn_count, tool = %name, id = %id, "Received ToolCallStart");
                            has_activity_in_turn = true;
                            current_tool = Some((id.clone(), name.clone(), String::new()));
                            yield Ok(AgentEvent::ToolStart { id, name });
                        }

                        StreamEvent::ToolCallDelta { arguments } => {
                            if let Some((ref id, _, ref mut input_str)) = current_tool {
                                input_str.push_str(&arguments);
                                yield Ok(AgentEvent::ToolInputDelta {
                                    id: id.clone(),
                                    delta: arguments,
                                });
                            }
                        }

                        StreamEvent::ToolCallDone(_id) => {
                            if let Some((id, name, input_str)) = current_tool.take() {
                                let mut input: serde_json::Value =
                                    serde_json::from_str(&input_str).unwrap_or_else(|_| serde_json::json!({}));

                                // Run on_tool_start hooks
                                let hook_action = hooks.on_tool_start(&name, &input).await;
                                let blocked = match hook_action {
                                    Ok(HookAction::Continue) => false,
                                    Ok(HookAction::ModifyInput(new_input)) => {
                                        debug!(tool = %name, "Hook modified input");
                                        input = new_input;
                                        false
                                    }
                                    Ok(HookAction::Block(reason)) => {
                                        warn!(tool = %name, reason = %reason, "Hook blocked tool execution");
                                        yield Ok(AgentEvent::Error { message: reason.clone() });
                                        true
                                    }
                                    Err(e) => {
                                        warn!(tool = %name, error = %e, "Hook error");
                                        false
                                    }
                                };

                                // Check policy for approval
                                let (needs_approval, reason) = if !blocked {
                                    let policy_guard = policy.read().await;
                                    let needs = policy_guard.needs_approval(&name, &input);
                                    let reason = if needs {
                                        policy_guard.approval_reason(&name, &input)
                                    } else {
                                        String::new()
                                    };
                                    (needs, reason)
                                } else {
                                    (false, String::new())
                                };

                                if !blocked && needs_approval {
                                    // Create approval request
                                    let approval_id = uuid::Uuid::new_v4().to_string();
                                    let request = ApprovalRequest {
                                        id: approval_id.clone(),
                                        tool: name.clone(),
                                        input: input.clone(),
                                        reason: reason.clone(),
                                    };

                                    debug!(tool = %name, approval_id = %approval_id, "Tool requires approval");

                                    // Emit approval required event
                                    yield Ok(AgentEvent::ApprovalRequired {
                                        request: request.clone(),
                                    });

                                    // Try to get approval decision from channels if available
                                    let decision = if let Some(ref mut channels) = approval_channels {
                                        // Send request to UI
                                        if channels.request_tx.send(request.clone()).await.is_ok() {
                                            // Wait for decision from UI with timeout
                                            match tokio::time::timeout(
                                                std::time::Duration::from_secs(300),
                                                channels.decision_rx.recv()
                                            ).await {
                                                Ok(Some((resp_id, dec))) if resp_id == approval_id => {
                                                    Some(dec)
                                                }
                                                _ => None,
                                            }
                                        } else {
                                            None
                                        }
                                    } else {
                                        // No approval channels - auto-deny
                                        debug!(tool = %name, "No approval channels, auto-denying");
                                        None
                                    };

                                    match decision {
                                        Some(ApprovalDecision::Approve) => {
                                            debug!(tool = %name, "Tool approved, executing");
                                            // Fall through to execute
                                        }
                                        Some(ApprovalDecision::AlwaysApprove) => {
                                            debug!(tool = %name, "Tool approved with remember, executing");
                                            // Add to auto-approve list
                                            {
                                                let mut policy_guard = policy.write().await;
                                                policy_guard.add_auto_approve(&name);
                                            }
                                            // Fall through to execute
                                        }
                                        Some(ApprovalDecision::Deny) | None => {
                                            // Denied or timeout/no channels
                                            warn!(tool = %name, "Tool denied or approval timeout");
                                            let output = match decision {
                                                Some(ApprovalDecision::Deny) => format!("Tool execution denied by user: {}", reason),
                                                None => format!("Tool execution requires approval but no approval handler available: {}", reason),
                                                _ => unreachable!(),
                                            };

                                            let record = ToolExecutionRecord::new(
                                                id,
                                                name,
                                                input,
                                                output,
                                                true,
                                            );

                                            yield Ok(record.completion_event());
                                            record_tool_execution(
                                                &mut total_result,
                                                &mut tool_uses,
                                                &mut tool_results,
                                                &record,
                                            );
                                            continue; // Skip execution
                                        }
                                    }

                                    // If approved, execute the tool
                                    let record = Agent::<C>::execute_tool_call(
                                        &tools,
                                        &hooks,
                                        id,
                                        name,
                                        input,
                                    ).await;

                                    yield Ok(record.completion_event());
                                    record_tool_execution(
                                        &mut total_result,
                                        &mut tool_uses,
                                        &mut tool_results,
                                        &record,
                                    );
                                } else if !blocked {
                                    let record = Agent::<C>::execute_tool_call(
                                        &tools,
                                        &hooks,
                                        id,
                                        name,
                                        input,
                                    ).await;

                                    yield Ok(record.completion_event());
                                    record_tool_execution(
                                        &mut total_result,
                                        &mut tool_uses,
                                        &mut tool_results,
                                        &record,
                                    );
                                }
                            }
                        }

                        StreamEvent::StopReason(reason) => {
                            debug!(turn = turn_count, reason = %reason, "Stream stopped");
                            turn_stop_reason = reason;
                        }

                        StreamEvent::TokenUsage { input_tokens, output_tokens } => {
                            let total = input_tokens + output_tokens;
                            debug!(
                                turn = turn_count,
                                input = input_tokens,
                                output = output_tokens,
                                total = total,
                                "Token usage"
                            );
                            yield Ok(AgentEvent::TokenUsage {
                                input_tokens,
                                output_tokens,
                                total_tokens: total,
                            });
                        }
                    }
                }

                // Update history and decide whether to continue
                let mut history_guard = history.write().await;
                if !tool_uses.is_empty() {
                    debug!(turn = turn_count, tool_calls = tool_uses.len(), "Pushing tool results to history, continuing loop");
                    history_guard.push(Message::assistant(tool_uses));
                    history_guard.push(Message::tool_results(tool_results));
                    should_continue = true;
                } else if has_activity_in_turn {
                    // LLM provided text but no tools
                    debug!(turn = turn_count, "Final text provided, ending loop");
                    total_result.text = turn_text.clone();
                    history_guard.push(Message::assistant(vec![ContentBlock::Text {
                        text: turn_text,
                    }]));
                    should_continue = false;
                } else {
                    debug!(turn = turn_count, stop_reason = %turn_stop_reason, "No activity in turn, ending loop");
                    should_continue = false;
                }
                drop(history_guard);
            }

            // After turns complete, save session if logging is enabled
            let stats = SessionStats {
                total_tokens: 0,
                tool_calls: total_result.tool_calls.len(),
                duration_ms: start.elapsed().as_millis() as u64,
            };

            if let Ok(Some(path)) = agent.save_session(stats).await {
                yield Ok(AgentEvent::SessionSaved { path: path.display().to_string() });
            }

            debug!("Yielding Done event");
            yield Ok(AgentEvent::Done { result: total_result });
        })
    }

    /// Run the agent and return an mpsc::Receiver for events.
    pub async fn run_channel(&self) -> Result<mpsc::Receiver<AgentEvent>> {
        let (tx, rx) = mpsc::channel(64);
        let mut stream = self.run_stream();

        tokio::spawn(async move {
            while let Some(event) = stream.next().await {
                match event {
                    Ok(e) => {
                        let is_done = matches!(e, AgentEvent::Done { .. });
                        if tx.send(e).await.is_err() {
                            break;
                        }
                        if is_done {
                            break;
                        }
                    }
                    Err(e) => {
                        let _ = tx
                            .send(AgentEvent::Error {
                                message: e.to_string(),
                            })
                            .await;
                        break;
                    }
                }
            }
        });

        Ok(rx)
    }

    async fn execute_tool_call(
        tools: &ToolRegistry,
        hooks: &HookRegistry,
        id: String,
        name: String,
        input: serde_json::Value,
    ) -> ToolExecutionRecord {
        let tool_start = Instant::now();
        let output = match tools.execute(&name, input.clone()).await {
            Ok(out) => out,
            Err(e) => format!("Error: {}", e),
        };
        let duration_ms = tool_start.elapsed().as_millis() as u64;

        if let Err(e) = hooks.on_tool_complete(&name, &output, duration_ms).await {
            warn!(tool = %name, error = %e, "Hook error on complete");
        }

        debug!(tool = %name, duration_ms = duration_ms, "Tool executed");

        let is_error = output.starts_with("Error:");
        ToolExecutionRecord::new(id, name, input, output, is_error)
    }
}
