use std::sync::Arc;
use tokio::sync::RwLock;
use crate::agent::messages::{Message, ContentBlock};
use crate::client::LLMClient;
use crate::client::StreamEvent;
use crate::tools::{bash::BashTool, schema::bash_tool};
use crate::ui::colors::{print_command, print_tool_result};
use crate::error::Result;
use futures::StreamExt;

pub struct Agent<C: LLMClient> {
    client: C,
    bash_tool: BashTool,
    workdir: String,
    use_streaming: bool,
}

impl<C: LLMClient> Agent<C> {
    pub fn new(client: C, workdir: String, timeout_secs: u64, use_streaming: bool) -> Self {
        let bash_tool = BashTool::new(timeout_secs, workdir.clone());
        Self {
            client,
            bash_tool,
            workdir,
            use_streaming,
        }
    }

    pub async fn run(
        &self,
        prompt: &str,
        history: Arc<RwLock<Vec<Message>>>,
    ) -> Result<String> {
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

    async fn run_non_streaming(
        &self,
        history: Arc<RwLock<Vec<Message>>>,
    ) -> Result<String> {
        let system = format!(
            "You are a CLI agent at {}. Solve problems using bash commands.\n\n\
             Rules:\n\
             - Prefer tools over prose. Act first, explain briefly after.\n\
             - Read files: cat, grep, find, rg, ls, head, tail\n\
             - Write files: echo '...' > file, sed -i, or cat << 'EOF' > file\n\
             - Subagent: For complex subtasks, spawn a subagent to keep context clean:\n\
               cargo run -- 'explore src/ and summarize'\n\n\
             When to use subagent:\n\
             - Task requires reading many files (isolate exploration)\n\
             - Task is independent and self-contained\n\
             - You want to avoid polluting current conversation with intermediate details\n\n\
             The subagent runs in isolation and returns only its final summary.",
            self.workdir
        );

        let tools = vec![bash_tool()];

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

            let mut tool_results = Vec::new();
            for block in &content {
                if let ContentBlock::ToolUse {
                    name,
                    input,
                    id,
                } = block
                {
                    if name == "bash" {
                        print_command(&input.command);

                        let output = match self.bash_tool.execute(input).await {
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
            history_guard.push(Message {
                role: "user".to_string(),
                content: tool_results,
            });
            drop(history_guard);
        }
    }

    async fn run_streaming(
        &self,
        history: Arc<RwLock<Vec<Message>>>,
    ) -> Result<String> {
        let system = format!(
            "You are a CLI agent at {}. Solve problems using bash commands.\n\n\
             Rules:\n\
             - Prefer tools over prose. Act first, explain briefly after.\n\
             - Read files: cat, grep, find, rg, ls, head, tail\n\
             - Write files: echo '...' > file, sed -i, or cat << 'EOF' > file\n\
             - Subagent: For complex subtasks, spawn a subagent to keep context clean:\n\
               cargo run -- 'explore src/ and summarize'\n\n\
             When to use subagent:\n\
             - Task requires reading many files (isolate exploration)\n\
             - Task is independent and self-contained\n\
             - You want to avoid polluting current conversation with intermediate details\n\n\
             The subagent runs in isolation and returns only its final summary.",
            self.workdir
        );

        let tools = vec![bash_tool()];

        loop {
            let history_guard = history.read().await;
            let mut stream = self
                .client
                .create_message_stream(&system, &history_guard, &tools, 8000)
                .await?;
            drop(history_guard);

            let mut text_content = String::new();
            let mut tool_calls: Vec<ContentBlock> = Vec::new();
            let mut current_tool: Option<ContentBlock> = None;

            while let Some(event) = stream.next().await {
                match event? {
                    StreamEvent::TextDelta(text) => {
                        print!("{}", text);
                        text_content.push_str(&text);
                    }
                    StreamEvent::ToolCallStart { id, name } => {
                        println!("Calling tool: {}", name);
                        current_tool = Some(ContentBlock::ToolUse {
                            id,
                            name,
                            input: serde_json::json!({"command": ""}),
                        });
                    }
                    StreamEvent::ToolCallDelta { arguments } => {
                        if let Some(ref mut tool) = current_tool {
                            if let ContentBlock::ToolUse { ref mut input, .. } = tool {
                                if let Ok(json) = serde_json::from_str::<serde_json::Value>(arguments) {
                                    *input = serde_json::from_value(json)
                                        .unwrap_or_else(|_| serde_json::json!({"command": ""}));
                                }
                            }
                        }
                    }
                    StreamEvent::ToolCallDone(id) => {
                        if let Some(tool) = current_tool.take() {
                            if let ContentBlock::ToolUse { ref input, .. } = tool {
                                print_command(&input["command"].as_str().unwrap_or(""));

                                let output = match self.bash_tool.execute(input).await {
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

            let mut tool_results = Vec::new();
            for block in &tool_calls {
                if let ContentBlock::ToolUse { id, .. } = block {
                    tool_results.push(ContentBlock::ToolResult {
                        tool_use_id: id.clone(),
                        content: String::new(),
                    });
                }
            }

            let mut history_guard = history.write().await;
            history_guard.push(Message {
                role: "user".to_string(),
                content: tool_results,
            });
            drop(history_guard);
        }
    }
}
