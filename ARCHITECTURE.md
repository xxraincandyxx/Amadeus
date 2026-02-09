# Amadeus - AI Coding Agent (Rust Implementation)

Amadeus is a Rust-based AI coding agent implementing the v0 "Bash is All You Need" philosophy from the learn-claude-code project.

## Overview

This agent demonstrates that **one tool is enough** for a fully functional AI coding agent. With just bash, the model can:
- Read files (`cat`, `grep`, `head`)
- Write files (`echo > file`, `sed`)
- Execute any command (`python`, `npm`, `make`)
- Spawn subagents via recursive process calls

## Features

### 1. Core Agent Loop (src/agent/loop_agent.rs)

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
