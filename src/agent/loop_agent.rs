//! # Agent Loop
//!
//! The main agent loop that drives conversation with the LLM.
//!
//! ## How It Works
//!
//! 1. Receive user prompt and add to history
//! 2. Send history to LLM via client
//! 3. If response is text → display and finish
//! 4. If response is tool call → execute tool and add result to history
//! 5. Repeat from step 2

/*
 * ============================================================================
 * IMPORTS
 * ============================================================================
 */

// Arc = Atomic Reference Counting
// Used for sharing data across multiple owners (multiple tasks/threads)
// Arc is thread-safe (unlike Rc)
use std::sync::Arc;

// RwLock = Reader-Writer Lock
// Allows multiple readers OR one writer (not both at once)
// - read(): Multiple tasks can read simultaneously
// - write(): Only one task can write, no readers allowed
// 
// This is the async version from tokio (not std::sync::RwLock)
// The async version doesn't block the thread while waiting
use tokio::sync::RwLock;

// Our message types
use crate::agent::messages::{ContentBlock, Message};

// The trait that defines LLM operations
use crate::client::LLMClient;

// Event types for streaming
use crate::client::StreamEvent;

// The bash tool for executing commands
use crate::tools::bash::BashTool;

// Function that returns the bash tool schema
use crate::tools::schema::bash_tool;

// UI helper functions for printing
use crate::ui::colors::{print_command, print_tool_result};

// Our Result type
use crate::error::Result;

// StreamExt provides .next() method for streams
// This is a "trait extension" pattern - adds methods to types implementing Stream
use futures::StreamExt;

/*
 * ============================================================================
 * AGENT STRUCT
 * ============================================================================
 */

/// The main agent that orchestrates LLM interaction and tool execution.
///
/// The agent is GENERIC over the LLM client type, allowing different
/// providers to be swapped in.
///
/// # Type Parameter
///
/// * `C` - The LLM client type (must implement `LLMClient`)
///
/// This is like a "template" - the same Agent code works with any client
pub struct Agent<C: LLMClient> {
    /// The LLM client for making API requests
    /// 
    /// We store the client directly (not behind Arc) because:
    /// - Clients are cheap to use (just hold an HTTP client reference)
    /// - We don't share clients between agents
    client: C,
    
    /// Tool for executing bash commands
    /// 
    /// This holds the timeout and working directory settings
    bash_tool: BashTool,
    
    /// Working directory for commands
    /// 
    /// Also used for the system prompt
    workdir: String,
    
    /// Whether to use streaming responses
    /// 
    /// true = real-time text as it's generated
    /// false = wait for complete response
    use_streaming: bool,
}

/*
 * ============================================================================
 * AGENT IMPLEMENTATION
 * ============================================================================
 */

// The impl<C: LLMClient> means:
// - We're implementing methods for Agent<C>
// - C is a type parameter that must implement LLMClient
// - This impl block applies to ANY Agent with ANY LLMClient type
impl<C: LLMClient> Agent<C> {
    /// Create a new agent instance.
    pub fn new(client: C, workdir: String, timeout_secs: u64, use_streaming: bool) -> Self {
        // Create the bash tool with same settings
        let bash_tool = BashTool::new(timeout_secs, workdir.clone());
        
        Self {
            client,
            bash_tool,
            workdir,
            use_streaming,
        }
    }

    /// Run the agent with a user prompt.
    ///
    /// This is the main entry point for agent execution.
    pub async fn run(
        &self,
        prompt: &str,
        // history is wrapped in Arc<RwLock> because:
        // - Arc: Multiple references to the same history
        // - RwLock: Controlled access for reading/writing
        // - This allows the REPL to maintain history across multiple runs
        history: Arc<RwLock<Vec<Message>>>,
    ) -> Result<String> {
        // -----------------------------------------------------------------
        // ADD USER MESSAGE TO HISTORY
        // -----------------------------------------------------------------
        
        // This block creates a scope for the write lock
        // When the block ends, the lock is automatically released
        {
            // Acquire a WRITE lock on history
            // .write() returns a RwLockWriteGuard
            // .await waits for exclusive access (no other readers or writers)
            // 
            // The ? propagates any poisoning error (if a panic occurred while locked)
            let mut history_guard = history.write().await;
            
            // Add the user's prompt as a Message
            // Message::user() creates a Message with role="user"
            history_guard.push(Message::user(prompt));
        }
        // history_guard is dropped here, releasing the write lock
        // 
        // WHY DROP MANUALLY?
        // We need to release the lock before calling run_non_streaming
        // Otherwise, that function couldn't acquire its own read lock

        // Choose mode based on config
        if self.use_streaming {
            self.run_streaming(history).await
        } else {
            self.run_non_streaming(history).await
        }
    }

