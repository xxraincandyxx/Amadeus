use std::pin::Pin;
use std::sync::Arc;
use std::time::Instant;

use futures::{Stream, StreamExt};
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, info, instrument, warn};

use crate::agent::config::Config;
use crate::agent::events::{AgentEvent, RunResult, ToolCall};
use crate::agent::messages::{ContentBlock, Message};
use crate::client::{LLMClient, StreamEvent};
use crate::error::Result;
use crate::tools::bash::BashTool;
use crate::tools::file::{EditFileTool, FileTools, ReadFileTool, WriteFileTool};
use crate::tools::registry::ToolRegistry;

#[derive(Clone)]
pub struct Agent<C: LLMClient> {
    client: C,
    tools: ToolRegistry,
    config: Arc<Config>,
}

impl<C: LLMClient + Clone + 'static> Agent<C> {
    pub fn new(client: C, config: Arc<Config>) -> Self {
        let tools = ToolRegistry::new()
            .register(Box::new(BashTool::from_config(&config)))
            .register(Box::new(ReadFileTool::new(FileTools::from_config(&config))))
            .register(Box::new(WriteFileTool::new(FileTools::from_config(
                &config,
            ))))
            .register(Box::new(EditFileTool::new(FileTools::from_config(&config))));

        info!(
            model = %config.model,
            workdir = %config.workdir.display(),
            "Agent initialized"
        );

        Self {
            client,
            tools,
            config,
        }
    }

    pub fn registry(&self) -> &ToolRegistry {
        &self.tools
    }

    #[instrument(skip(self, history), fields(prompt = %prompt))]
    pub async fn run(&self, prompt: &str, history: Arc<RwLock<Vec<Message>>>) -> Result<RunResult> {
        debug!("Starting agent run");
        let start = Instant::now();

        {
            let mut history_guard = history.write().await;
            history_guard.push(Message::user(prompt));
        }

        let mut stream = self.run_stream(history.clone());
        while let Some(event) = stream.next().await {
            match event? {
                AgentEvent::Done { result } => {
                    info!(
                        duration_ms = start.elapsed().as_millis() as u64,
                        tool_calls = result.tool_calls.len(),
                        text_len = result.text.len(),
                        "Agent run completed"
                    );
                    return Ok(result);
                }
                AgentEvent::Error { message } => {
                    warn!(error = %message, "Agent run failed");
                    return Err(crate::error::AgentError::Api(message));
                }
                _ => {}
            }
        }
        Err(crate::error::AgentError::StreamEndedUnexpectedly)
    }

    pub fn run_stream(
        &self,
        history: Arc<RwLock<Vec<Message>>>,
    ) -> Pin<Box<dyn Stream<Item = Result<AgentEvent>> + Send>> {
        let client = self.client.clone();
        let tools = self.tools.clone();
        let config = Arc::clone(&self.config);
        let system = config.system_prompt();
        let tool_schemas: Vec<serde_json::Value> = tools.schemas().into_iter().cloned().collect();

        Box::pin(async_stream::stream! {
            let mut result = RunResult::default();
            let mut should_continue = true;

            while should_continue {
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

                                let tool_start = Instant::now();
                                let output = match tools.execute(&name, input.clone()).await {
                                    Ok(out) => out,
                                    Err(e) => format!("Error: {}", e),
                                };
                                let duration_ms = tool_start.elapsed().as_millis() as u64;

                                let is_error = output.starts_with("Error:");

                                info!(
                                    tool = %name,
                                    duration_ms = duration_ms,
                                    is_error = is_error,
                                    "Tool executed"
                                );

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
}
