# Amadeus - AI Coding Agent (Rust Implementation)

Amadeus is a Rust-based AI coding agent implementing the v0 "Bash is All You Need" philosophy from the learn-claude-code project.

## Overview

This agent demonstrates that **one tool is enough** for a fully functional AI coding agent. With just bash, the model can:
- Read files (`cat`, `grep`, `head`)
- Write files (`echo > file`, `sed`)
- Execute any command (`python`, `npm`, `make`)
- Spawn subagents via recursive process calls

## Features

### 1. Multi-Provider Support with Generic LLMClient Trait (src/client/mod.rs)

The agent now supports multiple AI providers through a generic trait abstraction:

```rust
#[async_trait]
pub trait LLMClient: Send + Sync {
    async fn create_message(
        &self,
        system: &str,
        messages: &[Message],
        tools: &[serde_json::Value],
        max_tokens: u32,
    ) -> Result<(String, Vec<ContentBlock>)>;

    async fn create_message_stream(
        &self,
        system: &str,
        messages: &[Message],
        tools: &[serde_json::Value],
        max_tokens: u32,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamEvent>> + Send>>>;
}
```

**Key Design Decisions:**
- **Generic over trait object**: `Agent<C: LLMClient>` uses zero-cost abstraction, resolved at compile time
- **Single internal format**: Transformations happen only at provider boundaries
- **Streaming optional**: Both sync and async methods available
- **Unified streaming events**: `StreamEvent` enum abstracts provider differences

**StreamEvent Enum:**
```rust
#[derive(Debug)]
pub enum StreamEvent {
    TextDelta(String),                        // Text content chunk
    ToolCallStart { id: String, name: String },  // Tool call initiated
    ToolCallDelta { arguments: String },      // Partial tool arguments (JSON string)
    ToolCallDone(String),                     // Tool call complete (id)
    StopReason(String),                       // Stream finished with reason
}
```

### 2. Provider Configuration (src/agent/config.rs)

Provider selection through environment variables:

```rust
pub enum Provider {
    Anthropic,
    OpenAI,
}

pub struct Config {
    pub provider: Provider,
    pub api_key: String,
    pub base_url: Option<String>,
    pub model: String,
    pub workdir: PathBuf,
    pub timeout_seconds: u64,
    pub use_streaming: bool,
}
```

**Environment Variables:**
- `PROVIDER`: "anthropic" (default) or "openai"
- Provider-specific keys: `ANTHROPIC_API_KEY` or `OPENAI_API_KEY`
- Provider-specific URLs: `ANTHROPIC_BASE_URL` or `OPENAI_BASE_URL`
- `MODEL_ID`: Defaults to provider's recommended model
- `USE_STREAMING`: Enable streaming responses (default: false)

### 3. Anthropic Client Implementation (src/client/anthropic.rs)

Implements LLMClient with Anthropic-specific transformations:

```rust
pub struct AnthropicClient {
    client: Client,
    api_key: String,
    base_url: String,
    model: String,
}
```

**Key Features:**
- Direct mapping of internal Message format to Anthropic's format
- SSE streaming parser for Anthropic's event format
- Stop reason mapping: "tool_use" → continue loop, else return

**Streaming Parser:**
- Handles `content_block_delta` events with `type: "text_delta"` or `type: "input_json_delta"`
- Accumulates tool arguments across multiple delta events
- Emits `StreamEvent::StopReason` on `message_stop`

### 4. OpenAI Client Implementation (src/client/openai.rs)

Implements LLMClient with OpenAI-specific transformations:

```rust
pub struct OpenAIClient {
    client: Client,
    api_key: String,
    base_url: String,
    model: String,
}
```

**Tool Transformation (`transform_tools`):**

Converts Anthropic format → OpenAI format:

```rust
// Anthropic (internal) format
{
    "name": "bash",
    "description": "Execute shell command",
    "input_schema": {
        "type": "object",
        "properties": {"command": {"type": "string"}},
        "required": ["command"]
    }
}

// OpenAI format (after transformation)
{
    "type": "function",
    "function": {
        "name": "bash",
        "description": "Execute shell command",
        "parameters": {
            "type": "object",
            "properties": {"command": {"type": "string"}},
            "required": ["command"]
        }
    }
}
```

**Message Transformation (`prepare_messages`):**

Converts internal Message format → OpenAI messages array:

- System message prepended as `{"role": "system", "content": system}`
- User/Assistant messages with content blocks converted
- `ContentBlock::Text` → `{"type": "text", "text": "..."}`
- `ContentBlock::ToolUse` → tool_call object
- `ContentBlock::ToolResult` → tool message with role "tool"

