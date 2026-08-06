# Coding Style Guide

This document outlines the coding style and conventions used in this project, which is primarily written in Rust (edition 2021) and organized as a Cargo workspace of independent crates (`amadeus-*`) coordinated by a compatibility facade at the repository root.

It is the authoritative, project-specific elaboration of the rules summarized in [`AGENTS.md`](./AGENTS.md). Where the two disagree, this file is more specific and wins on matters of *form*; [`AGENTS.md`](./AGENTS.md) remains authoritative on *behavior*.

## Table of Contents

1. [General Principles](#general-principles)
2. [File Organization](#file-organization)
3. [Naming Conventions](#naming-conventions)
4. [Formatting](#formatting)
5. [Async and Concurrency](#async-and-concurrency)
6. [Comments and Documentation](#comments-and-documentation)
7. [Source-File Headers](#source-file-headers)
8. [Generics and Traits](#generics-and-traits)
9. [Error Handling](#error-handling)
10. [Feature Flags](#feature-flags)
11. [Testing](#testing)
12. [Tools](#tools)

## General Principles

- Follow modern Rust (edition 2021) idioms; prefer the standard library and well-maintained ecosystem crates (`tokio`, `serde`, `thiserror`, `axum`, `ratatui`, `reqwest`).
- Prioritize correctness and readability first, performance second — but never allocate in a hot path when a borrow or `Arc` clone will do.
- Treat the agent loop, the `LLMClient` trait, and the `Tool` trait as the load-bearing interfaces of the SDK. Changes to their signatures ripple across every provider and tool; see [Generics and Traits](#generics-and-traits).
- Maintain consistency with the surrounding code: when in doubt, match the nearest neighbor in the same module rather than importing a new style.
- **No comments in code unless explicitly requested by the user.** Rationale belongs in doc comments or commit messages, not inline narration.

## File Organization

- The repository is a Cargo workspace. Crates live under `crates/<name>/` and are named `amadeus_<name>` on disk (e.g. `crates/core/` → `amadeus-core`). The root crate `amadeus` is a compatibility facade (`src/lib.rs`).
- Group related functionality into submodules by concern: `agent/`, `client/`, `tools/`, `policy/`, `context/`, `telemetry/`, `compaction/`, `benchmark/`.
- Prefer many small files over one large file. A module that grows past ~600 lines is a candidate for splitting into a subdirectory with `mod.rs` (or a named file) re-exporting the public surface.
- One public type or one cohesive family of functions per file is the sweet spot.
- Tests live either inline (`#[cfg(test)] mod tests`) for unit tests, or in the `tests/` directory for integration tests. Name integration test files by *behavior*, not by implementation: `tool_approval_test.rs`, `stress_memory_test.rs`, `compaction_test.rs`.

## Naming Conventions

### Modules, files, and functions

- Use `snake_case` for module names, file names, function names, methods, and local variables.
- Examples:
  ```rust
  // crates/core/src/tools/tool_trait.rs
  pub trait Tool: Send + Sync {
      fn name(&self) -> &'static str;
      fn schema(&self) -> &'static Value;
      async fn execute(&self, input: Value) -> Result<String>;
  }

  // crates/core/src/agent/loop_agent.rs
  async fn run_turn(&self, input: Value) -> Result<AgentEvent> { ... }
  ```

### Types and traits

- Use `PascalCase` for structs, enums, traits, and type aliases.
- Generic type parameters carry a meaningful suffix: `C` for an `LLMClient` (`Agent<C>`), `T` for a generic value. Avoid single-letter names outside of trivial generic contexts.
- Examples: `Agent`, `AnthropicClient`, `OpenAIClient`, `LLMClient`, `Tool`, `Policy`, `AgentError`, `ContentBlock`, `BenchmarkRunSummary`.

### Constants

- Use `SCREAMING_SNAKE_CASE` for `const` and `static` items.
- Examples: `MAX_TOOL_RETRIES`, `DEFAULT_MODEL_ID`, `STREAM_CHUNK_TIMEOUT`.

### Variants

- Enum variants are `PascalCase` (`AgentError::ToolNotFound`, `RunStatus::Passed`). When an enum is serialized with `serde`, annotate it with `#[serde(rename_all = "snake_case")]` so the wire format stays `snake_case`:
  ```rust
  #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
  #[serde(rename_all = "snake_case")]
  pub enum RunStatus {
      Passed,
      Failed,
      Error,
  }
  ```

## Formatting

Amadeus relies on `cargo fmt` defaults — there is intentionally **no `rustfmt.toml`**. Do not introduce one. The rules below are what `cargo fmt` produces on this codebase.

### Indentation

- Use **4 spaces** for indentation — the `rustfmt` default, which is what the codebase is formatted with. No tabs.
- Continuation lines are aligned by `cargo fmt`; do not hand-align wrapped calls differently.

### Braces

- K&R style: opening brace on the same line.
- Always brace control-flow and match arms that contain more than a single trivial expression.

### Imports

- Group `use` statements in three blocks separated by blank lines, in this order:
  1. `std` and explicit allocator/core items.
  2. Third-party crates (`serde`, `tokio`, `thiserror`, …).
  3. `crate::` modules and `super::` items.
- Within each block, keep the order stable; `cargo fmt` sorts lexicographically when lines are disjoint.
- Example (from `crates/core/src/benchmark/runner.rs`):
  ```rust
  use std::fs;
  use std::path::{Path, PathBuf};
  use std::sync::Arc;
  use std::time::Instant;

  use chrono::Utc;
  use futures::StreamExt;

  use crate::agent::config::{Config, Provider};
  use crate::agent::events::AgentEvent;
  use crate::error::{AgentError, Result};
  ```

### Line length and wrapping

- `cargo fmt` wraps at 100 characters. Do not reflow by hand.
- Break long function signatures with one argument per line when they exceed the limit; keep the closing paren and return type on their own line as `cargo fmt` does.

### Spacing

- One space around binary operators (`a + b`, `x == y`).
- No space before the semicolon, no space between a unary operator and its operand (`-x`, `&self`).
- A space after keywords (`if `, `for `, `match `) and after the colon in a label or turbofish is dictated by `cargo fmt` — do not fight it.

## Async and Concurrency

- The project uses the `tokio` runtime throughout. Never call `.unwrap()` on a `Future` in production paths; drive async code to completion with `.await`.
- Execute independent tools in parallel with `futures::future::join_all` rather than sequential `.await`s when ordering does not matter.
- Prefer interior mutability with `Arc<RwLock<T>>` for read-heavy shared state and `Arc<Mutex<T>>` for write-heavy or short critical sections. Use `tokio::sync::{Mutex, RwLock}` for state held across `.await` points, and `std::sync::RwLock` for tiny, synchronous snapshots (e.g. `Policy` clones).
- Minimize heap allocations in the agent loop. Share ownership via `Arc<T>` rather than cloning large structures; pass `&T` when the callee does not need ownership.
- Example (canonical sharing pattern from `loop_agent.rs`):
  ```rust
  history: Option<Arc<RwLock<Vec<Message>>>>,
  policy: Arc<RwLock<Policy>>,
  subagent_coordinator: Arc<SubAgentCoordinator>,
  ```

## Comments and Documentation

### Doc comments are required

- **Every public function, trait, struct, enum, and module needs a doc comment** (`///` for items, `//!` for module-level docs at the top of the file).
- Private items are encouraged but not required to carry doc comments.
- Document Args, Returns, panics, and any non-obvious complexity (e.g. allocation behavior, lock ordering, retry semantics).
- Module-level docs (`//!`) sit directly under the source-file header block and explain the module's purpose, as in `crates/core/src/error.rs`.

### Inline comments

- **No inline comments unless explicitly requested.** Code should communicate intent through naming and structure. When a comment is genuinely necessary (a non-obvious algorithm, a performance workaround, a safety invariant), place it *above* the line it describes and keep it short.
- Prefer `//` for any comment. Use `/* */` only inside expressions where `//` would be ambiguous.
- Never leave commented-out code in committed files.

## Source-File Headers

Every source (`.rs`) file in scope begins with an `@amadeus-header` comment block. This is **mandatory** and enforced by the rules in [`docs/SOURCE_FILE_HEADERS.md`](./docs/SOURCE_FILE_HEADERS.md). Touching an in-scope file means keeping its header accurate in the same change.

Canonical shape (from `crates/core/src/error.rs`):

```rust
// @amadeus-header
// summary: Shared agent error types and conversions from config and runtime failures.
// layer: infra
// status: active
// feature_flags: none
// provides:
// - module: crate::error
// - type: crate::error::AgentError
// - type: crate::error::Result
// uses: none
// invariants:
// - Listed interfaces stay aligned with the implementation in this file.
// side_effects: none
// tests:
// - tests/error_recovery_test.rs
// @end-amadeus-header
```

Rules:
- Keep `provides:` and `uses:` synchronized with the actual public items and dependencies in the file. A stale header is a policy violation.
- Set `feature_flags:` to the gating feature (e.g. `tui`, `orchestra`) or `none`.
- Set `status:` to `active` for maintained files; use `deprecated` only during a documented removal window.
- When you add, rename, or remove a public item, update `provides:` in the same commit.

## Generics and Traits

### Generic parameters

- Prefer **generic traits** (`Agent<C: LLMClient>`) over dynamic dispatch (`Box<dyn LLMClient>`) on hot paths. Monomorphization removes a vcall and lets the compiler inline provider-specific code.
- Constrain generics with trait bounds at the declaration site; do not defer bounds to `impl` blocks in a way that splits the contract across files.
- Order type parameters: provider/client traits first, then value types, then integer generics (mirrors the C++ guide's "types, then integers, then booleans").

### Trait design

- The three load-bearing traits are `LLMClient` (provider abstraction), `Tool` (capability abstraction), and the policy/permission checks layered around them. Keep their method signatures minimal and stable.
- `async fn` in traits is used directly (edition 2021 + current toolchain). Do not reach for the `async-trait` macro unless a downstream constraint forces it.
- Every trait that crosses an `await` boundary or is stored in an `Arc` must be `Send + Sync`.

## Error Handling

- Use `crate::error::Result<T>` (a `Result` alias backed by `AgentError`) as the return type for fallible operations. `AgentError` is derived with `thiserror`.
- **Never use `unwrap()` or `expect()` in production code.** `clippy.toml` (`allow-unwrap-in-tests = true`, `allow-expect-in-tests = true`) permits them only inside `#[cfg(test)]`.
- Convert foreign errors with `#[from]` on enum variants so `?` works end-to-end:
  ```rust
  #[derive(Debug, Error)]
  pub enum AgentError {
      #[error("API request failed: {0}")]
      ApiRequest(#[from] reqwest::Error),

      #[error("IO error: {0}")]
      Io(#[from] std::io::Error),

      #[error("Serde error: {0}")]
      Serde(#[from] serde_json::Error),
  }
  ```
- Use `Result::map_err` with a concrete `AgentError` variant when `#[from]` is not available, and keep error messages actionable (name the failing operation and the offending value).
- Handle edge cases explicitly and early (early-return on degenerate input) rather than letting them propagate as opaque `unwrap` panics.

## Feature Flags

The crate ships with **no default features**; everything is opt-in via `--features ...`. The umbrella `full` flag enables everything and is what you should build and test with day-to-day.

Feature graph:
- `full` → everything (`api`, `tui`, `concurrency`, `orchestra`, `context`, `test-utils`).
- `api` → `orchestra` → `concurrency`.
- `tui` → `concurrency`.
- `context`, `test-utils` are standalone.
- `orchestra` is the only multi-agent flag. `team`, `supervisor`, and `mesh` do **not** exist as features — do not reintroduce them as aliases.

Conventions:
- Gate optional code with `#[cfg(feature = "...")]` and reflect the gating feature in the file's `feature_flags:` header.
- New optional capabilities go behind a feature flag from day one; do not leave code ungated and retrofit a flag later.
- When adding a feature, update the Feature Flags table in [`AGENTS.md`](./AGENTS.md) in the same change.

## Testing

- **Mock-first.** Deterministic tests use `tests/mock_llm.rs` and HTTP mocks (`mockito` / `wiremock`). Do not hit real providers from unit tests.
- Unit tests are inline (`#[cfg(test)] mod tests`) and exercise private items. Integration tests live in `tests/` and exercise the public API.
- Integration test suites assume `--features full` unless they declare a `[[test]] required-features` entry in `Cargo.toml`. Prefer the `--features full` convention for new suites.
- Name test functions by the behavior they verify: `rejects_dangerous_command_under_strict_policy`, not `test_policy_1`.
- Keep tests hermetic: no shared mutable files on disk, no reliance on wall-clock ordering.

## Tools

### Formatting

- Run `cargo fmt --all` before every commit. There is no configuration file — the defaults *are* the style.
- Never hand-format in a way that contradicts `cargo fmt`; CI will reformat it and produce noise.

### Linting

- Run `cargo clippy --all-features -- -D warnings`. Warnings are errors; fix them in the change that introduces them.
- `clippy.toml` relaxes `unwrap`/`expect` rules *only* inside test code — do not widen this allowance.

### Building and testing

- `cargo build --features full` / `cargo build --release --features full`.
- `cargo test --features full` (append `-- --nocapture` to see `println!` output).
- `./verify.sh` runs the full pipeline in order: source-file headers → `cargo fmt --all -- --check` → `cargo metadata` → `cargo clippy --all-features -- -D warnings` → feature-matrix `cargo check` (`--no-default-features`, `tui`, `api`, `full`) → `cargo test --features full`. Run it before opening a PR.
- Iterate on one crate with `-p` (`cargo test -p core`, `cargo check -p tui`) — package names are bare (`core`, `tui`, `runtime`), not `amadeus_*`.

### Web client (`apps/web`)

The React/Tauri client is a separate npm project and is not covered by `cargo fmt`/`clippy`:

- Lint with `npm run lint` (eslint + react/react-hooks plugins); test with `npm run test` (`node --test`, no extra framework).
- Keep the `@amadeus-header` block at the top of `.js`/`.jsx` modules, same fields as the Rust files.
- User-facing strings go through `translate()`/`t()` from `src/i18n.js`; English source text is the key. The TUI has its own catalog (`crates/tui/src/ui/i18n.rs`) — adding a string to one does not cover the other.

### Recommended workflow

1. Write or modify code, keeping file headers and `provides:`/`uses:` accurate.
2. `cargo fmt --all`.
3. `cargo clippy --all-features -- -D warnings`.
4. `cargo test --features full` (or the targeted `cargo test <name> --features full`).
5. `./verify.sh` as a final gate.

This style guide applies to all new contributions. For existing code that does not conform, prefer to align it with these rules during the next change that touches the same region, rather than mass-formatting in an unrelated commit.