    /// Run the agent loop in non-streaming mode.
    ///
    /// Wait for complete responses before processing.
    async fn run_non_streaming(
        &self,
        history: Arc<RwLock<Vec<Message>>>,
    ) -> Result<String> {
        // Build the system prompt once
        let system = self.build_system_prompt();
        
        // Get the tool schema
        // This is a Vec because the API expects an array of tools
        let tools = vec![bash_tool()];

        // -----------------------------------------------------------------
        // MAIN AGENT LOOP
        // -----------------------------------------------------------------
        
        // This loop continues until we get a final text response
        // Each iteration:
        // 1. Call LLM
        // 2. If tool_use: execute tool, add result to history, loop
        // 3. If end_turn: return text response
        loop {
            // -------------------------------------------------------------
            // CALL THE LLM
            // -------------------------------------------------------------
            
            // Acquire READ lock to get messages
            let history_guard = history.read().await;
            
            // Call the LLM client
            // create_message returns (stop_reason, content_blocks)
            // 
            // We pass:
            // - &system: The system prompt
            // - &history_guard: Derefs to &[Message]
            // - &tools: The available tools
            // - 8000: Max tokens in response
            let (stop_reason, content) = self
                .client
                .create_message(&system, &history_guard, &tools, 8000)
                .await?;
            
            // Release the read lock explicitly
            // We could also let it drop naturally, but this is clearer
            drop(history_guard);

            // -------------------------------------------------------------
            // EXTRACT TEXT CONTENT
            // -------------------------------------------------------------
            
            // Accumulate text from text blocks
            let mut text_content = String::new();
            
            // Iterate through content blocks
            for block in &content {
                // Pattern match: only process Text blocks
                if let ContentBlock::Text { text } = block {
                    // Print immediately (so user sees progress)
                    print!("{}", text);
                    // Also accumulate for return value
                    text_content.push_str(text);
                }
            }

            // -------------------------------------------------------------
            // ADD ASSISTANT RESPONSE TO HISTORY
            // -------------------------------------------------------------
            
            // Acquire WRITE lock
            let mut history_guard = history.write().await;
            
            // Add the assistant's response
            // Message::assistant() creates Message with role="assistant"
            // content.clone() because we need to use it again below
            history_guard.push(Message::assistant(content.clone()));
            
            drop(history_guard);

            // -------------------------------------------------------------
            // CHECK IF DONE
            // -------------------------------------------------------------
            
            // If stop_reason is NOT "tool_use", we're done
            // This means the LLM finished its response (no more tool calls)
            if stop_reason != "tool_use" {
                return Ok(text_content);
            }

            // -------------------------------------------------------------
            // EXECUTE TOOL CALLS
            // -------------------------------------------------------------
            
            // Collect tool results
            let mut tool_results = Vec::new();
            
            // Iterate through content blocks again
            for block in &content {
                // Pattern match: only process ToolUse blocks
                if let ContentBlock::ToolUse {
                    name,
                    input,
                    id,
                } = block
                {
                    // We only have the "bash" tool
                    if name == "bash" {
                        // Print the command being executed
                        print_command(&input.command);

                        // Execute the command
                        // input is &ToolInput, .execute() takes &ToolInput
                        let output = match self.bash_tool.execute(input).await {
                            Ok(out) => out,
                            Err(e) => format!("Error: {}", e),
                        };

                        // Print the result
                        print_tool_result(&output);

                        // Create a tool result block
                        tool_results.push(ContentBlock::ToolResult {
                            tool_use_id: id.clone(),
                            content: output,
                        });
                    }
                }
            }

            // -------------------------------------------------------------
            // ADD TOOL RESULTS TO HISTORY
            // -------------------------------------------------------------
            
            // Tool results are sent back as a USER message
            // (The LLM sees tool results as coming from "user")
            let mut history_guard = history.write().await;
            history_guard.push(Message {
                role: "user".to_string(),
                content: tool_results,
            });
            drop(history_guard);
            
            // Loop continues - LLM will see the tool results
        }
    }

