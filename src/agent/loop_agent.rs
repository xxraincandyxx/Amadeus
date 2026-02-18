//! # Agent Loop
//!
//! The main agent loop that drives conversation with the LLM.

use std::sync::Arc;

use futures::StreamExt;
use tokio::sync::RwLock;

use crate::agent::config::Config;
use crate::agent::messages::{ContentBlock, Message};
use crate::client::LLMClient;
use crate::client::StreamEvent;
use crate::error::Result;
use crate::tools::bash::BashTool;
use crate::tools::file::{EditFileTool, FileTools, ReadFileTool, WriteFileTool};
use crate::tools::registry::ToolRegistry;
use crate::ui::colors::{print_command, print_tool_result};

pub struct Agent<C: LLMClient> {
    client: C,
    tools: ToolRegistry,
    config: Arc<Config>,
}

impl<C: LLMClient> Agent<C> {
    pub fn new(client: C, config: Arc<Config>) -> Self {
        let tools = ToolRegistry::new()
            .register(Box::new(BashTool::from_config(&config)))
            .register(Box::new(ReadFileTool::new(FileTools::from_config(&config))))
            .register(Box::new(WriteFileTool::new(FileTools::from_config(
                &config,
            ))))
            .register(Box::new(EditFileTool::new(FileTools::from_config(&config))));

        Self {
            client,
            tools,
            config,
        }
    }

    pub fn registry(&self) -> &ToolRegistry {
        &self.tools
    }

    pub async fn run(&self, prompt: &str, history: Arc<RwLock<Vec<Message>>>) -> Result<String> {
        {
            let mut history_guard = history.write().await;
            history_guard.push(Message::user(prompt));
        }

        if self.config.use_streaming {
            self.run_streaming(history).await
        } else {
            self.run_non_streaming(history).await
        }
    }

    async fn run_non_streaming(&self, history: Arc<RwLock<Vec<Message>>>) -> Result<String> {
        let system = self.config.system_prompt();
        let tools: Vec<serde_json::Value> = self.tools.schemas().into_iter().cloned().collect();

        loop {
            let (stop_reason, content) = {
                let history_guard = history.read().await;
                self.client
                    .create_message(&system, &history_guard, &tools, 8000)
                    .await?
            };

            let mut text_content = String::new();

            for block in &content {
                if let ContentBlock::Text { text } = block {
                    print!("{}", text);
                    text_content.push_str(text);
                }
            }

            if stop_reason != "tool_use" {
                let mut history_guard = history.write().await;
                history_guard.push(Message::assistant(content));
                drop(history_guard);
                return Ok(text_content);
            }

            let tool_results = self.execute_tools(&content).await;

            let mut history_guard = history.write().await;
            history_guard.push(Message::assistant(content));
            history_guard.push(Message::tool_results(tool_results));
            drop(history_guard);
        }
    }

    async fn execute_tools(&self, content: &[ContentBlock]) -> Vec<ContentBlock> {
        let mut tool_results = Vec::new();

        for block in content {
            if let ContentBlock::ToolUse { name, input, id } = block {
                print_command(&format!("{}: {:?}", name, input));

                let output = match self.tools.execute(name, input.clone()).await {
                    Ok(out) => out,
                    Err(e) => format!("Error: {}", e),
                };

                print_tool_result(&output);

                tool_results.push(ContentBlock::ToolResult {
                    tool_use_id: id.clone(),
                    content: output,
                });
            }
        }

        tool_results
    }

    async fn run_streaming(&self, history: Arc<RwLock<Vec<Message>>>) -> Result<String> {
        let system = self.config.system_prompt();
        let tools: Vec<serde_json::Value> = self.tools.schemas().into_iter().cloned().collect();

        loop {
            let mut stream = {
                let history_guard = history.read().await;
                self.client
                    .create_message_stream(&system, &history_guard, &tools, 8000)
                    .await?
            };

            let mut text_content = String::new();
            let mut tool_uses: Vec<ContentBlock> = Vec::new();
            let mut tool_results: Vec<ContentBlock> = Vec::new();
            let mut current_tool: Option<(String, String, String)> = None;

            while let Some(event) = stream.next().await {
                match event? {
                    StreamEvent::TextDelta(text) => {
                        print!("{}", text);
                        text_content.push_str(&text);
                    }

                    StreamEvent::ToolCallStart { id, name } => {
                        current_tool = Some((id, name, String::new()));
                    }

                    StreamEvent::ToolCallDelta { arguments } => {
                        if let Some((_, _, ref mut input_str)) = current_tool {
                            input_str.push_str(&arguments);
                        }
                    }

                    StreamEvent::ToolCallDone(_id) => {
                        if let Some((id, name, input_str)) = current_tool.take() {
                            let input: serde_json::Value =
                                serde_json::from_str(&input_str).unwrap_or(serde_json::Value::Null);

                            print_command(&format!("{}: {:?}", name, input));

                            let output = match self.tools.execute(&name, input.clone()).await {
                                Ok(out) => out,
                                Err(e) => format!("Error: {}", e),
                            };

                            print_tool_result(&output);

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
                            let mut history_guard = history.write().await;
                            if !text_content.is_empty() || !tool_uses.is_empty() {
                                let mut assistant_content = Vec::new();
                                if !text_content.is_empty() {
                                    assistant_content.push(ContentBlock::Text {
                                        text: text_content.clone(),
                                    });
                                }
                                assistant_content.extend(tool_uses.clone());
                                history_guard.push(Message::assistant(assistant_content));
                            }
                            drop(history_guard);
                            return Ok(text_content);
                        }
                    }
                }
            }

            let mut history_guard = history.write().await;
            let has_tool_results = !tool_results.is_empty();
            if !tool_uses.is_empty() {
                history_guard.push(Message::assistant(tool_uses));
                history_guard.push(Message::tool_results(tool_results));
            } else if !text_content.is_empty() {
                history_guard.push(Message::assistant(vec![ContentBlock::Text {
                    text: text_content.clone(),
                }]));
            }
            drop(history_guard);

            if !has_tool_results {
                return Ok(text_content);
            }
        }
    }
}
