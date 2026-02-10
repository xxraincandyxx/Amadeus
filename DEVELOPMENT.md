# Development Guide

This document explains the working theory and provides a development guide for Amadeus.

## How It Works

### Core Agent Loop Pattern

The agent follows a simple but powerful loop pattern used by all AI coding agents:

```
while not done:
    response = llm.call(messages, tools)
    if no tool calls in response:
        return text_content
    for tool in response.tool_calls:
        output = tool.execute(input)
        messages.append(tool_result(output))
```

### Message Flow

```
1. User enters prompt
   ↓
2. System prompt + prompt added to history
   ↓
3. LLM API called with history + tools schema
   ↓
4. LLM responds with:
   - Text content
   - Tool calls (optional)
   ↓
5a. If no tool calls: Display text, done
5b. If tool calls:
    - Execute each tool
    - Append tool results to history
    - Repeat from step 3
```

### Key Abstractions

#### 1. LLMClient Trait

Generic provider abstraction that lets the core agent work with any LLM provider:

```rust
#[async_trait]
pub trait LLMClient: Send + Sync {
    async fn create_message(...) -> Result<(String, Vec<ContentBlock>)>;
    async fn create_message_stream(...) -> Result<Pin<Box<dyn Stream<...>> + Send>>;
}
```

**Why this matters:**
- Zero-cost generic abstraction (resolved at compile time)
- Easy to add new providers (Google DeepSeek, etc.)
- Single internal message format, provider-specific transformations at boundaries

#### 2. Message Types

Tag-based enum for type-safe content:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]  // Matches Anthropic's format exactly
pub enum ContentBlock {
    Text { text: String },
    ToolUse { id: String, name: String, input: ToolInput },
    ToolResult { tool_use_id: String, content: String },
}
```

**Why tag-based enum:**
- Automatic deserialization based on `"type"` field
- Type safety ensures all content is handled correctly
- Matches Anthropic's response format exactly

#### 3. StreamEvent Enum

Unified streaming events across providers:

```rust
pub enum StreamEvent {
    TextDelta(String),                        // Text chunk arriving
    ToolCallStart { id: String, name: String },  // Tool call initiated
    ToolCallDelta { arguments: String },      // Partial JSON arguments
    ToolCallDone(String),                     // Tool call complete
    StopReason(String),                       // Stream finished
}
```

**Why needed:**
- Anthropic and OpenAI have different streaming formats
- Unified interface lets agent loop work regardless of provider
- Tools can execute immediately on `ToolCallDone` (don't wait for stream end)

## Architecture Deep Dive

### Provider Transformations

Each provider transforms data at API boundaries:

#### OpenAI Transformations

**Tool Schema:**
```json
// Internal (Anthropic-style)
{
  "name": "bash",
  "description": "Execute shell command",
  "input_schema": {"type": "object", ...}
}

// OpenAI format (after transformation)
{
  "type": "function",
  "function": {
    "name": "bash",
    "description": "Execute shell command",
    "parameters": {"type": "object", ...}
  }
}
```

**Response:**
- `finish_reason = "tool_calls"` → mapped to `"tool_use"`
- `message.tool_calls[]` → parsed into `ToolUse` content blocks

#### Anthropic Transformations

- Direct mapping (internal format matches Anthropic's)
- No transformation needed for most fields
- Streaming: parses `content_block_delta`, `message_stop` events

### Async Concurrency Model

#### Tokio Runtime

```rust
#[tokio::main]
async fn main() -> Result<()> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    runtime.block_on(async_main())
}
```

**Why multi-threaded:**
- Better CPU utilization
- Can run multiple async tasks in parallel
- Default for production Rust async applications

#### Bash Tool Concurrency

```rust
pub async fn execute_all(&self, inputs: Vec<ToolInput>) -> Vec<Result<String>> {
    let futures = inputs.iter().map(|input| {
        let cmd = input.command.clone();
        let tool = BashTool::new(self.timeout_secs, self.workdir.clone());
        async move { tool.execute_with_timeout(&cmd).await }
    }).collect();

    join_all(futures).await
}
```

**Key patterns:**
- `async move` - each closure owns its data
- `tool.clone()` - each execution gets its own tool instance
- `join_all()` - waits for all to complete, returns results in order

**Why this matters:**
- Independent commands run truly in parallel
- Solves borrow checker issues (no shared mutable state)
- Example: `ls src/`, `cat README.md`, `wc -l *.rs` simultaneously

#### Thread-Safe Message History

```rust
let history = Arc<RwLock<Vec<Message>>>::new(Arc::new(vec![]));
```

- `Arc` - allows multiple owners (sharing across async tasks)
- `RwLock` - multiple readers OR one writer (not both)
- Safe access without data races

**Usage pattern:**
```rust
// Read
let msgs = history.read().await?.clone();

