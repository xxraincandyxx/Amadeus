# GEMINI.md - Project Context & Ruleset

## Project Overview
**Amadeus** is a high-performance AI coding agent implemented in Rust. It follows the "Bash is All You Need" philosophy, providing a minimalist yet powerful toolset for autonomous software engineering tasks. The system is built on a modular, async architecture using **Tokio**, with support for multiple LLM providers (Anthropic, OpenAI) and a modern **Ratatui**-based TUI.

### Core Architecture
- **Agent Loop (`src/agent/`)**: Implements the ReAct (Reason + Act) pattern with support for both streaming and non-streaming modes.
- **LLM Clients (`src/client/`)**: Provider-agnostic abstractions for Anthropic and OpenAI APIs.
- **Tool Registry (`src/tools/`)**: Extensible tool system featuring a high-performance `bash` executor and surgical file manipulation tools (`read_file`, `write_file`, `edit_file`).
- **TUI Layer (`src/ui/`)**: A Dracula-themed terminal interface providing real-time feedback, multiline input, and structured tool execution panels.
- **HTTP Server (`src/api/`)**: An Axum-based API layer for remote agent interaction.

---

## Building and Running
The project uses standard Cargo workflows. Runtime settings are loaded from `.amadeus/settings.json` with optional global defaults in `~/.amadeus/settings.json`.

| Task | Command |
| :--- | :--- |
| **Build (Debug)** | `cargo build` |
| **Build (Release)** | `cargo build --release` |
| **Run (Interactive TUI)** | `cargo run` |
| **Run (Single Task)** | `cargo run -- "task description"` |
| **Run (HTTP Server)** | `cargo run -- --server` |
| **Test (All)** | `cargo test` |
| **Test (Specific)** | `cargo test --test <test_name>` |
| **Lint** | `cargo clippy` |

---

## Development Conventions

### 1. Strict Google Style Guide (Rust Adaptation)
*   **Naming**: 
    *   `snake_case`: Variables, functions, and modules.
    *   `PascalCase`: Structs, Enums, and Traits.
    *   `SCREAMING_SNAKE_CASE`: Constants and Statics.
*   **Indentation**: 2-space indentation (Google standard).
*   **Imports**: Grouped as: `std` → `third-party crates` → `crate modules`, separated by blank lines.
*   **Documentation**: Every function must include a docstring explaining *Args*, *Returns*, and *Time Complexity*.

### 2. Performance & Efficiency Mandates
*   **Memory Management**: Minimize heap allocations. Use `Arc<T>` for shared ownership and `RwLock<T>` for thread-safe interior mutability.
*   **Async Patterns**: Leverage `tokio` for non-blocking I/O. Use `join_all` for parallel tool execution.
*   **Zero-Cost Abstractions**: Prefer generic traits (e.g., `LLMClient`) to dynamic dispatch where performance is critical.
*   **OPTIMIZATION**: If an optimization (like SIMD or bit-twiddling) reduces readability, it must be preceded by an `// OPTIMIZATION:` comment.

### 3. Defensive Engineering
*   **Error Handling**: Use `crate::error::Result<T>` (aliased `thiserror`). Never use `unwrap()` in production code.
*   **Path Safety**: All file tools must validate that paths do not escape the workspace directory.
*   **Command Security**: Blocked commands (e.g., `rm -rf /`) are enforced via the `Config` layer.

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
3.  **Validate**: Always run `cargo check` and relevant tests after making changes.
4.  **Security**: Never expose API keys or secrets in logs or git history.
