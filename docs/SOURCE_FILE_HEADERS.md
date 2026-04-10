# Source File Headers

This document defines the mandatory header format for hand-maintained source files in the Amadeus repository.

The goal is to make each file answer the same questions quickly:
- What is this file responsible for?
- What interfaces does it provide to the rest of the system?
- What interfaces does it depend on?
- What invariants and side effects matter when changing it?

## Scope

The header is required for hand-maintained source files in repository-owned areas, including:
- `src/**/*.rs`
- `tests/**/*.rs`
- `examples/**/*.rs`
- `scripts/**/*.sh`
- top-level executable or checked-in source scripts such as `verify.sh`

The header is not required for:
- vendored or mirrored code under `refs/`
- generated files
- lockfiles
- fixture data and snapshots
- non-source configuration and documentation files

If there is doubt about whether a file is in scope, treat it as in scope unless it is clearly generated or external.

## Canonical Format

Use a machine-findable block bounded by `@amadeus-header` and `@end-amadeus-header`.

Keep the header at the top of the file:
- For scripts with a shebang, keep the shebang on line 1 and place the header immediately after it.
- For Rust files, place the header before imports and before the first item.

Use the native single-line comment marker for the language:
- Rust: `//`
- Shell: `#`
- Python: `#`

## Schema

Every in-scope file header must contain the following fields.

| Field | Required | Format | Purpose |
| --- | --- | --- | --- |
| `summary` | yes | one sentence | Primary responsibility of the file |
| `layer` | yes | enum | Architectural area: `core`, `agent`, `client`, `tools`, `policy`, `ui`, `api`, `benchmark`, `test`, `example`, `script`, `infra` |
| `status` | yes | enum | `active`, `experimental`, `deprecated`, `test-only`, `generated` |
| `feature_flags` | yes | list or `none` | Relevant Cargo features or runtime gates |
| `provides` | yes | list or `none` | External interfaces defined or exposed by this file |
| `uses` | yes | list or `none` | External interfaces consumed by this file |
| `invariants` | yes | list or `none` | Non-obvious truths that must remain true after edits |
| `side_effects` | yes | list or `none` | Observable effects outside local pure computation |
| `tests` | yes | list or `none` | Primary verification targets for the file |

## Interface Rules

`provides` and `uses` are the core of the schema and must be complete.

An interface belongs in one of these kinds:
- `module`
- `type`
- `trait`
- `fn`
- `const`
- `tool`
- `route`
- `event`
- `cmd`
- `format`
- `artifact`
- `env`
- `protocol`
- `runtime`

Use repo-qualified or system-qualified names where practical.

Examples:
- `type: crate::agent::loop_agent::Agent<C>`
- `trait: crate::client::LLMClient`
- `tool: sub_agent`
- `route: POST /v1/chat`
- `artifact: session log JSON`
- `env: OPENAI_API_KEY`
- `cmd: cargo test --features full`
- `runtime: tokio tasks and channels`

## What Counts As An External Interface

Include interfaces that cross a file, module, package, process, or persistence boundary, such as:
- public or cross-module Rust types, traits, functions, constants, and modules
- implemented or consumed trait contracts
- tool names, route shapes, event types, message formats, and serialized artifacts
- environment variables, feature flags, file formats, CLI commands, and network endpoints
- files written to or read from when they are part of a stable workflow

Do not include:
- every `std` type or trivial crate import
- local helper functions and private structs that do not form a boundary contract
- implementation detail that can change freely without affecting other files
- historical notes, authorship, dates, issue links, or changelog text

## Required Style

- Keep `summary` under 24 words.
- Keep lists short and high-signal.
- Prefer stable nouns over prose fragments.
- Sort `provides` and `uses` by kind, then by name.
- Use `none` instead of an empty list.
- Do not include placeholders such as `TBD`, `TODO`, `etc`, or `misc`.
- Do not claim an interface unless it is actually present in the file or materially consumed by it.

## Rust Example

