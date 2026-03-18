# Amadeus Architecture

> AI Agent SDK - Core building blocks for building AI agents

## Overview

Amadeus is a production-ready Rust SDK for building AI agents with LLM support. It provides multi-provider compatibility (Anthropic Claude, OpenAI GPT), streaming responses, an extensible tool system, and both TUI and HTTP API interfaces.

## High-Level Architecture

```
┌─────────────────────────────────────────────────────────────────────────┐
│                           Amadeus SDK                                   │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  ┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐    │
│  │     Agent       │     │    Supervisor   │     │      Mesh       │    │
│  │     Loop        │     │    (Workers)    │     │  (Coordination) │    │
│  └────────┬────────┘     └────────┬────────┘     └────────┬────────┘    │
│           │                       │                       │             │
│           └───────────────────────┼───────────────────────┘             │
│                                   │                                     │
│           ┌───────────────────────┴───────────────────────┐             │
│           │              Tool Registry                    │             │
│           │  ┌─────────────────────────────────────────┐  │             │
│           │  │ bash │ file │ glob │ grep │ web │ ...  │ │  │             │
│           │  └─────────────────────────────────────────┘  │             │
│           └───────────────────────┬───────────────────────┘             │
│                                   │                                     │
│           ┌───────────────────────┴───────────────────────┐             │
│           │              Policy System                    │             │
│           │     (Auto/Ask/Strict approval modes)          │             │
│           └───────────────────────┬───────────────────────┘             │
│                                   │                                     │
│  ┌────────────────────────────────┼────────────────────────────────┐    │
│  │                                ▼                                │    │
│  │  ┌──────────────────────────────────────────────────────────┐   │    │
│  │  │                    LLMClient Trait                       │   │    │
│  │  │            (Provider Abstraction Layer)                  │   │    │
│  │  └────────────────────────────┬─────────────────────────────┘   │    │
│  │                               │                                 │    │
│  │         ┌─────────────────────┴─────────────────────┐           │    │
│  │         ▼                                           ▼           │    │
│  │  ┌─────────────────┐                         ┌──────────────┐   │    │
│  │  │ AnthropicClient │                         │ OpenAIClient │   │    │
│  │  └─────────────────┘                         └──────────────┘   │    │
│  └─────────────────────────────────────────────────────────────────┘    │
│                                                                         │
│  ┌─────────────────────────────────────────────────────────────────┐    │
│  │                        Output Interfaces                        │    │
│  │  ┌─────────────────┐    ┌─────────────────┐    ┌─────────────┐  │    │
│  │  │  TUI (ratatui)  │    │  HTTP API       │    │  Streaming  │  │    │
│  │  │                 │    │  (Axum)         │    │  Events     │  │    │
│  │  └─────────────────┘    └─────────────────┘    └─────────────┘  │    │
│  └─────────────────────────────────────────────────────────────────┘    │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

## Module Structure

### Core Modules

| Module | Purpose |
|--------|---------|
| `agent/` | Agent loop, configuration, messages, events, supervisor, worker |
| `client/` | LLM provider abstraction (trait-based) |
| `tools/` | Tool registry and implementations |
| `policy/` | Approval/policy system |
| `hooks/` | Extensibility hooks |
| `error/` | Error types (thiserror-based) |

### Optional Modules (Feature-Gated)

| Module | Feature | Purpose |
|--------|---------|---------|
| `ui/` | `tui` | Terminal UI (ratatui) |
| `api/` | `api` | HTTP REST API (axum) |
| `concurrency/` | `concurrency` | Lock management |
| `supervisor/` | `supervisor` | Multi-agent coordination |
| `mesh/` | `mesh` | Distributed agent mesh |
| `mcp/` | - | Model Context Protocol |
| `skills/` | - | Reusable prompt templates |
| `benchmark/` | - | Benchmark & evaluation |

### Module Dependencies

```
lib.rs
├── agent/
│   ├── config.rs          ← Depends on: error, context
│   ├── loop_agent.rs      ← Depends on: client, tools, policy, hooks
│   ├── supervisor.rs     ← Depends on: concurrency
│   └── ...
├── client/
│   ├── mod.rs             ← Defines LLMClient trait
│   ├── anthropic.rs       ← Depends on: reqwest
│   └── openai.rs          ← Depends on: reqwest
├── tools/
│   ├── mod.rs
│   ├── registry.rs
│   ├── bash.rs            ← Depends on: std::process
│   ├── file.rs            ← Depends on: std::fs
│   └── ...
├── policy/                ← No external deps
├── hooks/                 ← Depends on: async_trait
├── ui/                    ← Depends on: ratatui
├── api/                   ← Depends on: axum
├── context.rs             ← Depends on: std::fs
├── error.rs               ← Uses: thiserror
└── ...
```

## Core Components

### 1. Agent Loop (`agent/loop_agent.rs`)

The heart of the SDK - orchestrates LLM interactions and tool execution.

```
User Prompt
    │
    ▼
┌─────────────────┐
│  Add to History │
└────────┬────────┘
         │
         ▼