**Response Transformation (`parse_response`):**

Converts OpenAI response → internal format:

```rust
// OpenAI response format
{
    "choices": [{
        "message": {
            "role": "assistant",
            "content": [...]
        }
    }],
    "finish_reason": "tool_calls"  // or "stop", "length", etc.
}

// Maps finish_reason:
- "tool_calls" → "tool_use" (continue loop)
- "stop" → "stop" (return)
- "length" → "max_tokens" (return)
```

**Streaming Parser:**
- Handles OpenAI's SSE format with `data:` prefix
- Parses `choices[0].delta` events
- `delta.content` → `StreamEvent::TextDelta`
- `delta.tool_calls` → Accumulates tool across chunks
- `finish_reason` → `StreamEvent::StopReason`

### 5. Generic Agent Implementation (src/agent/loop_agent.rs)

Agent is now generic over LLMClient:

```rust
pub struct Agent<C: LLMClient> {
    client: C,
    bash_tool: BashTool,
    workdir: String,
    use_streaming: bool,
}
```

**Streaming Execution (`run_streaming`):**

Handles tool execution during streaming:

```rust
async fn run_streaming(&self, history: Arc<RwLock<Vec<Message>>>) -> Result<String> {
    let mut stream = self.client.create_message_stream(...).await?;
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
                current_tool = Some(ContentBlock::ToolUse {
                    id, name,
                    input: ToolInput { command: String::new() },
                });
            }
            StreamEvent::ToolCallDelta { arguments } => {
                // Parse JSON string arguments, extract command field
                if let Some(ref mut tool) = current_tool {
                    if let ContentBlock::ToolUse { ref mut input, .. } = tool {
                        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&arguments) {
                            if let Some(cmd) = json.get("command").and_then(|v| v.as_str()) {
                                input.command = cmd.to_string();
                            }
                        }
                    }
                }
            }
            StreamEvent::ToolCallDone(id) => {
                // Execute tool immediately
                if let Some(tool) = current_tool.take() {
                    if let ContentBlock::ToolUse { input, .. } = tool {
                        let output = self.bash_tool.execute(&input).await?;
                        tool_calls.push(ContentBlock::ToolResult {
                            tool_use_id: id,
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
    // Continue loop if tool calls exist
}
```