    /// Run the agent loop in streaming mode.
    ///
    /// Process events as they arrive for real-time feedback.
    async fn run_streaming(
        &self,
        history: Arc<RwLock<Vec<Message>>>,
    ) -> Result<String> {
        let system = self.build_system_prompt();
        let tools = vec![bash_tool()];

        loop {
            // Get a stream from the LLM
            let history_guard = history.read().await;
            let mut stream = self
                .client
                .create_message_stream(&system, &history_guard, &tools, 8000)
                .await?;
            drop(history_guard);

            // Accumulators for this turn
            let mut text_content = String::new();
            let mut tool_calls: Vec<ContentBlock> = Vec::new();
            let mut current_tool: Option<ContentBlock> = None;

            // -------------------------------------------------------------
            // PROCESS STREAM EVENTS
            // -------------------------------------------------------------
            
            // .next() returns Option<Result<StreamEvent>>
            // - Some(Ok(event)): Got an event
            // - Some(Err(e)): Error in stream
            // - None: Stream ended
            while let Some(event) = stream.next().await {
                // The ? propagates any stream error
                match event? {
                    // Text content - print immediately
                    StreamEvent::TextDelta(text) => {
                        print!("{}", text);
                        text_content.push_str(&text);
                    }

                    // Tool call starting
                    StreamEvent::ToolCallStart { id, name } => {
                        println!("Calling tool: {}", name);
                        
                        // Store the partial tool call
                        current_tool = Some(ContentBlock::ToolUse {
                            id,
                            name,
                            // Command will be filled in by deltas
                            input: crate::agent::messages::ToolInput { 
                                command: String::new() 
                            },
                        });
                    }

                    // Tool arguments arriving
                    StreamEvent::ToolCallDelta { arguments } => {
                        // Update the current tool with arguments
                        if let Some(ref mut tool) = current_tool {
                            if let ContentBlock::ToolUse { ref mut input, .. } = tool {
                                // Parse the arguments JSON
                                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&arguments) {
                                    if let Some(cmd) = json.get("command").and_then(|v| v.as_str()) {
                                        input.command = cmd.to_string();
                                    }
                                }
                            }
                        }
                    }

                    // Tool call complete - execute it
                    StreamEvent::ToolCallDone(id) => {
                        // Take ownership of the current tool
                        if let Some(tool) = current_tool.take() {
                            if let ContentBlock::ToolUse { input, .. } = tool {
                                // Execute the command
                                print_command(&input.command);

                                let output = match self.bash_tool.execute(&input).await {
                                    Ok(out) => out,
                                    Err(e) => format!("Error: {}", e),
                                };

                                print_tool_result(&output);

                                // Store result
                                tool_calls.push(ContentBlock::ToolResult {
                                    tool_use_id: id.clone(),
                                    content: output,
                                });
                            }
                        }
                    }

                    // Response finished
                    StreamEvent::StopReason(reason) => {
                        // If not tool_use, we're done
                        if reason != "tool_use" && reason != "tool_calls" {
                            break;
                        }
                    }
                }
            }

            // Update history with this turn
            let mut history_guard = history.write().await;
            history_guard.push(Message::assistant(tool_calls.clone()));
            drop(history_guard);

            // If no tool calls, return the text
            if tool_calls.is_empty() {
                return Ok(text_content);
            }

            // Prepare tool results for next iteration
            let mut tool_results = Vec::new();
            for block in &tool_calls {
                if let ContentBlock::ToolUse { id, .. } = block {
                    tool_results.push(ContentBlock::ToolResult {
                        tool_use_id: id.clone(),
                        content: String::new(),
                    });
                }
            }

            // Add tool results as user message
            let mut history_guard = history.write().await;
            history_guard.push(Message {
                role: "user".to_string(),
                content: tool_results,
            });
            drop(history_guard);
            
            // Loop continues
        }
    }

    /// Build the system prompt with current working directory.
    fn build_system_prompt(&self) -> String {
        format!(
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
        )
    }
}