┌─────────────────┐     ┌─────────────────┐
│  Call LLM       │────▶│  Parse Response │
└────────┬────────┘     └────────┬────────┘
         │                       │
         │              ┌────────┴────────┐
         │              ▼                 ▼
         │      ┌───────────┐     ┌────────────┐
         │      │  Text     │     │  Tool Call │
         │      └─────┬─────┘     └──────┬─────┘
         │            │                  │
         │            ▼                  ▼
         │      ┌─────────────────────────────┐
         │      │  Policy Check (approval)    │
         │      └──────────────┬──────────────┘
         │                     │
         │            ┌────────┴────────┐
         │            ▼                 ▼
         │    ┌──────────────┐   ┌──────────────┐
         │    │   Execute    │   │    Deny      │
         │    │   Tool       │   │    Tool      │
         │    └──────┬───────┘   └──────────────┘
         │           │
         │           ▼
         │    ┌──────────────┐
         │    │  Add Result  │
         │    │  to History  │
         │    └──────┬───────┘
         │           │
         └───────────┘
              (loop until done)
```

**Key Types:**
- `Agent<C: LLMClient>` - Main agent struct
- `AgentBuilder<C>` - Fluent builder for agent construction
- `RunResult` - Result of an agent run
- `AgentEvent` - Events emitted during execution

### 2. LLM Client Trait (`client/mod.rs`)

Abstraction layer for LLM providers:

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

**Implementations:**
- `AnthropicClient` - Anthropic Claude API
- `OpenAIClient` - OpenAI GPT API

### 3. Tool System (`tools/`)

```
Tool Trait
    │
    ├── name() -> &'static str
    ├── schema() -> &'static Value
    └── execute(input: Value) -> Result<String>

ToolRegistry
    │
    ├── register(tool: Box<dyn Tool>)
    ├── get(name: &str) -> Option<&dyn Tool>
    └── get_all_schemas() -> Vec<Value>
```

**Built-in Tools:**
| Tool | Description |
|------|-------------|
| `bash` | Execute shell commands |
| `read_file` | Read file contents |
| `write_file` | Write/create files |
| `edit_file` | Surgical file edits |
| `glob` | Pattern-based file matching |
| `grep` | Search file contents |
| `web_fetch` | Fetch web content |
| `todo` | Task management |
| `sub_agent` | Recursive sub-agent spawning |
| `peer` | Peer-to-peer agent communication |

### 4. Policy System (`policy/mod.rs`)

Three approval modes:
- **Auto** - Execute all tools automatically
- **Ask** (default) - Only dangerous operations require approval
- **Strict** - All tools require approval

**Dangerous Pattern Detection:**
- `sudo` commands
- `chmod 777`
- `rm -rf /`
- Writing to `.env`, `.pem`, `.key` files

### 5. Supervisor/Worker Pattern (`agent/supervisor.rs`)

Multi-agent coordination with:
- **Dispatch Strategies**: RoundRobin, LeastLoaded, CapabilityMatch
- **Task Queue**: Buffered task execution
- **Lock Manager**: Resource coordination

### 6. Session Management

Automatic session logging with:
- Full conversation history
- JSON or compressed JSON.gz format
- Session restoration capability

## Data Flow

### Streaming Response Flow

```
LLM API (SSE)
    │
    ▼
StreamEvent
    │
    ├─▶ TextDelta ──────────────────▶ Display
    │
    ├─▶ ThinkingDelta ──────────────▶ Display (reasoning)
    │
    ├─▶ ToolCallStart ──────────────▶ Record tool call
    │
    ├─▶ ToolCallDelta ──────────────▶ Append arguments
    │
    ├─▶ ToolCallDone ───────────────▶ Execute tool
    │
    ├─▶ TokenUsage ──────────────────▶ Track usage
    │
    └─▶ StopReason ──────────────────▶ Check if done
```

### Request/Response Types

```rust
// Message types for conversation history
enum Message {
    System { content: String },
    User { content: String },
    Assistant { content: Vec<ContentBlock> },
    Tool { tool_use_id: String, content: String },
}

// Content blocks in responses
enum ContentBlock {
    Text { text: String },
    ToolUse { id: String, name: String, input: Value },
    ToolResult { tool_use_id: String, content: String },
}
```

## Feature Flags

```toml
[features]
default = []

# Testing/Examples
api = ["axum", "tower", "tower-http", "tokio-util", "supervisor"]
tui = ["crossterm", "ratatui", "tui-textarea", "unicode-width", "colored", "lazy_static"]
test-utils = ["tempfile"]

# Concurrency & Multi-Agent
concurrency = []
supervisor = ["concurrency"]
mesh = ["supervisor"]

# Context Management
context = []

# All features
full = ["api", "tui", "concurrency", "supervisor", "mesh", "context", "test-utils"]
```

## Testing Strategy

- **Unit Tests**: Co-located with implementation
- **Integration Tests**: `tests/` directory
- **Feature-Gated Tests**: Using `#[cfg(feature = "...")]`
- **Mock LLM**: For deterministic testing

## Configuration

Environment-based configuration via `.env`:
- Provider selection (Anthropic/OpenAI)
- API keys and endpoints
- Model selection
- Working directory
- Timeout settings
- Session logging
- Context window management
- Blocked commands

## Extension Points

1. **Custom Tools**: Implement `Tool` trait
2. **Custom Hooks**: Implement `Hook` trait
3. **Custom Providers**: Implement `LLMClient` trait
4. **Skills**: YAML-based prompt templates
5. **Policy**: JSON-based approval rules
