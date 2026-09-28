# Amadeus Architecture

> Workspace-oriented architecture guide for the current Amadeus SDK.

## Overview

Amadeus is organized as a Cargo workspace with a thin compatibility facade at the root and most implementation living in dedicated crates.

- The root `amadeus` crate re-exports the workspace crates for downstream compatibility.
- `crates/core` contains the transport-agnostic agent runtime, provider clients, tool system, policy layer, orchestration runtime, and shared commands.
- `crates/runtime` contains reusable coordination models and selection logic for teams, orchestras, workers, and scheduling.
- `crates/api` is the Axum HTTP adapter.
- `crates/tui` is the ratatui terminal adapter.
- Supporting crates such as `config`, `commands`, `skills`, `telemetry`, `messages`, `events`, and `permissions` hold reusable building blocks consumed by `core`.

The practical mental model is:

`CLI or library call -> config + provider selection -> core runtime -> TUI or HTTP adapter`

## Workspace Shape

The root `amadeus` crate is a compatibility facade and the CLI entry point; implementation lives in `crates/`.

```text
amadeus/
├── src/                    # compatibility facade + CLI mode switch
├── crates/                 # implementation crates (table below)
├── tests/                  # integration suites and shared harnesses
├── examples/               # adapter bootstraps
└── docs/
```

| Crate | Role |
|-------|------|
| `crates/core` | Agent loop, LLM clients, tools, policy, hooks, orchestration |
| `crates/runtime` | Orchestration models, worker selection, task dispatch |
| `crates/api` | Axum HTTP + SSE server |
| `crates/tui` | ratatui terminal UI adapter |
| `crates/config` | Layered settings loading |
| `crates/events` | Shared event model (`AgentEvent`, `RunResult`, …) |
| `crates/messages` | Message and content block types |
| `crates/compaction` | Context-window compaction triggers and results |
| `crates/context` | Project context loading, memory providers |
| `crates/memory` | Mid-term memory database and context gate |
| `crates/memory-domain` | Versioned domain memory models |
| `crates/memory-service` | Structured, privacy-aware memory service |
| `crates/privacy` | Sensitive-data detection and redaction |
| `crates/rag` | Semantic search, embedding backends, vector store |
| `crates/telemetry` | Structured event recording with pluggable sinks |
| `crates/permissions` | Permission modes and enforcement |
| `crates/hooks` | Pre/post-tool hook descriptors |
| `crates/profiles` | Agent profile definitions |
| `crates/prompts` | System prompt templating |
| `crates/commands` | Slash commands and citation handling |
| `crates/skills` | Prompt template skill loading |
| `crates/ids` | Identity types (`AgentId`, `TeamId`) |

## Entry Points

### Root facade

`src/lib.rs` is intentionally small. It re-exports `amadeus_core::*` and conditionally re-exports the API and TUI adapters.

That means library users can still import through `amadeus::...`, while implementation continues to move into workspace crates.

### CLI bootstrap

`src/main.rs` is a mode switch, not the main runtime layer.

Its flow is:

1. Parse flags.
2. Load config.
3. Build the configured LLM client.
4. Branch into one of:
   - assessment mode
   - HTTP server mode
   - TUI mode

In other words, the binary selects an adapter and hands control to the shared runtime.

## Core Runtime

### `crates/core`

`crates/core` is the heart of the system. It owns:

- the ReAct-style `Agent` loop
- provider abstractions and concrete clients
- tool registration and execution
- approval and permission handling
- hooks and telemetry
- orchestration surfaces such as `AgentOrchestrator` and `OrchestraRuntime`

Important module groups:

| Module | Responsibility |
|---|---|
| `agent/loop_agent.rs` | Main agent loop, history, streaming, approvals, session logging |
| `agent/orchestra.rs` | Local orchestration surface and queued orchestration runtime |
| `client/` | `LLMClient` trait plus Anthropic and OpenAI implementations |
| `tools/` | Tool trait, registry, built-in tools, peer and sub-agent tools |
| `policy/` | Dangerous-operation policy decisions |
| `permissions.rs` | Permission modes and enforcement hooks |
| `hooks/` | Hook loading and execution |
| `assessment/` | Read-only feature assessment flow |
| `benchmark/` | Benchmark runner and reporting |

### `crates/runtime`

`crates/runtime` provides the provider-independent execution and coordination foundation used by higher layers. The current live ReAct agent has not migrated to the workflow kernel yet; it still runs in `crates/core` during the compatibility phase.

It contains:

- typed workflow nodes, transitions, validation, suspension, and bounded execution
- team and orchestra state models
- worker/task models
- dispatch strategies and worker selection
- transport-agnostic helpers for agent routing