// Write
history.write().await?.push(message);
```

### Streaming vs Non-Streaming

#### Non-Streaming (simpler)

```rust
let (stop_reason, content) = client.create_message(...).await?;
// Process entire response at once
```

**When to use:**
- Debugging
- Simple tasks
- When streaming not available

#### Streaming (better UX)

```rust
let mut stream = client.create_message_stream(...).await?;
while let Some(event) = stream.next().await {
    match event? {
        StreamEvent::TextDelta(text) => {
            print!("{}", text);  // Immediate feedback
            text_content.push_str(&text);
        }
        StreamEvent::ToolCallDone(id) => {
            // Execute immediately
            let output = bash_tool.execute(&input).await?;
        }
        // ...
    }
}
```

**Why streaming is better:**
- Real-time feedback (feels faster)
- Tools execute immediately (don't wait for stream end)
- Better user experience

**Streaming challenges:**
- Tool arguments arrive in chunks (need to accumulate)
- Need buffer for accumulating JSON chunks
- More complex state management

## Development Guide

### Adding a New Tool

1. Define tool struct in `src/tools/`:

```rust
pub struct NewTool {
    timeout_secs: u64,
}

impl NewTool {
    pub async fn execute(&self, input: &NewToolInput) -> Result<String> {
        // Your logic here
    }
}
```

2. Add tool to `src/tools/mod.rs`:

```rust
pub mod bash;
pub mod new_tool;  // Add this
```

3. Add to agent's tool list in `src/agent/loop_agent.rs`:

```rust
let tools = vec![
    create_tool_schema("bash", "...", bash_schema),
    create_tool_schema("new_tool", "...", new_tool_schema),  // Add this
];
```

### Adding a New Provider

1. Create `src/client/new_provider.rs`:

```rust
pub struct NewProviderClient { ... }

#[async_trait]
impl LLMClient for NewProviderClient {
    async fn create_message(...) -> Result<(String, Vec<ContentBlock>)> {
        // Transform messages to NewProvider format
        // Call NewProvider API
        // Transform response back to internal format
    }

    async fn create_message_stream(...) -> Result<Pin<Box<dyn Stream<...>>>> {
        // Same as above, but return stream
    }
}
```

2. Add to `src/client/mod.rs`:

```rust
pub mod anthropic;
pub mod openai;
pub mod new_provider;  // Add this

pub use new_provider::NewProviderClient;
```

3. Add to `src/agent/config.rs` and `src/main.rs` for provider selection.

### Debugging Tips

1. **Enable debug output:**

```bash
RUST_LOG=debug cargo run
```

2. **Print full JSON responses:**

```rust
eprintln!("Response: {}", serde_json::to_string_pretty(&json)?);
```

3. **Test tool in isolation:**

```bash
cargo run -- "echo test"
cargo run -- "ls -la"
```

4. **Check backtrace on panic:**

```bash
RUST_BACKTRACE=1 cargo run
```

### Testing

#### Unit Tests

```rust
#[tokio::test]
async fn test_bash_echo() {
    let tool = BashTool::new(10, ".".into());
    let input = ToolInput { command: "echo hello".into() };
    let output = tool.execute(&input).await.unwrap();
    assert!(output.contains("hello"));
}
```

#### Integration Tests with Mocking

```rust
#[tokio::test]
async fn test_client() {
    let mock_server = MockServer::start().await;
    // Set up mock response...
    let client = OpenAIClient::new(
        "key".into(),
        Some(mock_server.uri()),
        "model".into()
    );
    // Test client...
}
```

#### Run Tests

```bash
# All tests
cargo test

