# GEMINI.md - Project Context & Ruleset

## Project Overview
**Amadeus** is a high-performance AI coding agent implemented in Rust. It follows the "Bash is All You Need" philosophy, providing a minimalist yet powerful toolset for autonomous software engineering tasks. The system is built on a modular, async architecture using **Tokio**, with support for multiple LLM providers (Anthropic, OpenAI) and a modern **Ratatui**-based TUI.

### Repository Shape
Amadeus is a **Cargo workspace**. The root `amadeus` crate is a thin compatibility facade (`src/lib.rs`) that re-exports `amadeus_core` and, when enabled, the `api`/`tui` adapters. Implementation lives under `crates/`; library callers still import through `amadeus::...`. `apps/web` (React + Tauri), `python-sdk/`, `benchmarks/`, and `runtime/` are outside the Cargo workspace.

### Core Architecture
- **Agent Loop (`crates/core/src/agent/`)**: Implements the ReAct (Reason + Act) pattern with support for both streaming and non-streaming modes.
- **LLM Clients (`crates/core/src/client/`)**: Provider-agnostic abstractions for Anthropic and OpenAI APIs; `Agent<C>` is generic over the provider.
- **Tool Registry (`crates/core/src/tools/`)**: Extensible tool system featuring a high-performance `bash` executor and surgical file manipulation tools (`read_file`, `write_file`, `edit_file`), composed into profiles/packs.
- **TUI Layer (`crates/tui/src/ui/`)**: An inline terminal interface providing real-time feedback, multiline input, and structured tool execution panels.
- **HTTP Server (`crates/api/src/api/`)**: An Axum-based API layer for remote agent interaction, consumed by `apps/web`.
- **Leaf crates**: `runtime` (orchestration models), `config`, `commands`, `permissions`, `messages`, `events`, `prompts`, `profiles`, `skills`, `hooks`, `telemetry`, `compaction`, `context`, `rag`, `ids`.

---

## Building and Running
The crate has **no default features** — every command needs an explicit feature set, and the `amadeus` binary declares `required-features = ["full"]`. Runtime settings are layered: `~/.amadeus/settings.json` → `.amadeus/settings.json` → `.amadeus/settings.local.json`.

| Task | Command |
| :--- | :--- |
| **Build (Debug)** | `cargo build --features full` |
| **Build (Release)** | `cargo build --release --features full` |
| **Run (Interactive TUI)** | `cargo run --features full` |
| **Run (HTTP Server)** | `cargo run --features full -- --server [PORT]` |
| **Test (All)** | `cargo test --features full` |
| **Test (Specific)** | `cargo test --test <test_name> --features full` |
| **Single crate** | `cargo check -p tui` / `cargo test -p core` |
| **Lint** | `cargo clippy --all-features -- -D warnings` |
| **Full verification** | `./verify.sh` |
| **Clean generated output** | `make clean` |

There is no positional prompt argument. Supported flags: `--server [PORT]`, `--record [DIR]`, `--export PATH`, `--assess-features [DIR]`, `--permission-mode MODE`, `--llm-trace [DIR]`, `--help`.

### Web / Desktop Client
`cd apps/web && npm install`, then `npm run dev` (Vite, expects the server on `http://127.0.0.1:3000`; override with `VITE_AMADEUS_API_URL`). `npm run test` runs `node --test src/*.test.js`, `npm run lint` runs eslint, `npm run mock-api` serves a mock backend, `npm run desktop:dev` builds the Tauri shell.

---

## Development Conventions

### 1. Formatting and Naming
*   **Naming**: 
    *   `snake_case`: Variables, functions, and modules.
    *   `PascalCase`: Structs, Enums, and Traits.
    *   `SCREAMING_SNAKE_CASE`: Constants and Statics.
*   **Indentation**: 4 spaces — the `rustfmt` default. There is intentionally no `rustfmt.toml`; run `cargo fmt --all` and never hand-format against it.
*   **Imports**: Grouped as: `std` → `third-party crates` → `crate modules`, separated by blank lines.
*   **Documentation**: Every public item needs a doc comment explaining *Args*, *Returns*, and non-obvious complexity. Module docs use `//!` directly below the file header.
*   **Source-file headers**: Every in-scope file starts with an `@amadeus-header` block per `docs/SOURCE_FILE_HEADERS.md`; keep `provides:`/`uses:` accurate in the same change and validate with `python scripts/check_source_headers.py`.
*   `CODING_STYLE.md` is the long-form elaboration of these rules.

### 2. Performance & Efficiency Mandates
*   **Memory Management**: Minimize heap allocations. Use `Arc<T>` for shared ownership and `RwLock<T>` for thread-safe interior mutability.
*   **Async Patterns**: Leverage `tokio` for non-blocking I/O. Use `join_all` for parallel tool execution.
*   **Zero-Cost Abstractions**: Prefer generic traits (e.g., `LLMClient`) to dynamic dispatch where performance is critical.
*   **OPTIMIZATION**: If an optimization (like SIMD or bit-twiddling) reduces readability, it must be preceded by an `// OPTIMIZATION:` comment.

### 3. Defensive Engineering
*   **Error Handling**: Use `crate::error::Result<T>` (aliased `thiserror`). Never use `unwrap()`/`expect()` in production code — `clippy.toml` permits them only under `#[cfg(test)]`.
*   **Path Safety**: All file tools must validate that paths do not escape the workspace directory.
*   **Command Security**: Blocked commands (e.g., `rm -rf /`) are enforced via the `Config` layer; the `PermissionMode` enum (`read-only` | `workspace-write` | `danger-full-access` | `prompt`) is enforced by `PermissionEnforcer` and is the **default and always-on** gate. The separate `Policy`/`ApprovalMode` layer (`core/src/policy/mod.rs`) is **opt-in** — it only applies when an agent is built with `with_policy(...)`, and is not wired into the agent loop or `/execute` by default (see `docs/plans/2026-08-05-structure-improvement-plan.md` F2c).

### 4. Localization
*   Two independent catalogs, English (default) + Simplified Chinese: `crates/tui/src/ui/i18n.rs` (`i18n::text("key")`) and `apps/web/src/i18n.js` (`translate`/`t`). Adding a string to one does not cover the other; English is the fallback.
*   Language is selected by `tui.language` in any settings layer or `/language en|zh-CN` at runtime.

---

## Tool-Specific Guidelines

### Bash Tool
- **Timeout**: Enforced via `tokio::time::timeout`.
- **Output Management**: Truncated based on `MAX_OUTPUT_BYTES` to prevent context window overflow.

### File Tools
- **read_file**: Preferred over `cat` for large files to ensure structured handling.
- **write_file**: Atomic operations preferred to prevent data corruption.
- **edit_file**: Uses surgical string replacement or `sed`-like logic to minimize changes.

---

## Agent Instructions
When operating within this codebase:
1.  **No Comments**: Do not add comments to code unless explicitly requested.
2.  **Surgical Changes**: Use the provided file tools to make precise modifications.
3.  **Validate**: Always run `cargo check --features full` and relevant tests after making changes; `./verify.sh` before opening a PR.
4.  **Security**: Never expose API keys or secrets in logs or git history.
5.  **Headers**: Keep `@amadeus-header` blocks in sync with the code you change — a stale header is a policy violation.

---

## Synced Instruction Files
`AGENTS.md`, `CLAUDE.md`, and this file describe the same codebase for different tools and are meant to stay in sync. When you change one, check whether the others need the same change. `CODING_STYLE.md` wins on matters of form; `AGENTS.md` wins on behavior.