This split keeps workflow and coordination semantics reusable while `crates/core` supplies model clients, tools, policy, memory, and the current live agent implementation.

### Workflow kernel

`crates/runtime/src/workflow.rs` is the first migration slice toward architecture-defined agents. It models an agent architecture as typed state moving through asynchronous nodes:

```text
Agent = Architecture + State + Resources + Runtime
```

The kernel is intentionally unaware of LLM messages, provider clients, concrete tools, and frontends. A workflow node receives shared resources and mutable state, then returns one of four transitions:

- continue at another node
- suspend and return owned state
- complete successfully
- fail with an architecture-defined error

Declared destinations are validated before execution, and the runner applies one transition limit across initial execution and resumed suspensions.

```rust
let workflow = Workflow::builder("plan")
    .node("plan", plan_node)?
    .node("act", act_node)?
    .build()?;

let status = WorkflowRunner::default()
    .run(&workflow, &resources, initial_state)
    .await?;
```

The existing `Agent<C>` API remains the compatibility surface. Its ReAct loop will be decomposed into reusable core nodes and then reimplemented as a workflow preset without changing public behavior. The staged design and compatibility gates are documented in [`plans/2026-08-20-agent-architecture-runtime.md`](plans/2026-08-20-agent-architecture-runtime.md).

### Workflow-backed agents

The workflow kernel separates reusable architecture from configured agent identity and per-run state:

```text
Workflow<S, R>                         reusable architecture blueprint
    ↓ bound with identity + resources
WorkflowAgent<S, R>                    one configured agent instance
    ↓ run(initial_state)
WorkflowAgentRun<S>                    one execution session
    ├── Completed(final_state)
    └── Suspended(WorkflowAgentCheckpoint<S>)

WorkflowAgentRegistry<S, R>
    ├── Agent A → Workflow A + Resources A
    ├── Agent B → Workflow B + Resources B
    └── Agent C → shared Workflow A + Resources C
```

Each `WorkflowAgent` binds exactly one workflow for its lifetime. Several agents may share the same `Arc<Workflow<_, _>>`, or use different workflow graphs. Every run receives owned state, and suspended checkpoints record the owning agent so they cannot be resumed by another agent accidentally.

`WorkflowAgentRegistry` provides typed registration, lookup, removal, execution routing, and checkpoint routing. Agents in one registry share Rust state and resource types `S` and `R`; applications that need heterogeneous implementations can use an enum state and trait-object resource bundle while preserving a common routing contract.

The complete hierarchy and usage example are documented in [`AGENT_ARCHITECTURES.md`](AGENT_ARCHITECTURES.md).

## Request and Event Flows

### Agent loop

The core execution loop in `crates/core/src/agent/loop_agent.rs` follows a ReAct pattern:

1. Compaction check — if the context window exceeds the configurable threshold (default 75%), summarize older messages to reclaim tokens.
2. Add user input to history.
3. Call the configured `LLMClient`.
4. Stream or parse model output.
5. If the model emits tool calls, run policy and permission checks.
6. Execute tools through the `ToolRegistry`.
7. Append tool results to history.
8. Continue until a final text result is produced.

```text
User/Input
   ↓
History update
   ↓
LLMClient call
   ↓
Model output
   ├── text delta -> emit events / accumulate final response
   └── tool call -> policy + permissions -> execute tool -> append result -> loop
```

Sub-agents are spawned as full child `Agent` instances with namespaced event IDs, bounded recursion depth, and optional UI delegation.

### Tool system

Every tool implements the shared `Tool` trait and is registered in a `ToolRegistry`.

The registry is responsible for:

- exposing schemas to the model
- dispatching execution by tool name
- composing default tool packs
- adding recursive sub-agent and peer capabilities when enabled

Built-in tools include bash, file, glob, grep, web, todo, sub-agent, and peer collaboration surfaces.

### Orchestration surfaces

Amadeus has two related but distinct orchestration surfaces in `agent/orchestra.rs`.

`AgentOrchestrator`
- Manages the local roster of agents.
- Supports create/list/get/switch/kill operations.
- Routes direct tasks to one local agent.
- Is the main orchestration surface used by the current HTTP server.

`OrchestraRuntime`
- Adds queued background execution, help-request handling, and worker scheduling.
- Uses channels plus a periodic processing loop.
- Is the heavier coordination runtime for delegated and queued work.

The distinction matters because current docs sometimes blur them together. The HTTP adapter currently uses `AgentOrchestrator`, not the queued `OrchestraRuntime`.

## Adapter Architecture

### HTTP adapter

`crates/api` is an Axum wrapper around the core runtime.

`run_server` builds shared `AppState` containing:

- the shared base client
- loaded config
- an `AgentOrchestrator`
- the default orchestra id used by stateless task endpoints

Current ingress map:

| Path | Actual runtime path |
|---|---|
| `/v1/sessions/*` | `LocalSessionBridge` stateful sessions, events, approvals, history, cancellation, and checkpoints |
| `POST /chat` | request -> `Task` -> `AgentOrchestrator::execute_task` -> `Agent::run` |
| `POST /tasks` | request -> `Task` -> `AgentOrchestrator::execute_task` |
| `GET /v1/sessions/:id/events` | subscribe to `LocalSessionBridge` session events |
| `POST /execute` | instantiate `BashTool` directly and execute it |

Two important details:

- The removed legacy `/stream` route bypassed the orchestrator and created a fresh agent for SSE output.
- `/execute` is a direct tool endpoint, not a normal agent-turn path.

The canonical external interactive interface is documented in `docs/HTTP_API.md`. It uses `/v1/sessions/*`; unversioned stateless and administrative routes are not a substitute for the live-session contract.

### TUI adapter

`crates/tui` is an in-process ratatui frontend over the core `Agent`.

Its flow is:

1. Read terminal events through the TUI event handler.
2. Update `App` state.
3. Push user messages into agent history.
4. Start `agent.run_stream_with_approval(...)`.
5. Render incoming agent events, tool activity, approvals, and session state.

The TUI is not an API client. It talks to the same in-process runtime used by the library and server.

## Provider Layer

The provider abstraction lives behind `LLMClient` in `crates/core/src/client/mod.rs`.

The two primary implementations are:

- `AnthropicClient`
- `OpenAIClient`

The agent and orchestration types are generic over `C: LLMClient`, which keeps provider swapping explicit and avoids pushing dynamic dispatch through the hot path.

## Configuration

Configuration is loaded from the dedicated `config` crate and merged from layered settings roots under `.amadeus/`.

Important runtime concerns include:

- provider and model selection
- workdir resolution
- permission mode and rules
- tool profile settings
- session logging
- compaction thresholds
- hooks and telemetry

## Feature Flags

The root crate intentionally has no default features.

Current top-level feature relationships from `Cargo.toml`:

```toml
default = []

api = ["amadeus_core/api", "dep:amadeus_api", "orchestra"]
tui = [
  "amadeus_core/tui",
  "dep:amadeus_tui",
  "dep:crossterm",
  "dep:ratatui",
  "concurrency",
]
test-utils = ["amadeus_core/test-utils", "amadeus_tui?/test-utils", "tempfile", "rustc_version_runtime"]

concurrency = ["amadeus_core/concurrency"]
orchestra = ["amadeus_core/orchestra", "concurrency"]
context = ["amadeus_core/context"]

full = ["api", "tui", "concurrency", "orchestra", "context", "test-utils"]
```

Important implications:

- `api` implies `orchestra`
- `tui` implies `concurrency`
- `orchestra` implies `concurrency`
- removed legacy names (`team`, `supervisor`, and `mesh`) are not active features

| Feature | Description |
|---------|-------------|
| `api` | HTTP adapter and REST/SSE server (implies `orchestra`) |
| `tui` | Terminal UI adapter (implies `concurrency`) |
| `concurrency` | Locking and shared coordination primitives |
| `orchestra` | Multi-agent orchestration surface (implies `concurrency`) |
| `context` | Context management and memory providers |
| `test-utils` | Test helpers and recording support |
| `full` | All of the above |

## Testing Structure

Tests are split across three layers.

### Unit tests

Many workspace crates, especially `crates/core`, keep unit tests inline with implementation modules.

### Root integration tests

The root `tests/` directory covers end-to-end behavior such as:

- agent runs
- approvals
- compaction
- telemetry
- file locking
- orchestration and P2P delegation
- TUI snapshots and scenarios

### Shared harnesses

Reusable testing infrastructure lives under:

- `tests/scenarios/`
- `tests/fixtures/scenarios/`
- `tests/mock_llm.rs`

Feature gating is mixed:

- some integration tests are gated in `Cargo.toml` with `required-features`
- some use `cfg(feature = "...")`
- many simply assume `--features full`

For contributor workflows, the safest default remains:

```bash
cargo check --features full
cargo test --features full
```

## Architectural Notes

- The root crate is now mostly a compatibility surface.
- The current public architecture is workspace-first, not monolithic.
- `AgentOrchestrator` is the main local orchestration API today.
- `OrchestraRuntime` is the queued/background coordination layer.
- HTTP and TUI are adapters over the same in-process runtime rather than independent implementations.
- Documentation should only claim behavior that is both implemented and exercised by tests where practical.