# Specific test
cargo test test_bash_echo

# With output
cargo test -- --nocapture

# Specific file
cargo test --test bash_test
```

### Performance Optimization

1. **Use release builds:**

```bash
cargo build --release
cargo run --release
```

2. **Profile with cargo-flamegraph:**

```bash
cargo install flamegraph
cargo flamegraph --bin claude-agent
```

3. **Reduce allocations:**

- Reuse strings where possible
- Use `&str` instead of `String` for function arguments
- Pre-allocate vectors with `Vec::with_capacity()`

### Common Issues

#### Issue: "no entry found for key" panic

**Cause:** Using `map["key"]` instead of `map.get("key")` for serde_json Values

**Fix:**
```rust
// Wrong
let value = json["key"].as_str().unwrap();

// Correct
let value = json.get("key").and_then(|v| v.as_str()).unwrap_or("");
```

#### Issue: Borrow checker errors in async loops

**Cause:** Shared mutable state across async awaits

**Fix:** Clone data before async move:
```rust
let cmd = input.command.clone();  // Clone
let tool = tool.clone();          // Clone
async move { tool.execute(&cmd).await }  // Take ownership
```

#### Issue: Race conditions on history

**Cause:** Multiple tasks accessing `Arc<RwLock<Vec<Message>>>` incorrectly

**Fix:** Always use proper locking:
```rust
// Read
let msgs = history.read().await?.clone();

// Write
history.write().await?.push(message);
```

### Adding Features

#### 1. File Tools (Read/Write/Edit)

Instead of `echo '...' > file`, add dedicated tools:

```rust
pub struct ReadFileTool;
pub struct WriteFileTool;
pub struct EditFileTool;
```

**Trade-off:**
- Pro: Simpler prompts, better error messages
- Con: More code, more surface area

**Decision:**
- Stay with bash for v0 (philosophy: "bash is all you need")
- Consider for v1 if needed

#### 2. Todo List Tool

Add explicit planning:

```rust
{
  "name": "todo_write",
  "input_schema": {
    "todos": [{"id": "1", "content": "...", "status": "pending"}]
  }
}
```

**Benefits:**
- Explicit task tracking
- Better for complex multi-step tasks
- Helps model decompose work

**When to add:**
- v2 (adds explicit planning step)

#### 3. Skills System

Load domain knowledge from files:

```rust
{
  "name": "skill",
  "description": "Load skill: rust, python, frontend, ...",
  "input_schema": {"name": "skill_name"}
}
```

**Benefits:**
- Domain-specific knowledge
- Better code in specific languages
- Reusable across agents

**When to add:**
- v4 (adds curated skill modules)

## Code Style

See [AGENTS.md](AGENTS.md) for complete style guidelines:
- Imports: std → third-party → internal
- Naming: snake_case for functions, PascalCase for types
- Error handling: Use `Result<T>` from `crate::error`
- No comments unless asked

## Build Commands

```bash
# Debug build
cargo build

# Release build (optimized)
cargo build --release

# Run tests
cargo test

# Lint
cargo check
cargo clippy

# Run agent
cargo run
cargo run -- "prompt"
PROVIDER=openai USE_STREAMING=true cargo run
```

## Resources

- [Tokio Async Guide](https://tokio.rs/tokio/tutorial)
- [serde_json Documentation](https://docs.rs/serde_json/)
- [reqwest HTTP Client](https://docs.rs/reqwest/)
- [thiserror Error Derive](https://docs.rs/thiserror/)

## Contributing

1. Follow existing code patterns
2. Add tests for new features
3. Update documentation (ARCHITECTURE.md, AGENTS.md)
4. Run `cargo test` and `cargo clippy` before committing
5. Keep it minimal - "bash is all you need"