```rust
// @amadeus-header
// summary: ReAct loop for LLM turns, tool execution, approvals, and session logging.
// layer: agent
// status: active
// feature_flags:
// - full
// provides:
// - type: crate::agent::loop_agent::Agent<C>
// - type: crate::agent::loop_agent::AgentBuilder<C>
// - type: crate::agent::loop_agent::SessionLog
// - event: crate::agent::events::AgentEvent
// - artifact: session log JSON
// uses:
// - trait: crate::client::LLMClient
// - type: crate::agent::config::Config
// - type: crate::policy::Policy
// - type: crate::tools::registry::ToolRegistry
// - runtime: tokio tasks and channels
// invariants:
// - Policy approval is resolved before any tool side effect.
// - Tool-use and tool-result history stay order-aligned.
// side_effects:
// - Writes session logs to disk.
// - Sends agent events across channels.
// - Spawns async tasks.
// tests:
// - tests/agent_integration_test.rs
// - tests/tool_approval_test.rs
// @end-amadeus-header
```

## Script Example

```bash
#!/bin/bash
# @amadeus-header
# summary: Repository verification runner for formatting, linting, feature checks, and tests.
# layer: script
# status: active
# feature_flags:
# - full
# provides:
# - cmd: verify.sh
# uses:
# - cmd: cargo fmt --all -- --check
# - cmd: cargo clippy --all-features -- -D warnings
# - cmd: cargo check --features full
# - cmd: cargo test --features full
# invariants:
# - Verification fails fast on the first failing command.
# side_effects:
# - Runs build and test commands.
# tests:
# - cmd: ./verify.sh
# @end-amadeus-header
```

## Maintenance Guide

This section is the strict operating policy for humans and coding agents.

### Mandatory Rules

1. Every new in-scope source file must include a valid header before merge.
2. Every touched in-scope source file must have its header reviewed and updated in the same change if any header-relevant fact changed.
3. `provides` and `uses` must be maintained as strictly as function signatures and tests.
4. A stale header is a correctness bug, not a documentation nit.
5. If a contributor cannot state the current interfaces confidently, they must inspect the file and adjacent callers before editing the header.
6. Headers must describe the file as it exists after the change, not as it existed before or may exist later.
7. Headers must not be used as a narrative changelog.
8. Headers must not contain speculation, ownership politics, or AI provenance.

### Header-Relevant Changes

Update the header whenever a change affects any of the following:
- public or cross-module types, traits, functions, constants, or modules
- implemented or consumed trait contracts
- tool names, routes, events, protocols, formats, or persistent artifacts
- feature gates or runtime gating behavior
- environment variables, CLI entrypoints, or external commands
- filesystem, process, network, or concurrency side effects
- invariants that future maintainers need in order to edit safely
- primary tests or verification commands that define the expected behavior

### Review Standard

A header is acceptable only if all of the following are true:
- `summary` states the file’s actual job and not a vague category
- `provides` includes every meaningful boundary the file exposes
- `uses` includes every meaningful boundary the file relies on
- `invariants` capture the non-obvious conditions that make the file safe
- `side_effects` capture all meaningful external effects
- `tests` point to the best local verification surface
- nothing material is missing, duplicated, stale, or hand-wavy

### Merge Blockers

The following should block merge:
- missing header in an in-scope file
- header markers present but required fields missing
- `provides` or `uses` known to be incomplete
- header contradicts the implementation
- renamed interface in code but old name left in the header
- side effects changed but `side_effects` not updated
- file split or merged without rewriting headers to match the new boundaries

### Editing Workflow

When editing an in-scope file, perform this check before finishing:
1. Read the existing header.
2. Compare it against the actual code you changed.
3. Update `provides` and `uses` first.
4. Update `invariants` and `side_effects` next.
5. Confirm `tests` still point to the best verification surface.
6. Only then consider the file ready for review.

### Rollout Policy

Because the repository already has many in-scope files, adoption should be strict going forward:
- new files must start compliant
- touched files must be brought into compliance as part of the same change
- untouched legacy files can be backfilled separately

That rule avoids a giant one-time migration while still driving the repository toward full coverage.
