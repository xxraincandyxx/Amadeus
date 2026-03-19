# Amadeus Architecture

> AI Agent SDK - Production-ready multi-agent system with LLM support

## Overview

Amadeus is a Rust SDK for building AI agents with comprehensive LLM support. It provides multi-provider compatibility (Anthropic Claude, OpenAI GPT), streaming responses, an extensible tool system, and both TUI and HTTP API interfaces.

**Key Capabilities:**
- Concurrent execution with `tokio::task::JoinSet` for parallel task processing
- Task queuing and backpressure control via centralized `TaskQueue`
- P2P recursive delegation through `PeerTool` for inter-agent collaboration
- Resilient error handling with deadlock prevention and saturation management

---

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
│           │  │ bash │ file │ glob │ grep │ web │ ...   │  │             │
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

---

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
│   ├── supervisor.rs      ← Depends on: concurrency
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

---

## Core Components

### 1. Agent Loop (`agent/loop_agent.rs`)

The heart of the SDK - orchestrates LLM interactions and tool execution using the ReAct (Reason + Act) pattern.

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
- `Agent<C: LLMClient>` - Main agent struct, generic over LLM provider
- `AgentBuilder<C>` - Fluent builder for agent construction
- `RunResult` - Result of an agent run
- `AgentEvent` - Events emitted during execution

### 2. LLM Client Trait (`client/mod.rs`)

Abstraction layer for LLM providers, enabling zero-cost provider switching:

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

### 5. Session Management

Automatic session logging with:
- Full conversation history
- JSON or compressed JSON.gz format
- Session restoration capability

---

## Multi-Agent Orchestration

### Supervisor-Worker Pattern

Amadeus uses a **Supervisor-Worker** pattern where a central supervisor manages a pool of specialized agents.

| Feature | Implementation |
|---------|----------------|
| **Concurrency** | Parallel task execution via `JoinSet` |
| **Queuing** | Async `VecDeque` with periodic processing |
| **Load Balancing** | `LeastLoaded`, `RoundRobin`, and `CapabilityMatch` strategies |
| **P2P Help** | Recursive sub-tasking via the `HelpRequest` bus |

### The Supervisor Loop

The Supervisor runs a reactive background loop that handles two main event sources:
1. **P2P Help Requests**: Incoming from agents via `HelpRequest` channels
2. **Task Queue**: Periodic processing of pending tasks whenever workers become available

```rust
pub async fn run(&self) -> Result<()> {
    loop {
        tokio::select! {
            help_req = self.help_rx.recv() => {
                // Dispatch or fail immediately if no workers
            }
            _ = interval.tick() => {
                self.process_queue().await;
            }
        }
    }
}
```

### Task Buffering

When `Supervisor::execute` is called and no workers are immediately available, the task is pushed into a `VecDeque`. This ensures bursty traffic doesn't fail immediately, provided it stays within the `max_pending_tasks` limit.

```
┌─────────────────────────────────────────────────────────────────┐
│                        SUPERVISOR                               │
│                                                                 │
│   Task Queue: [Task1, Task2, Task3, ...]                        │
│                                                                 │
│   ┌─────────┐  ┌─────────┐  ┌─────────┐  ┌─────────┐            │
│   │Worker A │  │Worker B │  │Worker C │  │Worker D │            │
│   │ 2 tasks │  │ 0 tasks │  │ 3 tasks │  │ 1 task  │            │
│   │ [bash]  │  │ [web]   │  │ [file]  │  │ [bash]  │            │
│   └─────────┘  └─────────┘  └─────────┘  └─────────┘            │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### Dispatch Strategies

The Supervisor supports three load balancing strategies for distributing tasks across worker agents.

#### RoundRobin (default)

Cycles through workers in order, regardless of current load.

```rust
DispatchStrategy::RoundRobin => {
    let mut next_idx = next_idx_mutex.lock().await;
    let idx = *next_idx % candidates.len();
    *next_idx += 1;
    Some(candidates[idx].0)
}
```

**Example:**
```
Task 1 → Worker A
Task 2 → Worker B
Task 3 → Worker C
Task 4 → Worker A  (cycles back)
Task 5 → Worker B
```

| Pros | Cons |
|------|------|
| Simple, predictable | Ignores current workload |
| Even distribution over time | Busy workers get equal share |
| No state tracking beyond index | Can queue on overloaded workers |

**Best for:** Homogeneous tasks, equal worker capacity

#### LeastLoaded

Always picks the worker with the fewest active tasks.

```rust
DispatchStrategy::LeastLoaded => candidates
    .iter()
    .min_by_key(|(_, info)| info.active_tasks)
    .map(|(id, _)| *id)
```

**Example:**
```
Current state:  A(2 tasks), B(0 tasks), C(3 tasks), D(1 task)
Task arrives → Worker B (has 0 active tasks)
Next state:    A(2), B(1), C(3), D(1)
Another task → Worker B or D (tie, both have 1)
```

| Pros | Cons |
|------|------|
| Balances load dynamically | Requires tracking active_tasks per worker |
| Prevents hot spots | Doesn't consider task complexity |
| Better resource utilization | Race conditions possible (handled via locks) |

**Best for:** Variable task durations, uneven workloads

#### CapabilityMatch

First filters workers by required capabilities, then picks least loaded among them.

```rust
DispatchStrategy::CapabilityMatch => candidates
    .iter()
    .filter(|(_, info)| info.has_capabilities(&task.required_capabilities))
    .min_by_key(|(_, info)| info.active_tasks)
    .map(|(id, _)| *id)