**Key Streaming Behaviors:**
- Text displayed as it arrives (immediate feedback)
- Tools executed immediately on `ToolCallDone` (don't wait for stream end)
- Tool calls accumulated and results appended to history
- Loop continues until no tool calls

### 6. Generic REPL Implementation (src/ui/repl.rs)

REPL is now generic over LLMClient:

```rust
pub struct Repl<C: LLMClient> {
    agent: Agent<C>,
}
```

Enables seamless switching between providers without changing REPL code.

### 7. Core Agent Loop (src/agent/loop_agent.rs)

The heart of the agent - implements the pattern used by all coding agents:

```rust
while not done:
    response = model(messages, tools)
    if no tool calls: return
    execute tools, append results
```

**Implementation Details:**
- Async/await using Tokio runtime
- Thread-safe message history using `Arc<RwLock<Vec<Message>>>`
- Automatic loop until `stop_reason != "tool_use"`
- Display of model text output in real-time

### 2. Configuration (src/agent/config.rs)

Environment-based configuration with strong validation:

```rust
pub struct Config {
    pub api_key: String,           // Required: ANTHROPIC_API_KEY
    pub base_url: Option<String>,   // Optional: ANTHROPIC_BASE_URL
    pub model: String,               // Optional: MODEL_ID (default: claude-sonnet-4-5-20250929)
    pub workdir: PathBuf,          // Current working directory
    pub timeout_seconds: u64,        // Command timeout (default: 300)
}
```

**Validation:**
- `ANTHROPIC_API_KEY` is **required** - returns `AgentError::MissingEnvVar` if unset
- All other fields have sensible defaults
- `dotenvy` crate loads `.env` file automatically

### 3. Message Types (src/agent/messages.rs)

Type-safe message handling with Serde serialization:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ContentBlock {
    Text { text: String },
    ToolUse { id: String, name: String, input: ToolInput },
    ToolResult { tool_use_id: String, content: String },
}

pub struct Message {
    pub role: String,
    pub content: Vec<ContentBlock>,
}
```

**Why tag-based enum?**
- Matches Anthropic's response format exactly
- Automatic deserialization based on `"type"` field
- Type safety ensures all content is handled correctly

### 4. Bash Tool with Async & Concurrent Execution (src/tools/bash.rs)

Advanced bash execution with Rust's async capabilities:

**Core Features:**

```rust
pub struct BashTool {
    timeout_secs: u64,
    workdir: String,
}

impl BashTool {
    // Execute single command with timeout
    pub async fn execute(&self, input: &ToolInput) -> Result<String>

    // Execute multiple commands concurrently
    pub async fn execute_all(&self, inputs: Vec<ToolInput>) -> Vec<Result<String>>
}
```

**Timeout Handling:**
- Uses `tokio::time::timeout()` for reliable cancellation
- Configurable timeout (default: 300 seconds)
- Returns `AgentError::Timeout(secs)` on timeout

**Concurrent Execution:**
- `execute_all()` uses `futures::future::join_all()`
- Runs independent bash commands in parallel
- Each command has its own async block and tool instance
- Solves borrow checker issues with `async move` blocks
- Returns results in the same order as inputs

**Why concurrent execution matters:**
- Faster for independent tasks (e.g., checking multiple files)
- Better resource utilization
- Example: Running `ls src/`, `cat README.md`, `wc -l *.rs` simultaneously

### 5. API Client (src/client/anthropic.rs)

HTTP client for Anthropic API using reqwest:

```rust
pub struct AnthropicClient {
    client: Client,
    api_key: String,
    base_url: String,
    model: String,
}

impl AnthropicClient {
    pub async fn create_message(
        &self,
        system: &str,
        messages: &[Message],
        tools: &[Value],
        max_tokens: u32,
    ) -> Result<(String, Vec<ContentBlock>)>
}
```

**Error Handling:**
- Custom `AgentError::InvalidResponse` for API errors
- Captures HTTP status code and error text
- Type-safe error propagation with `?` operator

### 6. Dracula-Themed UI (src/ui/colors.rs)

Purple/pink color scheme inspired by Dracula theme:

```rust
impl Palette {
    pub fn header() -> String     // 🎣 purple bold
    pub fn prompt() -> String     // >> purple bold
    pub fn command(cmd: &str)   // $ {cmd} purple
    pub fn tool_result() -> String // ✓ magenta (RGB 255,0,255)
    pub fn error(msg: &str)      // ✗ {msg} red bold
    pub fn info(msg: &str)       // ℹ {msg} cyan
}
```

**Output Truncation:**
- Tool results truncated to 50,000 characters
- Prevents context bloat from large outputs
- Mimics Python reference behavior

### 7. Interactive REPL (src/ui/repl.rs)

Command-line interface with user-friendly features:

```rust
pub struct Repl {
    agent: Agent,
}

impl Repl {
    pub async fn run(&self) -> Result<(), anyhow::Error>
}
```

**Features:**
- Purple `>>` prompt
- Exit on `q`, `exit`, empty input, or Ctrl+D
- Persistent conversation history across interactions
- Error messages with proper error handling
- `Goodbye!` message on graceful exit

### 8. Error Handling (src/error.rs)

Strongly-typed errors using `thiserror` crate:

```rust
#[derive(Debug, Error)]
pub enum AgentError {
    #[error("Configuration error: {0}")]
    Config(String),

    #[error("API request failed: {0}")]
    Api(#[from] reqwest::Error),

    #[error("Command execution failed: {0}")]
    Command(String),

    #[error("Command timed out after {0}s")]
    Timeout(u64),

    #[error("Tool '{0}' not found")]
    ToolNotFound(String),

    #[error("Serde error: {0}")]
    Serde(#[from] serde_json::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Environment variable '{0}' not set")]
    MissingEnvVar(String),

    #[error("Invalid response: {0}")]
    InvalidResponse(String),
}

pub type Result<T> = std::result::Result<T, AgentError>;
```

**Why `thiserror`?**
- Automatic `Display` implementation
- Automatic `Error` trait implementation
- Automatic `From` implementations for wrapped types
- Clean, declarative error definitions

## Mode Selection

### Subagent Mode

Execute single task and return result:

```bash
cargo run -- "echo hello world and tell me the output"
```

**Use case:**
- Testing agent behavior
- Automated scripting
- Parent agent spawning child for isolated context

**Implementation:**
- Detects `args.len() > 1`
- Creates fresh `Arc<RwLock<Vec<Message>>>` for history
- Prints only final text result
- Exits after completion

### Interactive Mode

REPL for interactive conversations:

```bash
cargo run
```

**Features:**
- Persistent conversation history
- Real-time streaming of tool commands and outputs
- Color-coded output (purple for commands, magenta for results)
- Graceful exit handling

## Testing

### Unit Tests (tests/bash_test.rs)

Comprehensive tests for bash tool:

```rust
#[tokio::test]
async fn test_bash_echo() {
    // Tests: echo command works, output contains expected text
}

#[tokio::test]
async fn test_bash_timeout() {
    // Tests: timeout after 1 second, returns Timeout error
}

#[tokio::test]
async fn test_bash_concurrent() {
    // Tests: execute 3 commands concurrently, all succeed
}
```

**Running tests:**
```bash
# All tests
cargo test

# Specific test file
cargo test --test bash_test

# With output
cargo test -- --nocapture
```

**Test Results:**
```
running 3 tests
test test_bash_echo .............. ok
test test_bash_timeout ........ ok
test test_bash_concurrent ...... ok

test result: ok. 3 passed; 0 failed
```

## Performance Characteristics

### Compared to Python Reference

| Aspect | Python v0 | Rust v0 |
|---------|-------------|----------|
| Binary size | ~5KB | ~3MB (release) |
| Startup time | ~100ms | ~10ms |
| Memory usage | ~20MB | ~5MB |
| Type safety | Dynamic | Strong |
| Error handling | String returns | Result<T> |
| Concurrency | No | Yes (tokio) |
| Compilation | N/A | 19.74s release |

### Async Benefits

- **Non-blocking I/O**: API calls and bash execution don't block
- **Parallel execution**: Multiple independent tools run simultaneously
- **Efficient resource use**: Tokio runtime manages tasks efficiently
- **Scalability**: Easy to add more async operations

## System Prompt

The agent uses this system prompt:

```
You are a CLI agent at {workdir}. Solve problems using bash commands.

Rules:
- Prefer tools over prose. Act first, explain briefly after.
- Read files: cat, grep, find, rg, ls, head, tail
- Write files: echo '...' > file, sed -i, or cat << 'EOF' > file
- Subagent: For complex subtasks, spawn a subagent to keep context clean:
  cargo run -- 'explore src/ and summarize'

When to use subagent:
- Task requires reading many files (isolate exploration)
- Task is independent and self-contained
- You want to avoid polluting current conversation with intermediate details

The subagent runs in isolation and returns only its final summary.
```

## Tool Schema

Single bash tool with comprehensive description:

```json
{
  "name": "bash",
  "description": "Execute shell command. Common patterns:\n\
                        - Read: cat/head/tail, grep/find/rg/ls, wc -l\n\
                        - Write: echo 'content' > file, sed -i 's/old/new/g' file\n\
                        - Subagent: For complex subtasks, spawn a subagent to keep context clean:\n\
                          cargo run -- 'task description' (spawns isolated agent, returns summary)",
  "input_schema": {
    "type": "object",
    "properties": {
      "command": {
        "type": "string",
        "description": "The shell command to execute"
      }
    },
    "required": ["command"]
  }
}
```

## Future Enhancements

Potential additions for v1+ versions:

1. **Streaming API responses** - Real-time text streaming for better UX
2. **More file tools** - `read_file`, `write_file`, `edit_file` like v1
3. **Todo tracking** - `TodoWrite` tool for explicit planning (v2)
4. **Subagent types** - `explore`, `code`, `plan` agents (v3)
5. **Skills system** - Load domain knowledge from files (v4)

## Dependencies

### Core Dependencies

```toml
tokio = { version = "1.39", features = ["full"] }   # Async runtime
reqwest = { version = "0.12", features = ["json", "stream"] }  # HTTP client
serde = { version = "1.0", features = ["derive"] }     # Serialization
serde_json = "1.0"                                      # JSON
dotenvy = "0.15"                                         # .env loading
anyhow = "1.0"                                          # Error convenience
thiserror = "1.0"                                         # Error derive
colored = "2.1"                                          # Terminal colors
futures = "0.3"                                          # Async utilities
```

### Dev Dependencies

```toml
mockito = "1.4"       # HTTP mocking for tests
wiremock = "0.6"       # Alternative HTTP mocking
tokio-test = "0.4"      # Tokio testing utilities
```

## Building

```bash
# Debug build
cargo build

# Release build
cargo build --release

# Run tests
cargo test

# Run with release binary
cargo run --release
```

## License

MIT - Same as Python reference implementation.

## Credits

- Original Python v0 implementation: [learn-claude-code](https://github.com/shareAI-lab/learn-claude-code)
- Rust implementation: Amadeus
