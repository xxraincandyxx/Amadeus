use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Instant;

use chrono::Local;
use flate2::write::GzEncoder;
use flate2::Compression;
use futures::{Stream, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, info, instrument, warn};

use crate::agent::config::Config;
use crate::agent::events::{AgentEvent, RunResult, ToolCall};
use crate::agent::messages::{ContentBlock, Message};
use crate::client::{LLMClient, StreamEvent};
use crate::error::{AgentError, Result};
use crate::tools::bash::BashTool;
use crate::tools::file::{EditFileTool, FileTools, ReadFileTool, WriteFileTool};
use crate::tools::registry::ToolRegistry;

/// A log of a single conversation session.
#[derive(Debug, Serialize, Deserialize)]
pub struct SessionLog {
    pub timestamp: String,
    pub model: String,
    pub system_prompt: String,
    pub history: Vec<Message>,
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
}

impl<C: LLMClient + Clone + 'static> AgentBuilder<C> {
    pub fn new(client: C, config: Arc<Config>) -> Self {
        Self {
            client,
            config,
            tools: ToolRegistry::new(),
            history: None,
        }
    }

    /// Add default tools (bash, file operations) to the agent.
    pub fn with_default_tools(mut self) -> Self {
        self.tools = self
            .tools
            .register(Box::new(BashTool::from_config(&self.config)))
            .register(Box::new(ReadFileTool::new(FileTools::from_config(
                &self.config,
            ))))
            .register(Box::new(WriteFileTool::new(FileTools::from_config(
                &self.config,
            ))))
            .register(Box::new(EditFileTool::new(FileTools::from_config(
                &self.config,
            ))));
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

    pub fn build(self) -> Agent<C> {
        let history = self
            .history
            .unwrap_or_else(|| Arc::new(RwLock::new(Vec::new())));

        Agent {
            client: self.client,
            tools: self.tools,
            config: self.config,
            history,
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
        let session_log = SessionLog {
            timestamp: Local::now().to_rfc3339(),
            model: self.config.model.clone(),
            system_prompt: self.config.system_prompt(),
            history: history.clone(),
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
    pub fn run_stream(&self) -> Pin<Box<dyn Stream<Item = Result<AgentEvent>> + Send>> {
        let agent = self.clone();
        let client = self.client.clone();
        let tools = self.tools.clone();
        let config = Arc::clone(&self.config);
        let history = Arc::clone(&self.history);
        let system = config.system_prompt();
        let tool_schemas: Vec<serde_json::Value> = tools.schemas().into_iter().cloned().collect();

        Box::pin(async_stream::stream! {
            let start = Instant::now();
            let mut total_result = RunResult::default();
            let mut should_continue = true;
            let mut turn_count = 0;

            while should_continue {
                turn_count += 1;
                debug!(turn = turn_count, "Starting agent turn");

                let mut stream = {
                    let history_guard = history.read().await;
                    client
                        .create_message_stream(&system, &history_guard, &tool_schemas, 8000)
                        .await?
                };

                let mut tool_uses: Vec<ContentBlock> = Vec::new();
                let mut tool_results: Vec<ContentBlock> = Vec::new();
                let mut current_tool: Option<(String, String, String)> = None;
                let mut has_activity_in_turn = false;
                let mut turn_stop_reason = String::new();

                while let Some(event) = stream.next().await {
                    match event? {
                        StreamEvent::TextDelta(text) => {
                            debug!(turn = turn_count, text = %text, "Received TextDelta");
                            has_activity_in_turn = true;
                            total_result.text.push_str(&text);
                            yield Ok(AgentEvent::TextDelta { delta: text });
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
                                let input: serde_json::Value =
                                    serde_json::from_str(&input_str).unwrap_or_else(|_| serde_json::json!({}));

                                let tool_start = Instant::now();
                                let output = match tools.execute(&name, input.clone()).await {
                                    Ok(out) => out,
                                    Err(e) => format!("Error: {}", e),
                                };
                                let duration_ms = tool_start.elapsed().as_millis() as u64;

                                debug!(
                                    tool = %name,
                                    duration_ms = duration_ms,
                                    "Tool executed"
                                );

                                yield Ok(AgentEvent::ToolComplete {
                                    id: id.clone(),
                                    name: name.clone(),
                                    input: input.clone(),
                                    output: output.clone(),
                                    is_error: output.starts_with("Error:"),
                                });

                                total_result.tool_calls.push(ToolCall {
                                    name: name.clone(),
                                    input: input.clone(),
                                    output: output.clone(),
                                    is_error: output.starts_with("Error:"),
                                });

                                tool_uses.push(ContentBlock::ToolUse {
                                    id: id.clone(),
                                    name,
                                    input,
                                });

                                tool_results.push(ContentBlock::ToolResult {
                                    tool_use_id: id,
                                    content: output,
                                });
                            }
                        }

                        StreamEvent::StopReason(reason) => {
                            debug!(turn = turn_count, reason = %reason, "Stream stopped");
                            turn_stop_reason = reason;
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
                    history_guard.push(Message::assistant(vec![ContentBlock::Text {
                        text: total_result.text.clone(),
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
}
