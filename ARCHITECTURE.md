# Amadeus - AI Coding Agent (Rust Implementation)

Amadeus is a Rust-based AI coding agent implementing the v0 "Bash is All You Need" philosophy with a modern ratatui-based TUI.

## Overview

This agent demonstrates that **a few tools are enough** for a fully functional AI coding agent:
- **bash**: Execute shell commands (git, npm, python, ls, grep, etc.)
- **read_file**: Read file contents
- **write_file**: Create or overwrite files
- **edit_file**: Make surgical changes to existing files

## Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                              TUI Layer                               │
│  ┌─────────┐ ┌──────────┐ ┌─────────┐ ┌────────┐ ┌───────────────┐  │
│  │  Input  │ │ Messages │ │ Sidebar │ │ Status │ │  Tool Panels  │  │
│  └────┬────┘ └────┬─────┘ └────┬────┘ └───┬────┘ └───────┬───────┘  │
└───────┼───────────┼────────────┼───────────┼──────────────┼──────────┘
        │           │            │           │              │
        ▼           ▼            ▼           ▼              ▼
┌─────────────────────────────────────────────────────────────────────┐
│                            App State                                 │
│  ┌──────────────┐  ┌──────────────┐  ┌───────────────────────────┐  │
│  │ Agent<C>     │  │ History      │  │ ToolPanel, Messages, etc. │  │
│  │ (generic)    │  │ Arc<RwLock>  │  │                           │  │
│  └──────┬───────┘  └──────────────┘  └───────────────────────────┘  │
└─────────┼───────────────────────────────────────────────────────────┘
          │
          ▼
┌─────────────────────────────────────────────────────────────────────┐
│                          Agent Loop                                  │
│  ┌─────────────┐    ┌──────────────┐    ┌────────────────────┐      │
│  │ LLMClient   │───▶│ ToolRegistry │───▶│ RunResult          │      │
│  │ (trait)     │    │ (bash,file)  │    │ {text, tool_calls} │      │
│  └──────┬──────┘    └──────────────┘    └────────────────────┘      │
└─────────┼───────────────────────────────────────────────────────────┘
          │
    ┌─────┴─────┐
    ▼           ▼
┌───────┐   ┌───────┐
│Anthropic  │ OpenAI│
│Client │   │ Client│
└───────┘   └───────┘
```

## Core Components

### 1. Agent Loop (`src/agent/loop_agent.rs`)

The heart of the agent - implements the ReAct pattern:

```
while not done:
    response = model(messages, tools)
    if no tool calls: return RunResult
    execute tools, collect results
    append to history, continue
```

**Key Structures:**

```rust
pub struct RunResult {
    pub text: String,           // Final text response
    pub tool_calls: Vec<ToolCall>,  // All tool executions
}

pub struct ToolCall {
    pub name: String,      // Tool name (bash, read_file, etc.)
    pub input: Value,      // JSON input
    pub output: String,    // Tool output
    pub is_error: bool,    // Whether execution failed
}