```

**Example:**
```
Workers:
  Worker A - capabilities: [bash, file]     - active: 2
  Worker B - capabilities: [web, search]    - active: 0
  Worker C - capabilities: [bash, docker]   - active: 1
  Worker D - capabilities: [web, bash]      - active: 3

Task arrives requiring: [bash]
Eligible: A, C, D (all have bash)
Selected: Worker C (has bash, lowest active count = 1)
```

| Pros | Cons |
|------|------|
| Routes tasks to specialized workers | Can fail if no capable worker available |
| Prevents dispatching to incapable workers | More complex matching logic |
| Combines capability filtering with load balancing | Requires capability declaration at spawn |

**Best for:** Heterogeneous workers, specialized tasks (e.g., web scraping, code execution)

#### Strategy Selection Guide

| Scenario | Best Strategy | Why |
|----------|---------------|-----|
| Quick uniform tasks | RoundRobin | Simplicity, even spread |
| Mixed short/long tasks | LeastLoaded | Prevents queuing |
| Specialized workers | CapabilityMatch | Routes to right worker |
| Unknown task profiles | LeastLoaded | Safe default |

#### Configuration

Set the dispatch strategy in `SupervisorConfig`:

```rust
let config = SupervisorConfig {
    strategy: DispatchStrategy::LeastLoaded,
    max_pending_tasks: 100,
    task_timeout: Duration::from_secs(300),
    retry_failed_tasks: true,
    max_retries: 3,
};
```

#### Worker Capabilities

Workers declare capabilities when spawned:

```rust
let worker_configs = vec![
    WorkerConfig {
        name: "code-executor".to_string(),
        capabilities: vec!["bash".to_string(), "file".to_string()],
        max_concurrent: 3,
        ..Default::default()
    },
    WorkerConfig {
        name: "web-scraper".to_string(),
        capabilities: vec!["web".to_string(), "search".to_string()],
        max_concurrent: 2,
        ..Default::default()
    },
];
```

---

## P2P Collaboration (Help System)

### The PeerTool

Agents are initialized with a `PeerTool`, which allows them to send `HelpRequest`s back to the Supervisor. This enables recursive collaboration where a Coder agent can ask a Reviewer agent for feedback mid-task.

### Deadlock Prevention

To prevent circular dependency deadlocks (e.g., Worker A waits for Worker B, who is waiting for Worker A), the Supervisor implements:
1. **Timeout Enforcement**: Every task has a `task_timeout`
2. **Saturation Errors**: If a help request cannot be fulfilled because all potential workers are busy, it returns an error immediately rather than queuing indefinitely (which would block the requester)

---

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

---

## Performance

### Concurrent Execution

Tasks are spawned as independent Tokio tasks. In a batch of 5 tasks taking 2s each, total time is ~2s instead of 10s (5x speedup).

### Backpressure Control

The `SupervisorConfig::max_pending_tasks` (default: 100) prevents OOM and API exhaustion by rejecting new tasks when the buffer is full.

---

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

Feature flag chain: `mesh` → `supervisor` → `concurrency`

---

## Testing Strategy

- **Unit Tests**: Co-located with implementation
- **Integration Tests**: `tests/` directory
- **Feature-Gated Tests**: Using `#[cfg(feature = "...")]`
- **Mock LLM**: For deterministic testing without API calls

---

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

---

## Extension Points

1. **Custom Tools**: Implement `Tool` trait
2. **Custom Hooks**: Implement `Hook` trait
3. **Custom Providers**: Implement `LLMClient` trait
4. **Skills**: YAML-based prompt templates
5. **Policy**: JSON-based approval rules

---

## Implementation Status

| Feature | Status | Notes |
|---------|--------|-------|
| Agent Loop (ReAct) | ✅ Complete | Core orchestration |
| LLM Client Trait | ✅ Complete | Anthropic, OpenAI |
| Tool System | ✅ Complete | 10+ built-in tools |
| Policy System | ✅ Complete | Auto/Ask/Strict |
| Concurrent Execution (JoinSet) | ✅ Complete | Parallel processing |
| Task Queuing & Backpressure | ✅ Complete | Centralized TaskQueue |
| P2P Help System | ✅ Complete | PeerTool integration |
| Supervisor/Worker | ✅ Complete | Multi-agent coordination |
| TUI Interface | ✅ Complete | ratatui-based |
| HTTP API | ✅ Complete | axum-based |
| Actor-based Agents | ⏳ Planned | Persistent tasks with mailboxes |
| Delta State Management | ⏳ Planned | Surgical state updates |

---

## Roadmap

1. **Actor-Based Agents** - Transform agents into persistent tasks with mailboxes to support `Pause`/`Resume` and better state persistence
2. **Delta State** - Implement surgical state updates to handle large workspaces efficiently

---

*Document Version: 2.0*
*Last Updated: 2026-03-19*
