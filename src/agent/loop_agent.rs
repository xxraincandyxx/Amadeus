use std::sync::Arc;
use tokio::sync::RwLock;
use crate::agent::messages::{Message, ContentBlock};
use crate::client::anthropic::AnthropicClient;
use crate::tools::{bash::BashTool, schema::bash_tool};
use crate::ui::colors::{print_command, print_tool_result};
use crate::error::Result;

pub struct Agent {
    client: AnthropicClient,
    bash_tool: BashTool,
    workdir: String,
}

impl Agent {
    pub fn new(
        client: AnthropicClient,
        workdir: String,
        timeout_secs: u64,
    ) -> Self {
        let bash_tool = BashTool::new(timeout_secs, workdir.clone());
        Self {
            client,
            bash_tool,
            workdir,
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

            // Collect text content for display
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

            // Execute tool calls
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
}