pub struct Agent<C: LLMClient> {
    client: C,
    tools: ToolRegistry,
    config: Arc<Config>,
}
```

**Non-Streaming Loop (`run_non_streaming`):**

```rust
async fn run_non_streaming(&self, history: Arc<RwLock<Vec<Message>>>) -> Result<RunResult> {
    let mut result = RunResult::default();
    
    loop {
        // 1. Call LLM with current history
        let (stop_reason, content) = self.client
            .create_message(&system, &history_guard, &tools, 8000)
            .await?;
        
        // 2. Extract text content
        for block in &content {
            if let ContentBlock::Text { text } = block {
                result.text.push_str(text);
            }
        }
        
        // 3. If no tools, we're done
        if stop_reason != "tool_use" {
            return Ok(result);
        }
        
        // 4. Execute tools, collect results
        let (tool_results, tool_calls) = self.execute_tools(&content).await;
        result.tool_calls.extend(tool_calls);
        
        // 5. Append to history and continue
        history_guard.push(Message::assistant(content));
        history_guard.push(Message::tool_results(tool_results));
    }
}
```

**Streaming Loop (`run_streaming`):**

```rust
async fn run_streaming(&self, history: Arc<RwLock<Vec<Message>>>) -> Result<RunResult> {
    let mut result = RunResult::default();
    
    loop {
        let mut stream = self.client.create_message_stream(...).await?;
        let mut current_tool: Option<(String, String, String)> = None;
        
        while let Some(event) = stream.next().await {
            match event? {
                StreamEvent::TextDelta(text) => {
                    result.text.push_str(&text);
                }
                
                StreamEvent::ToolCallStart { id, name } => {
                    current_tool = Some((id, name, String::new()));
                }
                
                StreamEvent::ToolCallDelta { arguments } => {
                    // Accumulate tool arguments
                }
                
                StreamEvent::ToolCallDone(_id) => {
                    // Execute tool immediately
                    let output = self.tools.execute(&name, input).await?;
                    result.tool_calls.push(ToolCall { ... });
                }
                
                StreamEvent::StopReason(reason) => {
                    if reason != "tool_use" { return Ok(result); }
                }
            }
        }
    }
}
```

### 2. Tool Registry (`src/tools/registry.rs`)

Dynamic tool management with trait objects:

```rust
pub struct ToolRegistry {
    tools: Vec<Box<dyn Tool>>,
}

impl ToolRegistry {
    pub fn register(mut self, tool: Box<dyn Tool>) -> Self {
        self.tools.push(tool);
        self
    }
    
    pub fn execute(&self, name: &str, input: Value) -> Result<String> {
        self.tools
            .iter()
            .find(|t| t.name() == name)
            .ok_or_else(|| AgentError::ToolNotFound(name.to_string()))?
            .execute(input)
            .await
    }
}
```

**Tool Trait:**

```rust
#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &'static str;
    fn schema(&self) -> &'static Value;
    async fn execute(&self, input: Value) -> Result<String>;
}
```

### 3. LLM Client Trait (`src/client/mod.rs`)

Provider abstraction:

```rust
#[async_trait]
pub trait LLMClient: Send + Sync {
    async fn create_message(
        &self,
        system: &str,
        messages: &[Message],
        tools: &[Value],
        max_tokens: u32,
    ) -> Result<(String, Vec<ContentBlock>)>;
    
