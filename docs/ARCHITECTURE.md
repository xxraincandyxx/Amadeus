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

```text
amadeus/
├── src/
│   ├── lib.rs              # compatibility facade
│   └── main.rs             # CLI mode switch
├── crates/
│   ├── core/               # agent loop, tools, policy, orchestration
│   ├── runtime/            # shared orchestration data models and selectors
│   ├── api/                # Axum router + handlers
│   ├── tui/                # ratatui application
│   ├── config/             # layered settings loading
│   ├── commands/           # slash command parsing and helpers
│   ├── skills/             # skill registry and loading
│   ├── telemetry/          # sinks and events
│   ├── permissions/        # permission modes and rules
│   ├── messages/           # message/content block types
│   ├── events/             # event and tool-call payloads
│   └── ...
├── tests/                  # integration suites and shared harnesses
├── examples/               # adapter bootstraps
└── docs/
```

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

`crates/runtime` does not run the live agent loop. Instead, it provides reusable coordination types and algorithms used by the core orchestration layer.

It contains:

- team and orchestra state models
- worker/task models
- dispatch strategies and worker selection
- transport-agnostic helpers for agent routing

This split keeps the shared coordination semantics reusable while `crates/core` handles the live async execution with real agents.

## Request and Event Flows

### Agent loop

The core execution loop in `crates/core/src/agent/loop_agent.rs` follows a ReAct pattern:

1. Add user input to history.
2. Call the configured `LLMClient`.
3. Stream or parse model output.
4. If the model emits tool calls, run policy and permission checks.
5. Execute tools through the `ToolRegistry`.
6. Append tool results to history.
7. Continue until a final text result is produced.

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
| `POST /chat` | request -> `Task` -> `AgentOrchestrator::execute_task` -> `Agent::run` |
| `POST /tasks` | request -> `Task` -> `AgentOrchestrator::execute_task` |
| `GET /stream` | build fresh `Agent` -> inject user message -> `run_stream()` |
| `POST /execute` | instantiate `BashTool` directly and execute it |
| `/agents/*` | mostly agent roster management; some task endpoints are still provisional |

Two important details:

- `/stream` bypasses the orchestrator and creates a fresh agent for SSE output.
- `/execute` is a direct tool endpoint, not a normal agent-turn path.

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
team = ["orchestra"]       # legacy alias
supervisor = ["orchestra"] # legacy alias
context = ["amadeus_core/context"]

full = ["api", "tui", "concurrency", "orchestra", "context", "test-utils"]
```

Important implications:

- `api` implies `orchestra`
- `tui` implies `concurrency`
- `orchestra` implies `concurrency`
- `team` and `supervisor` are compatibility aliases, not separate subsystems
- there is no active `mesh` feature in the current manifest

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
- `tests/tui/`
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
