use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;

use futures::{Stream, StreamExt};
use tokio::sync::{mpsc, RwLock};

use crate::agent::agent_config::{AgentConfig, AgentStats, AgentStatus};
use crate::agent::events::{AgentEvent, RunResult, ToolCall};
use crate::agent::messages::{ContentBlock, Message};
use crate::client::{LLMClient, StreamEvent};
use crate::core::event::Event;
use crate::core::id::AgentId;
use crate::core::Workspace;
use crate::error::Result;
use crate::tools::bash::BashTool;
use crate::tools::file::{EditFileTool, FileTools, ReadFileTool, WriteFileTool};
use crate::tools::registry::ToolRegistry;

pub struct Agent<C: LLMClient> {
    id: AgentId,
    client: C,
    tools: ToolRegistry,
    config: AgentConfig,
    workspace: Arc<RwLock<Workspace>>,
    status: AgentStatus,
    stats: AgentStats,
    local_state: HashMap<String, serde_json::Value>,
}

impl<C: LLMClient + Clone + 'static> Agent<C> {
    pub fn new(client: C, config: AgentConfig, workspace: Arc<RwLock<Workspace>>) -> Self {
        let id = config.id.unwrap_or_default();

        let tools = ToolRegistry::new()
            .register(Box::new(BashTool::new(
                config.timeout_seconds,
                config.workdir.to_string_lossy().to_string(),
                config.blocked_commands.clone(),
                config.max_output_bytes,
            )))
            .register(Box::new(ReadFileTool::new(FileTools::new(
                config.workdir.clone(),
                config.max_output_bytes,
            ))))
            .register(Box::new(WriteFileTool::new(FileTools::new(
                config.workdir.clone(),
                config.max_output_bytes,
            ))))
            .register(Box::new(EditFileTool::new(FileTools::new(
                config.workdir.clone(),
                config.max_output_bytes,
            ))));

        let filtered_tools = if let Some(ref allowed) = config.tools {
            tools.filter_by_name(allowed)
        } else {
            tools
        };

        Self {
            id,
            client,
            tools: filtered_tools,
            config,
            workspace,
            status: AgentStatus::Idle,
            stats: AgentStats::default(),
            local_state: HashMap::new(),
        }
    }

    pub fn id(&self) -> AgentId {
        self.id
    }

    pub fn status(&self) -> AgentStatus {
        self.status
    }

    pub fn stats(&self) -> &AgentStats {
        &self.stats
    }

    pub fn config(&self) -> &AgentConfig {
        &self.config
    }

    pub fn registry(&self) -> &ToolRegistry {
        &self.tools
    }

    pub async fn run(&mut self, prompt: &str) -> Result<RunResult> {
        self.status = AgentStatus::Thinking;

        let history = self.get_or_create_history().await;
        {
            let mut history_guard = history.write().await;
            history_guard.push(Message::user(prompt));
        }

        let mut stream = self.run_stream(history.clone());
        while let Some(event) = stream.next().await {
            match event? {
                AgentEvent::Done { result } => {
                    self.status = AgentStatus::Idle;
                    return Ok(result);
                }
                AgentEvent::Error { message } => {
                    self.status = AgentStatus::Idle;
                    return Err(crate::error::AgentError::Api(message));
                }
                _ => {}
            }
        }

        self.status = AgentStatus::Idle;
        Err(crate::error::AgentError::StreamEndedUnexpectedly)
    }

    pub fn run_stream(
        &self,
        history: Arc<RwLock<Vec<Message>>>,
    ) -> Pin<Box<dyn Stream<Item = Result<AgentEvent>> + Send>> {
        let client = self.client.clone();
        let tools = self.tools.clone();
        let system = self.config.get_system_prompt();
        let tool_schemas: Vec<serde_json::Value> = tools.schemas().into_iter().cloned().collect();
        let workspace = self.workspace.clone();
        let agent_id = self.id;

        Box::pin(async_stream::stream! {
            let mut result = RunResult::default();
            let mut should_continue = true;
            let mut tool_call_count = 0;
            let max_tool_calls = 100;

            while should_continue && tool_call_count < max_tool_calls {
                let mut stream = {
                    let history_guard = history.read().await;
                    client
                        .create_message_stream(&system, &history_guard, &tool_schemas, 8000)
                        .await?
                };

                let mut tool_uses: Vec<ContentBlock> = Vec::new();
                let mut tool_results: Vec<ContentBlock> = Vec::new();
                let mut current_tool: Option<(String, String, String)> = None;

                while let Some(event) = stream.next().await {
                    match event? {
                        StreamEvent::TextDelta(text) => {
                            result.text.push_str(&text);
                            yield Ok(AgentEvent::TextDelta { delta: text });
                        }

                        StreamEvent::ToolCallStart { id, name } => {
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
                                    serde_json::from_str(&input_str).unwrap_or(serde_json::Value::Null);

                                {
                                    let mut ws = workspace.write().await;
                                    ws.append_event(Event::ToolCallStart {
                                        agent: agent_id,
                                        tool: name.clone(),
                                        args: input.clone(),
                                    });
                                }

                                let start = std::time::Instant::now();
                                let output = match tools.execute(&name, input.clone()).await {
                                    Ok(out) => out,
                                    Err(e) => format!("Error: {}", e),
                                };
                                let duration_ms = start.elapsed().as_millis() as u64;

                                let is_error = output.starts_with("Error:");

                                {
                                    let mut ws = workspace.write().await;
                                    if is_error {
                                        ws.append_event(Event::ToolCallError {
                                            agent: agent_id,
                                            tool: name.clone(),
                                            error: output.clone(),
                                        });
                                    } else {
                                        ws.append_event(Event::ToolCallComplete {
                                            agent: agent_id,
                                            tool: name.clone(),
                                            result: serde_json::json!(&output),
                                            duration_ms,
                                        });
                                    }
                                }

                                yield Ok(AgentEvent::ToolComplete {
                                    id: id.clone(),
                                    name: name.clone(),
                                    input: input.clone(),
                                    output: output.clone(),
                                    is_error,
                                });

                                result.tool_calls.push(ToolCall {
                                    name: name.clone(),
                                    input: input.clone(),
                                    output: output.clone(),
                                    is_error,
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

                                tool_call_count += 1;
                            }
                        }

                        StreamEvent::StopReason(reason) => {
                            if reason != "tool_use" && reason != "tool_calls" {
                                should_continue = false;
                            }
                        }
                    }
                }

                let mut history_guard = history.write().await;
                if !tool_uses.is_empty() {
                    history_guard.push(Message::assistant(tool_uses));
                    history_guard.push(Message::tool_results(tool_results));
                } else if !result.text.is_empty() {
                    history_guard.push(Message::assistant(vec![ContentBlock::Text {
                        text: result.text.clone(),
                    }]));
                    should_continue = false;
                } else {
                    should_continue = false;
                }
                drop(history_guard);
            }

            yield Ok(AgentEvent::Done { result });
        })
    }

    pub async fn run_channel(
        &self,
        history: Arc<RwLock<Vec<Message>>>,
    ) -> Result<mpsc::Receiver<AgentEvent>> {
        let (tx, rx) = mpsc::channel(64);
        let mut stream = self.run_stream(history);

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

    async fn get_or_create_history(&self) -> Arc<RwLock<Vec<Message>>> {
        let ws = self.workspace.read().await;
        let state = ws.state();
        let history_key = format!("agent.{}.history", self.id);

        if let Some(history_json) = state.read(&history_key) {
            if let Ok(messages) = serde_json::from_value::<Vec<Message>>(history_json.clone()) {
                return Arc::new(RwLock::new(messages));
            }
        }

        Arc::new(RwLock::new(Vec::new()))
    }

    pub async fn save_history(&self, history: &[Message]) -> Result<()> {
        let mut ws = self.workspace.write().await;
        let history_key = format!("agent.{}.history", self.id);
        let history_json =
            serde_json::to_value(history).map_err(crate::error::AgentError::Serde)?;
        ws.state_mut().write(&history_key, history_json);
        Ok(())
    }

    pub fn local_state(&self) -> &HashMap<String, serde_json::Value> {
        &self.local_state
    }

    pub fn local_state_mut(&mut self) -> &mut HashMap<String, serde_json::Value> {
        &mut self.local_state
    }
}