    async fn create_message_stream(
        &self,
        system: &str,
        messages: &[Message],
        tools: &[Value],
        max_tokens: u32,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamEvent>> + Send>>>;
}
```

**StreamEvent Enum:**

```rust
pub enum StreamEvent {
    TextDelta(String),
    ToolCallStart { id: String, name: String },
    ToolCallDelta { arguments: String },
    ToolCallDone(String),
    StopReason(String),
}
```

### 4. TUI Components (`src/ui/`)

#### App (`src/ui/app.rs`)

Main application state machine:

```rust
pub struct App<C: LLMClient> {
    agent: Agent<C>,
    history: Arc<RwLock<Vec<Message>>>,
    mode: AppMode,              // Normal or Input
    messages: MessagesComponent,
    input: InputComponent,
    status: StatusBar,
    tool_panel: ToolPanel,
    sidebar: Option<Sidebar>,
    should_quit: bool,
    workdir: PathBuf,
}
```

**Event Loop:**

```rust
async fn run_loop(&mut self, terminal: &mut Terminal, events: EventHandler) -> Result<()> {
    loop {
        if self.should_quit { break; }
        
        terminal.draw(|f| self.render(f))?;
        
        match events.next()? {
            AppEvent::Key(key) => self.handle_key(key).await?,
            AppEvent::Tick => self.status.tick(),
            // ...
        }
    }
}
```

**Submit Input Flow:**

```rust
async fn submit_input(&mut self) -> Result<()> {
    self.messages.add_user(input);
    self.tool_panel.clear();
    self.status.set_state(AppState::Processing);
    
    let result = self.agent.run(input, history).await?;
    
    // Add tool calls to panel
    for tool_call in &result.tool_calls {
        self.tool_panel.add_result(ToolResult { ... });
    }
    
    // Add text to messages
    self.messages.add_assistant(result.text);
    
    Ok(())
}
```

#### Input Component (`src/ui/components/input.rs`)

Multiline textarea with history:

```rust
pub struct InputComponent {
    textarea: TextArea<'static>,
    history: Vec<String>,
    history_index: Option<usize>,
    current_draft: String,
}
```

**Features:**
- Multiline input (Ctrl+Enter for newline)
- Command history (Up/Down arrows)
- Placeholder text

#### Messages Component (`src/ui/components/messages.rs`)

Scrollable message list with markdown rendering:

```rust
pub struct MessagesComponent {
    messages: Vec<Message>,
    scroll_offset: usize,
    auto_scroll: bool,
}

pub struct Message {
    pub role: MessageRole,
    pub content: String,
    pub tool_name: Option<String>,
    pub is_collapsed: bool,
}
```

#### Sidebar (`src/ui/components/sidebar.rs`)

Toggleable file tree and help:

```rust
pub enum Sidebar {
    Files(FileSidebar),
    Help(HelpSidebar),
}
```

- **File Tree**: Shows directory structure (max depth 3)
- **Help Panel**: Keyboard shortcuts

#### Status Bar (`src/ui/components/status.rs`)

Processing state with spinner:

```rust
pub struct StatusBar {
    state: AppState,      // Idle, Processing, Success, Error
    start_time: Option<Instant>,
    token_count: usize,
    model_name: String,
    spinner_frame: usize,
}
```

### 5. File Organization

```
src/
├── lib.rs              # Public exports
├── main.rs             # CLI entry point
├── error.rs            # Error types
├── agent/
│   ├── mod.rs          # Agent, RunResult, ToolCall exports
│   ├── config.rs       # Config, Provider
│   ├── messages.rs     # Message, ContentBlock
│   └── loop_agent.rs   # Agent implementation
├── client/
│   ├── mod.rs          # LLMClient trait, StreamEvent
│   ├── anthropic.rs    # Anthropic implementation
│   └── openai.rs       # OpenAI implementation
├── tools/
│   ├── mod.rs          # ToolRegistry
│   ├── tool_trait.rs   # Tool trait
│   ├── bash.rs         # BashTool
│   ├── file.rs         # ReadFileTool, WriteFileTool, EditFileTool
│   └── schema.rs       # Tool JSON schemas
├── ui/
│   ├── mod.rs          # UI exports
│   ├── app.rs          # Main App state
│   ├── event.rs        # Event handling
│   ├── colors.rs       # Dracula theme
│   ├── repl.rs         # Legacy REPL
│   └── components/
│       ├── mod.rs
│       ├── input.rs
│       ├── messages.rs
│       ├── markdown.rs
│       ├── sidebar.rs
│       ├── status.rs
│       └── tools.rs
└── api/
    ├── mod.rs          # API prelude
    ├── http.rs         # HTTP server
    ├── types.rs        # Request/Response types
    └── handlers/
        ├── chat.rs
        ├── execute.rs
        ├── stream.rs
        └── health.rs
```

## Data Flow

### Request Flow

```
User Input (TUI)
      │
      ▼
┌─────────────┐
│ App::submit │
│   _input()  │
└──────┬──────┘
       │
       ▼
┌─────────────┐
│ Agent::run  │
│   (prompt)  │
└──────┬──────┘
       │
       ├──────────────────────┐
       ▼                      ▼
┌─────────────┐        ┌─────────────┐
│ LLMClient   │        │ ToolRegistry│
│ ::create_   │        │ ::execute() │
│ message()   │        └──────┬──────┘
└──────┬──────┘               │
       │                      │
       ▼                      ▼
┌─────────────┐        ┌─────────────┐
│ Anthropic/  │        │ BashTool /  │
│ OpenAI API  │        │ FileTools   │
└─────────────┘        └─────────────┘
```

### Response Flow

```
┌─────────────┐
│ Agent::run  │
│   returns   │
└──────┬──────┘
       │
       ▼
┌─────────────┐
│ RunResult   │
│ {text,      │
│  tool_calls}│
└──────┬──────┘
       │
       ├──────────────────┐
       ▼                  ▼
┌─────────────┐    ┌─────────────┐
│ Messages    │    │ ToolPanel   │
│ ::add_      │    │ ::add_      │
│ assistant() │    │ result()    │
└──────┬──────┘    └──────┬──────┘
       │                  │
       ▼                  ▼
┌─────────────────────────────┐
│         TUI Render          │
│  (structured, no stdout)    │
└─────────────────────────────┘
```

## Configuration

### Environment Variables

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `PROVIDER` | No | `anthropic` | LLM provider (`anthropic` or `openai`) |
| `ANTHROPIC_API_KEY` | Yes* | - | Anthropic API key |
| `OPENAI_API_KEY` | Yes* | - | OpenAI API key |
| `MODEL_ID` | No | Provider default | Model identifier |
| `MAX_OUTPUT_BYTES` | No | `50000` | Max tool output size |
| `BLOCKED_COMMANDS` | No | `rm -rf /` | Comma-separated blocked commands |

### Config Struct

```rust
pub struct Config {
    pub provider: Provider,
    pub api_key: String,
    pub base_url: Option<String>,
    pub model: String,
    pub workdir: PathBuf,
    pub timeout_seconds: u64,
    pub max_output_bytes: usize,
    pub blocked_commands: Vec<String>,
}
```

## Keyboard Shortcuts

| Key | Action |
|-----|--------|
| `Enter` | Submit message |
| `Ctrl+Enter` | Insert newline |
| `↑` / `↓` | Navigate history |
| `PgUp` / `PgDn` | Scroll messages |
| `Ctrl+B` / `⌘B` | Toggle file tree sidebar |
| `Alt+B` / `⌥B` | Toggle help sidebar |
| `Esc` | Cancel stream / collapse panels / close sidebar |
| `q` / `Ctrl+D` | Exit |

## Dependencies

```toml
[dependencies]
tokio = { version = "1.39", features = ["full"] }
reqwest = { version = "0.12", features = ["json", "stream"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
dotenvy = "0.15"
anyhow = "1.0"
thiserror = "1.0"
colored = "2.1"
crossterm = { version = "0.27", features = ["event-stream"] }
ratatui = "0.28"
tui-textarea = "0.6"
unicode-width = "0.2"
futures = "0.3"
async-trait = "0.1"
axum = "0.7"
tower = "0.4"
tower-http = { version = "0.5", features = ["cors"] }
walkdir = "2.5"
```

## API Endpoints

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/chat` | POST | Send message to agent |
| `/execute` | POST | Execute bash command directly |
| `/stream` | POST | Streaming SSE response |
| `/health` | GET | Health check |

## Build Commands

```bash
cargo build                          # Debug build
cargo build --release                # Release build
cargo test                           # Run all tests
cargo test test_bash_echo            # Run specific test
cargo run                            # Interactive TUI
cargo run -- "prompt"                # Single-shot mode
cargo run -- --server                # HTTP server on port 3000
```

## Error Handling

```rust
#[derive(Debug, Error)]
pub enum AgentError {
    #[error("Configuration error: {0}")]
    Config(String),
    
    #[error("API request failed: {0}")]
    Api(#[from] reqwest::Error),
    
    #[error("Command timed out after {0}s")]
    Timeout(u64),
    
    #[error("Tool '{0}' not found")]
    ToolNotFound(String),
    
    #[error("Path escapes workspace: {0}")]
    PathEscape(PathBuf),
    
    // ... more variants
}
```

## License

MIT
