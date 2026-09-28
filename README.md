<div align="center">

English · [简体中文](README.zh-CN.md)

# Amadeus

**A composable, multi-provider AI agent framework in Rust.**

Typed workflows · ReAct agent loop · tiered memory · RAG · native macOS app, TUI, and REST API

[![CI](https://github.com/xxraincandyxx/Amadeus/actions/workflows/ci.yml/badge.svg)](https://github.com/xxraincandyxx/Amadeus/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange.svg)](https://www.rust-lang.org)
[![Platform](https://img.shields.io/badge/platform-macOS%20%7C%20Linux-lightgrey.svg)](#)
[![PRs Welcome](https://img.shields.io/badge/PRs-welcome-brightgreen.svg)](CONTRIBUTING.md)

[Quickstart](#quickstart) · [Architecture](#architecture) · [Memory](#tiered-memory) · [Web & macOS](#web-workspace--macos-app) · [HTTP API](#http-api) · [Python SDK](#python-sdk) · [TUI](#tui) · [Docs](#documentation)

</div>

---

## Highlights

- **Native macOS desktop app (primary client)** — a native webview workspace with a supervised embedded server: live sessions, tools, approvals, and checkpoints.
- **Composable agent architectures** — typed async workflows and multiple configurable agents in one registry; models and tools are injected resources.
- **Multi-provider LLMs** — Anthropic Claude and OpenAI GPT behind a generic `LLMClient` trait.
- **ReAct agent loop** — streaming turns with tool execution, context compaction, and retryable errors.
- **Extensible tool system** — shell, files, search, and web built in; custom tools via the `Tool` trait; MCP integration.
- **Three-layer safety** — hooks, permission modes, and Auto / Ask / Strict approval.
- **Multi-agent orchestration** — distinct agent profiles, capability-based routing, priority task queue.
- **Tiered memory + RAG** — short/mid/long-term memory plus a persistent vector store with pluggable embeddings (see [Tiered memory](#tiered-memory)).
- **HTTP API** — Axum REST + SSE, 30+ endpoints; the shared backend every client talks to.
- **Interactive TUI (secondary client)** — inline ratatui UI with approvals, 12 themes, and conversation export.
- **Telemetry** — structured event recording with pluggable sinks.
- **Session management** — persistence, restore, checkpoints with rewind, Markdown/JSON export.

## Preview

<table>
  <tr>
    <td width="50%" align="center">
      <img width="100%" alt="Amadeus native macOS app and web workspace — multi-agent sessions, reasoning disclosure, and live markdown" src="assets/web_preview.jpg">
      <br>
      <strong>Native macOS app &amp; web workspace</strong><br>
      <sub>Multi-agent sessions, reasoning disclosure, live markdown</sub>
    </td>
    <td width="50%" align="center">
      <img width="100%" alt="Amadeus task workflow designer — node-based control flow canvas" src="assets/web_workflow.jpg">
      <br>
      <strong>Task workflow designer</strong><br>
      <sub>Node-based canvas for task control flow</sub>
    </td>
  </tr>
</table>

## Quickstart

### Prerequisites

- Rust 1.70 or later
- An API key from Anthropic or OpenAI — or a local model (see [Run a local model](#run-a-local-model-no-api-key))

### Setup

```bash
git clone https://github.com/xxraincandyxx/Amadeus.git
cd Amadeus
mkdir -p .amadeus && cp .amadeus/settings.example.json .amadeus/settings.json
# Edit .amadeus/settings.json — add your API key and pick a provider
cargo build --release --features full
```

> [!TIP]
> Amadeus has no default features — use `--features full` for repository development and everyday use.

### Native macOS desktop app (primary)

Run it from source — builds the server sidecar, then opens the native window:

```bash
cd apps/client
npm install
npm run desktop:dev
```

Build a distributable application bundle:

```bash
cd apps/client
npm run desktop:build
# -> apps/client/src-tauri/target/release/bundle/macos/Amadeus.app
```

The app starts and supervises its own server on port 3000; if a server already
owns that port it reuses it. See [docs/MACOS_APP.md](docs/MACOS_APP.md).

### HTTP API server

```bash
# Default port 3000
cargo run --features full -- --server

# Custom port
cargo run --features full -- --server 8080
```

### Interactive terminal UI (secondary)

```bash
cargo run --features full
```

### Run a local model (no API key)

Amadeus talks to any OpenAI-compatible server, so you can develop against a
model running on your own machine. The bundled setup script downloads
`Qwen2.5-0.5B-Instruct` (GGUF, ~400 MB, cached under `.amadeus/models/`) and
serves it through [llama.cpp](https://github.com/ggml-org/llama.cpp)'s
OpenAI-compatible server on `127.0.0.1:8123`:

```bash
brew install llama.cpp   # macOS; on Linux build llama.cpp and add it to PATH
scripts/setup_local_llm.sh
```

Then point `.amadeus/settings.json` at it:

```json
{
  "provider": "openai",
  "api_key": "local",
  "base_url": "http://127.0.0.1:8123/v1",
  "model": "qwen2.5-0.5b-instruct"
}
```

`base_url` can also be a bare host (`http://127.0.0.1:8123`) or a full
endpoint — the client appends `/v1/chat/completions` as needed. LM Studio,
Ollama, and vLLM speak the same protocol; adjust `base_url`, port, and
`model` to match yours.

## Architecture

A shared core runtime, with every frontend as a thin adapter over it:

```mermaid
flowchart TB
    subgraph frontends [Frontends]
        direction LR
        TUI["TUI (ratatui)"]
        API["HTTP API (Axum, REST + SSE)"]
        WEB["Web workspace (React + Tauri)"]
        SDK["Python SDK"]
    end

    subgraph core [Core runtime — crates/core]
        LOOP["ReAct agent loop<br/>streaming · compaction · retries"]
        GATE["Three-layer safety gate<br/>hooks → permissions → policy"]
        TOOLS["Tool registry<br/>bash · files · search · web · MCP"]
        ORCH["Multi-agent orchestration"]
    end

    subgraph providers [LLM providers]
        CLAUDE["Anthropic Claude"]
        GPT["OpenAI GPT"]
    end

    subgraph state [State and knowledge]
        MEM["Tiered memory<br/>short · mid · long"]
        RAG["RAG vector store<br/>pluggable embeddings"]
        TEL["Telemetry sinks"]
    end

    TUI --> LOOP
    API --> LOOP
    WEB --> API
    SDK --> API
    ORCH --> LOOP
    LOOP --> CLAUDE
    LOOP --> GPT
    LOOP --> GATE --> TOOLS
    LOOP --> MEM
    LOOP --> RAG
    LOOP --> TEL
```

### Three-layer safety gate

| Layer | Purpose |
|-------|---------|
| **Hooks** | Extensible pre/post-tool interceptors that can modify input or block execution |
| **Permissions** | Mode-based hard blocks: `ReadOnly`, `WorkspaceWrite`, `DangerFullAccess`, `Prompt` |
| **Policy** | Runtime approval: `Auto` (none), `Ask` (dangerous only), `Strict` (all) |

## Tiered memory

Memory is organized in three tiers with different lifetimes:

| Tier | What it is | Where it lives | Lifetime |
|------|------------|----------------|----------|
| **Short-term** | The live context window: conversation history, session logs, compaction summaries | `Agent.history`, session JSON logs | One session / until compaction |
| **Mid-term** | A record database of what the conversation established: tasks, decisions, files touched, errors and resolutions, state snapshots | `crates/memory` → `.amadeus/mid_term_memory.json` | Across sessions, on disk |
| **Long-term** | Durable user/LLM-stored facts and semantic RAG | `JsonFileMemoryProvider` (`.amadeus/memory.json`), `VectorMemoryProvider` (`.amadeus/rag_index.json`) | Permanent |

When compaction retires context, a rule-based gate redacts sensitive data and upserts the surviving records into the mid tier. Full contract and storage format: [docs/MEMORY.md](docs/MEMORY.md).

## Built-in tools

| Tool | Description | Permission |
|------|-------------|------------|
| `bash` | Execute shell commands | Requires approval for dangerous commands |
| `read_file` | Read file contents | Auto-approved |
| `write_file` | Write or create files | Requires approval for sensitive paths |
| `edit_file` | Surgical file edits with diff rendering | Requires approval |
| `glob` | Pattern-based file matching | Auto-approved |
| `grep` | Search file contents with regex | Auto-approved |
| `web_fetch` | Fetch and render web page content | Requires approval |
| `todo` | Task tracking and planning | Auto-approved |
| `rag` | Ingest, search, and manage a vector knowledge base | Runtime |
| `memory` | Store and retrieve session-scoped notes | Runtime |

## Configuration

Structured settings live in `.amadeus/settings.json`, with global defaults in `~/.amadeus/settings.json` and workspace overrides in `.amadeus/settings.local.json`:

```json
{
  "provider": "anthropic",
  "api_key": "sk-ant-xxx",
  "base_url": "https://api.anthropic.com",
  "model": "claude-sonnet-4-5-20250929",
  "timeout_seconds": 120,
  "max_output_bytes": 50000,
  "session_log_dir": "./logs",
  "session_log_compress": true,
  "blocked_commands": ["rm -rf /", "sudo"],
  "tui": {
    "language": "en"
  }
}
```

The TUI supports English (`en`, the default) and Simplified Chinese (`zh-CN`). Set `tui.language` in any settings layer, or switch the current session with `/language en` and `/language zh-CN` (`/lang` is an alias). See [`.amadeus/README.md`](.amadeus/README.md) for the full configuration reference.

## Frontends

**One runtime, four ways to drive it** — the native desktop app or the browser workspace, the terminal, raw HTTP, and Python.

### Web workspace & macOS app (primary)

One React agent workspace, two shells: run it in a browser against any Amadeus server, or as the native macOS app that supervises its own embedded server. Live history, SSE events, tools, approvals, cancellation, and checkpoints ride the stable `/v1/sessions/*` API.

Startup instructions live in [`apps/client/README.md`](apps/client/README.md), desktop builds in [`docs/MACOS_APP.md`](docs/MACOS_APP.md), and the design contract in [`docs/WEB_DESIGN_SYSTEM.md`](docs/WEB_DESIGN_SYSTEM.md).

### HTTP API

Start it with `--server [port]` (default 3000) — 30+ REST endpoints plus SSE streaming. Highlights:

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/health` | Health check |
| `POST` | `/chat` | Stateless single-turn chat |
| `POST` | `/execute` | Direct bash command execution |
| `GET` | `/v1/sessions/:id/events` | Stable SSE stream for a live session |
| `POST` | `/v1/sessions/:id/messages` | Start an asynchronous live-session turn |
| `POST` | `/v1/sessions/:id/approvals/:approval_id` | Resolve a session-scoped approval |
| `GET/PUT` | `/v1/sessions/:id/checkpoint` | Capture or restore a checkpoint |
| `POST` | `/tasks` | Multi-agent task dispatch |
| `POST` | `/rag/ingest` · `/rag/query` | Ingest and search the vector store |
| `GET/PUT/PATCH` | `/config` · `/tools/catalog` · `/skills` | Runtime configuration and catalogs |

The full endpoint reference lives in [docs/HTTP_API.md](docs/HTTP_API.md).

> [!WARNING]
> The API has no built-in authentication and full CORS enabled — it is designed for trusted internal use or deployment behind a reverse proxy.

### Python SDK

An async Python client for the HTTP API lives in [`python-sdk`](python-sdk):

```python
import asyncio
from amadeus_sdk import Agent

async def main():
    async with Agent("http://localhost:3000") as agent:
        turn = await agent.send("What is the current directory?")
        print(turn.text)

asyncio.run(main())
```

See [`python-sdk/README.md`](python-sdk/README.md) for installation and the full API surface.

### TUI (terminal client)

<p align="center">
  <img width="48.5%" alt="Amadeus TUI — streaming ReAct turn with tool groups, markdown rendering, and a status footer" src="assets/tui_preview.jpg">
  <br>
  <strong>Interactive TUI</strong> — streaming ReAct turn with tool groups and a status footer
</p>

The terminal UI is an inline-mode application that sits at the bottom of your terminal with scrollable conversation history above.

<details>
<summary><strong>Layout, key bindings, and commands</strong></summary>

**Layout**

- **Messages pane** — Markdown-rendered conversation history with collapsible tool-execution groups and reasoning blocks
- **Input editor** — Multi-line input with slash-command completion, `@` file citation, and `!` shell mode
- **Footer** — Model name, context usage bar, session duration, Git branch, working directory, sandbox status
- **Sidebars** — File explorer, keyboard shortcut reference, and skill browser

**Key bindings**

| Key | Action |
|-----|--------|
| `Enter` | Submit prompt |
| `Ctrl+T` | Cycle themes (12 built-in) |
| `Shift+B` | Toggle file explorer |
| `Alt+S` | Toggle skill browser |
| `Ctrl+]` / `Ctrl+[` | Navigate sub-agent sessions |
| `Tab` / `Shift+Tab` | Cycle agent sessions |

**Slash commands** include `/compact`, `/context`, `/hooks`, `/language`, `/rewind`. Conversation export to Markdown or JSON includes full session metadata, a config snapshot, a context report, and statistics.

</details>

## Documentation

| Document | Covers |
|----------|--------|
| [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) | Architecture and ownership |
| [docs/AGENT_ARCHITECTURES.md](docs/AGENT_ARCHITECTURES.md) | Agent architecture runtime and manifests |
| [docs/EMBEDDING.md](docs/EMBEDDING.md) | Embedding Amadeus in Rust applications |
| [docs/HTTP_API.md](docs/HTTP_API.md) | Full HTTP contract |
| [docs/MEMORY.md](docs/MEMORY.md) | Tiered memory system and mid-term interfaces |
| [docs/RAG.md](docs/RAG.md) | Embedding backends and vector store |
| [docs/TOOLS.md](docs/TOOLS.md) | Tool system reference |
| [docs/COMPACTION.md](docs/COMPACTION.md) | Context compaction |
| [docs/MACOS_APP.md](docs/MACOS_APP.md) | Native macOS client |
| [docs/WEB_DESIGN_SYSTEM.md](docs/WEB_DESIGN_SYSTEM.md) | Web design contract |
| [DEVELOPMENT.md](DEVELOPMENT.md) | Development workflow |
| [.amadeus/README.md](.amadeus/README.md) | Configuration reference |
| [docs/TUI_TESTING.md](docs/TUI_TESTING.md) | TUI testing |

## Contributing

Contributions are welcome — [CONTRIBUTING.md](CONTRIBUTING.md) has the workflow and repository standards. PRs should pass `./verify.sh`, the CI-parity gate.

## License

MIT — see [LICENSE](LICENSE).
