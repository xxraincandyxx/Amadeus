//! # Agent Loop
//!
//! The main agent loop that drives conversation with the LLM.

use std::path::PathBuf;
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
use crate::tools::schema::all_tools;
use crate::tools::tool_trait::Tool;
use crate::ui::colors::{print_command, print_tool_result};

pub struct Agent<C: LLMClient> {
    client: C,
    tools: Vec<Box<dyn Tool>>,
    workdir: PathBuf,
    use_streaming: bool,
}

impl<C: LLMClient> Agent<C> {
    pub fn new(client: C, config: &Config) -> Self {
        let workdir = config.workdir.clone();

        let file_tools = FileTools::new(workdir.clone(), config.max_output_bytes);

        let tools: Vec<Box<dyn Tool>> = vec![
            Box::new(BashTool::new(
                config.timeout_seconds,
                workdir.to_string_lossy().to_string(),
                config.blocked_commands.clone(),
                config.max_output_bytes,
            )),
            Box::new(ReadFileTool::new(file_tools.clone())),
            Box::new(WriteFileTool::new(file_tools.clone())),
            Box::new(EditFileTool::new(file_tools)),
        ];

        Self {
            client,
            tools,
            workdir,
            use_streaming: config.use_streaming,
        }
    }

    pub async fn run(&self, prompt: &str, history: Arc<RwLock<Vec<Message>>>) -> Result<String> {
        {
            let mut history_guard = history.write().await;
            history_guard.push(Message::user(prompt));
        }

        if self.use_streaming {
            self.run_streaming(history).await
        } else {
            self.run_non_streaming(history).await
        }
    }

    async fn run_non_streaming(&self, history: Arc<RwLock<Vec<Message>>>) -> Result<String> {
        let system = self.build_system_prompt();
        let tools = all_tools();

        loop {
            let history_guard = history.read().await;

            let (stop_reason, content) = self
                .client
                .create_message(&system, &history_guard, &tools, 8000)
                .await?;

            drop(history_guard);

            let mut text_content = String::new();

            for block in &content {
                if let ContentBlock::Text { text } = block {
                    print!("{}", text);
                    text_content.push_str(text);
                }
            }

            let mut history_guard = history.write().await;
            history_guard.push(Message::assistant(content.clone()));
            drop(history_guard);

            if stop_reason != "tool_use" {
                return Ok(text_content);
            }

            let tool_count = content
                .iter()
                .filter(|b| matches!(b, ContentBlock::ToolUse { .. }))
                .count();

            let mut tool_results = Vec::with_capacity(tool_count);

            for block in &content {
                if let ContentBlock::ToolUse { name, input, id } = block {
                    if let Some(tool) = self.tools.iter().find(|t| t.name() == name) {
                        print_command(&format!("{}: {:?}", name, input));

                        let output = match tool.execute(input.clone()).await {
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
            }

            let mut history_guard = history.write().await;
            history_guard.push(Message::tool_results(tool_results));
            drop(history_guard);
        }
    }

    async fn run_streaming(&self, history: Arc<RwLock<Vec<Message>>>) -> Result<String> {
        let system = self.build_system_prompt();
        let tools = all_tools();

        loop {
            let history_guard = history.read().await;
            let mut stream = self
                .client
                .create_message_stream(&system, &history_guard, &tools, 8000)
                .await?;
            drop(history_guard);

            let mut text_content = String::new();
            let mut tool_calls: Vec<ContentBlock> = Vec::new();
            let mut current_tool: Option<(String, String, serde_json::Value)> = None;

            while let Some(event) = stream.next().await {
                match event? {
                    StreamEvent::TextDelta(text) => {
                        print!("{}", text);
                        text_content.push_str(&text);
                    }

                    StreamEvent::ToolCallStart { id, name } => {
                        println!("Calling tool: {}", name);
                        current_tool = Some((id, name, serde_json::Value::Null));
                    }

                    StreamEvent::ToolCallDelta { arguments } => {
                        if let Some((_, _, ref mut input)) = current_tool {
                            *input =
                                serde_json::from_str(&arguments).unwrap_or(serde_json::Value::Null);
                        }
                    }

                    StreamEvent::ToolCallDone(_id) => {
                        if let Some((id, name, input)) = current_tool.take() {
                            if let Some(tool) = self.tools.iter().find(|t| t.name() == name) {
                                print_command(&format!("{}: {:?}", name, input));

                                let output = match tool.execute(input.clone()).await {
                                    Ok(out) => out,
                                    Err(e) => format!("Error: {}", e),
                                };

                                print_tool_result(&output);

                                tool_calls.push(ContentBlock::ToolResult {
                                    tool_use_id: id.clone(),
                                    content: output,
                                });
                            }
                        }
                    }

                    StreamEvent::StopReason(reason) => {
                        if reason != "tool_use" && reason != "tool_calls" {
                            break;
                        }
                    }
                }
            }

            let mut history_guard = history.write().await;
            history_guard.push(Message::assistant(tool_calls.clone()));
            drop(history_guard);

            if tool_calls.is_empty() {
                return Ok(text_content);
            }

            let tool_results: Vec<ContentBlock> = tool_calls
                .into_iter()
                .filter_map(|block| {
                    if let ContentBlock::ToolResult {
                        tool_use_id,
                        content,
                    } = block
                    {
                        Some(ContentBlock::ToolResult {
                            tool_use_id,
                            content,
                        })
                    } else {
                        None
                    }
                })
                .collect();

            let mut history_guard = history.write().await;
            history_guard.push(Message::tool_results(tool_results));
            drop(history_guard);
        }
    }

    fn build_system_prompt(&self) -> String {
        format!(
            "You are a CLI agent at {}.\n\n\
             Loop: think briefly -> use tools -> report results.\n\n\
             Rules:\n\
             - Prefer tools over prose. Act, don't just explain.\n\
             - Never invent file paths. Use bash ls/find first if unsure.\n\
             - Make minimal changes. Don't over-engineer.\n\
             - After finishing, summarize what changed.\n\n\
             Available Tools:\n\
             - bash: Run shell commands (git, npm, python, ls, grep, etc.)\n\
             - read_file: Read file contents (use for understanding code)\n\
             - write_file: Create or overwrite files (use for new files)\n\
             - edit_file: Make surgical changes to existing files\n\n\
             When to use each tool:\n\
             - bash: For system commands, searching, running tests\n\
             - read_file: When you need to see file contents\n\
             - write_file: When creating new files or complete rewrites\n\
             - edit_file: When making precise changes to existing files",
            self.workdir.display()
        )
    }
}
