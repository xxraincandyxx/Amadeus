# Amadeus SDK Scope Definition

## Philosophy

**Amadeus is an Agent SDK, not a Platform.**

The SDK provides the core building blocks for building AI agents. It does NOT manage sessions, memory, persistence, or platform integrations. Those responsibilities belong to the Platform layer (e.g., NeuroCore).

## Scope

### ✅ SDK Responsibilities

| Component | Description |
|-----------|-------------|
| **Agent Loop** | ReAct pattern implementation, tool orchestration |
| **LLM Clients** | Anthropic, OpenAI, and extensible client interfaces |
| **Tool System** | Tool registry, execution, schema generation |
| **Streaming** | SSE streaming for real-time responses |
| **Error Handling** | Typed errors, result types |

### ❌ NOT SDK Responsibilities

| Component | Belongs To |
|-----------|------------|
| HTTP Server | Platform layer |
| Session Management | Platform layer |
| Memory/Persistence | Platform layer |
| Workspace Management | Platform layer |
| Platform Adapters (Discord, etc.) | Platform layer |
| User Authentication | Platform layer |
| Multi-tenancy | Platform layer |

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                     Platform Layer                           │
│  (NeuroCore, or custom implementation)                       │
│                                                             │
│  ┌─────────────┐ ┌─────────────┐ ┌─────────────────────┐   │
│  │ Session     │ │ Memory      │ │ Platform Adapters   │   │
│  │ Manager     │ │ Engine      │ │ Discord/Telegram    │   │
│  └─────────────┘ └─────────────┘ └─────────────────────┘   │
│                                                             │
│  ┌─────────────┐ ┌─────────────┐ ┌─────────────────────┐   │
│  │ HTTP API    │ │ Plugin Sys  │ │ UI (Web/TUI/Desktop)│   │
│  └─────────────┘ └─────────────┘ └─────────────────────┘   │
│                                                             │
└───────────────────────────┬─────────────────────────────────┘
                            │ uses
                            ▼
┌─────────────────────────────────────────────────────────────┐
│                     Amadeus SDK                              │
│                                                             │
│  ┌─────────────┐ ┌─────────────┐ ┌─────────────────────┐   │
│  │ Agent Loop  │ │ Tool System │ │ LLM Clients         │   │
│  │ (ReAct)     │ │ (Registry)  │ │ Anthropic/OpenAI    │   │
│  └─────────────┘ └─────────────┘ └─────────────────────┘   │
│                                                             │
│  No state, no persistence, no platform knowledge            │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

## SDK API

### Core Types

```rust
/// The main agent that orchestrates LLM interaction
pub struct Agent<C: LLMClient> {
    client: C,
    tools: ToolRegistry,
    config: AgentConfig,
}

impl<C: LLMClient> Agent<C> {
    /// Run a single turn
    pub async fn run(&self, prompt: &str, history: &[Message]) -> Result<RunResult>;
    
    /// Run with streaming
    pub async fn run_stream(&self, prompt: &str, history: &[Message]) 
        -> impl Stream<Item = AgentEvent>;
}

/// Result of a single agent run
pub struct RunResult {
    pub text: String,
    pub tool_calls: Vec<ToolCall>,
    pub usage: Usage,
}

/// A message in the conversation
pub struct Message {
    pub role: Role,
    pub content: Vec<ContentBlock>,
}

/// Tool registry for managing available tools
pub struct ToolRegistry {
    tools: HashMap<String, Box<dyn Tool>>,
}

impl ToolRegistry {
    pub fn register(&mut self, name: &str, tool: Box<dyn Tool>);
    pub fn execute(&self, name: &str, input: Value) -> Result<Value>;
    pub fn schemas(&self) -> Vec<ToolSchema>;
}
```

### LLM Client Trait

```rust
#[async_trait]
pub trait LLMClient: Send + Sync {
    async fn create_message(
        &self,
        system: &str,
        messages: &[Message],
        tools: &[ToolSchema],
        max_tokens: u32,
    ) -> Result<(String, Vec<ContentBlock>)>;
    
    async fn create_message_stream(
        &self,
        system: &str,
        messages: &[Message],
        tools: &[ToolSchema],
        max_tokens: u32,
    ) -> Result<Box<dyn Stream<Item = Result<StreamEvent>> + Send>>;
}
```

### Tool Trait

```rust
#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn schema(&self) -> Value;
    async fn execute(&self, input: Value) -> Result<Value>;
}
```

## Refactoring Plan

### Files to Keep (SDK Core)

```
src/
├── lib.rs              # SDK exports
├── error.rs            # Error types
├── agent/
│   ├── mod.rs
│   ├── agent.rs        # Agent struct
│   ├── config.rs       # AgentConfig (minimal)
│   ├── messages.rs     # Message types
│   └── events.rs       # AgentEvent
├── client/
│   ├── mod.rs          # LLMClient trait
│   ├── anthropic.rs    # Anthropic implementation
│   └── openai.rs       # OpenAI implementation
└── tools/
    ├── mod.rs          # Tool trait
    ├── bash.rs         # BashTool
    ├── file.rs         # FileTool (read/write/edit)
    └── schema.rs       # Tool schema generation
```

### Files to Move/Remove (Platform Layer)

```
src/
├── api/                # Move to examples/ or remove
│   ├── http.rs         # HTTP server → Platform
│   ├── types.rs        # Keep minimal SDK types
│   └── handlers/       # Remove
├── core/               # Remove
│   ├── workspace.rs    # → Platform
│   └── event.rs        # → Platform
├── concurrency/        # Remove
│   ├── lock.rs         # → Platform
│   └── transaction.rs  # → Platform
└── ui/                 # Keep for SDK testing
    ├── app.rs          # Refactor as SDK test harness
    └── components/     # Keep minimal
```

### TUI as Test Harness

The TUI (`src/ui/`) will be retained and refactored as a **SDK test harness**:

- Purpose: Test SDK performance and behavior
- No Platform features (no session persistence, no memory)
- Direct SDK API usage
- Can be run with `cargo run --example tui`

## Integration with Platform

### Python (NeuroCore)

```python
# Using PyO3 bindings
import amadeus

agent = amadeus.Agent(
    provider="openai",
    model="gpt-4",
)

# Register tools
tools = amadeus.ToolRegistry()
tools.register("bash", amadeus.BashTool())
tools.register("read_file", amadeus.ReadFileTool())
agent.set_tools(tools)

# Run
result = await agent.run("Create a hello world program", history)
```

### Rust (Native)

```rust
use amadeus::{Agent, OpenAIClient, ToolRegistry, BashTool};

let client = OpenAIClient::new(api_key, base_url, model);
let mut tools = ToolRegistry::new();
tools.register("bash", BashTool::new());

let agent = Agent::new(client, tools);
let result = agent.run("Create a hello world program", &history).await?;
```

## Versioning

- SDK follows semantic versioning
- Breaking changes only in major versions
- Platform implementations can upgrade independently

## Testing Strategy

1. **Unit Tests**: SDK core functionality
2. **Integration Tests**: LLM client mocking
3. **TUI Test Harness**: Manual performance testing
4. **Platform Tests**: In NeuroCore repository

---

*Document created: 2026-02-20*
*Last updated: 2026-02-20*
