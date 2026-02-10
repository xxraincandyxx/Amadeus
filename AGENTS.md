# AGENTS.md

This repository contains a Rust-based AI coding agent implementation supporting both Anthropic and OpenAI APIs. This document guides agentic coding agents working in this codebase.

## Build / Lint / Test Commands

### Install Dependencies
```bash
cargo install --locked
```

### Configuration
```bash
cp .env.example .env
# Edit .env with your ANTHROPIC_API_KEY or OPENAI_API_KEY
```

### Build
```bash
cargo build
```

### Run Tests
```bash
# Run all tests
cargo test

# Run tests with output
cargo test -- --nocapture

# Run a specific test file
cargo test --test bash_test

# Run a specific test function
cargo test test_bash_basic

# Run tests matching a pattern
cargo test test_bash

# Run integration tests specifically
cargo test --test openai_test
```

### Lint
```bash
# Check code without building
cargo check

# Check with clippy (if available)
cargo clippy
```

### Run the Agent
```bash
# Interactive mode (default: Anthropic)
cargo run

# One-shot mode with prompt
cargo run -- "your prompt here"

# Use OpenAI provider
PROVIDER=openai cargo run

# Enable streaming
USE_STREAMING=true cargo run
```

## Code Style Guidelines

### Imports
- Group imports: std library first, then third-party crates
- Use `use crate::*` for internal modules
- Keep imports at module level, not within functions

### Naming Conventions
- **Functions/Variables**: `snake_case` (e.g., `execute_tool`, `workdir`, `api_key`)
- **Types/Structs/Enums**: `PascalCase` (e.g., `AgentError`, `BashTool`, `Config`)
- **Constants**: `SCREAMING_SNAKE_CASE` (rare, prefer struct fields)
- **Modules**: `snake_case` (e.g., `mod agent`, `mod client`)
- **Traits**: `PascalCase` (e.g., `LLMClient`)

### File Organization
- `src/lib.rs`: Public exports, module declarations
- `src/main.rs`: CLI entry point only
- `src/error.rs`: All custom error types
- Module structure: `src/<domain>/mod.rs` with related files
- `tests/`: Integration tests, organized by component

### Types
- Use `Result<T>` from `crate::error` instead of `std::result::Result`
- Prefer `String` over `&str` for struct fields
- Use `Arc<T>` for shared ownership, `RwLock<T>` for concurrent state

### Error Handling
- All errors use `AgentError` enum in `src/error.rs`
- Wrap errors with `?` operator for propagation
- Use `#[from]` attribute for automatic conversion from standard errors
- Return descriptive error messages with context
- Use `anyhow::Result<T>` in main.rs for top-level convenience

### Async Patterns
- All async functions use `tokio::runtime`
- Use `.await` on async calls without blocking
- Prefer `await` propagation over blocking waits
- Use `tokio::process::Command` for subprocess execution
- Handle timeouts with `tokio::time::timeout`
- Use `futures::StreamExt` for streaming operations

### Traits and Abstractions
- `LLMClient` trait in `src/client/mod.rs` for provider abstraction
- Implementations: `AnthropicClient`, `OpenAIClient`
- Generic agent type: `Agent<C: LLMClient>`
- Use `async-trait` for async trait methods

### Tool Implementation
- Tools are structs with `execute` methods taking `ToolInput`
- Return `Result<String>` with output or error, use timeout wrapper
- BashTool uses `sh -c` for command execution
- Tool schemas defined in `tools/schema.rs`

### Agent Loop Pattern
```rust
loop {
    let response = client.create_message(&system, &history, &tools, max_tokens).await?;
    if stop_reason != "tool_use" {
        return Ok(text_content);
    }
    // Execute tools, collect results, append to history
    history.push(Message::user(tool_results));
}
```

### Configuration
- Load from environment variables via `dotenvy`
- `Config::load()` returns error for missing required env vars
- Supports Anthropic and OpenAI providers
- Optional: ANTHROPIC_BASE_URL/OPENAI_BASE_URL, MODEL_ID, USE_STREAMING
- Default timeout: 300 seconds

### System Prompts & Visibility
- System prompts in `Config::system_prompt()` with workdir interpolation
- Keep concise with rules and subagent guidance
- Types explicitly marked `pub` for export at `lib.rs` level

### Testing
- Unit tests in `src/` alongside implementation, integration tests in `tests/`
- Test functions named `test_<subject>_<scenario>`
- Use `assert!`, `assert_eq!` for assertions

### Concurrency
- Use `tokio::sync::RwLock` for shared mutable state
- Clone `Arc` for passing shared references
- Use `futures::future::join_all` for concurrent execution
- Avoid blocking operations in async contexts

## Key Design Principles
- Strong type safety with `Result<T>` error handling
- Async-first with tokio runtime
- Provider abstraction via traits
- Single bash tool covers all file operations
- Streaming support for faster output
- Subagents for task isolation and context cleanup
- Dracula-themed UI with colored output
