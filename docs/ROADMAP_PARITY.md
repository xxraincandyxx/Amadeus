# Amadeus Parity Roadmap

This document captures the current architectural gaps between Amadeus and the `claw-code-parity` reference baseline, the execution order for closing them, and the concrete first phases to implement.

## Current Gap Summary

### 0. Architecture

- Amadeus currently uses a 3-crate workspace: `core`, `tui`, and `api`.
- The current `core` crate is still a large monolith and carries too many responsibilities.
- The reference baseline uses a larger workspace with sharper crate boundaries and clearer subsystem ownership.

### 1. Telemetry and Observability

- Amadeus currently has basic tracing logs.
- The reference baseline has a dedicated telemetry subsystem with structured session tracing, analytics events, JSONL sinks, in-memory sinks, and request profiling.

### 2. Hooks

- Amadeus currently supports shell hooks with a limited lifecycle surface.
- The reference baseline supports more configurable hooks with deeper lifecycle integration and policy interaction.

### 3. MCP

- Amadeus currently provides an adapter layer that wraps external MCP tools as local tool objects.
- MCP servers must be configured manually.
- The reference baseline implements a fuller MCP lifecycle with transport abstraction, discovery, lifecycle validation, degraded-mode reporting, bridging, and auth support.

### 4. LSP Integration

- Amadeus currently has no language-server integration.
- Code understanding remains lexical and file-based.
- The reference baseline exposes LSP-backed symbol, reference, diagnostics, definition, hover, completion, and formatting workflows.

### 5. Permissions

- Amadeus currently has a simpler permission model plus dangerous-pattern checks.
- The reference baseline has more permission modes, rule-based matching, hook-driven overrides, and stronger workspace validation heuristics.

### 6. Configuration

- Amadeus already loads structured settings from `~/.amadeus/settings.json` and `.amadeus/settings.json`.
- The remaining gap is multi-source depth, local overrides, deep merge semantics, and stronger validation.
- The reference baseline supports a deeper 3-tier merge model with validation and richer typed sections.

### 7. Worker Boot Protocol

- Amadeus currently has no explicit worker lifecycle state machine.
- The reference baseline models worker boot and execution as explicit states with trust handling and prompt delivery validation.

## Execution Order

The work should be done in this order:

1. Remove mesh mode
2. Split the monolithic `core` crate into sharper workspace crates
3. Add a dedicated telemetry crate
4. Rebuild configuration into a validated, layered config system
5. Expand the permission system into rule-based policy evaluation
6. Expand hooks into a richer, configurable lifecycle system
7. Add a worker boot and lifecycle protocol
8. Replace the current MCP adapter-only approach with a full MCP lifecycle stack
9. Add LSP integration

## Why This Order

- Mesh removal should happen first because it is a low-depth feature with cross-cutting behavior and little architectural value.
- Workspace restructuring should happen early because telemetry, MCP, LSP, hooks, and permissions all become harder if they continue to accumulate inside `core`.
- Telemetry should come before deeper MCP and worker lifecycle work so those systems produce structured diagnostics from the start.
- Configuration and permissions should stabilize before hook expansion, MCP auth, and worker boot, because those subsystems depend on policy and settings resolution.
- MCP should come before LSP if the primary parity target is tool/runtime lifecycle depth rather than editor assistance first.

## Target Workspace Shape

The current 3-crate layout should evolve toward a sharper workspace:

- `crates/runtime`
  Agent loop, sessions, team coordination, worker lifecycle, approval routing
- `crates/tools`
  Tool trait, tool registry, built-in tools, tool bridge integration
- `crates/commands`
  Slash command specs, parsers, command handlers, shared command metadata
- `crates/config`
  Settings loading, layered merge, validation, typed config sections
- `crates/permissions`
  Modes, rules, validators, workspace boundaries, approval decisions
- `crates/hooks`
  Hook config, hook runner, hook policy integration, hook result handling
- `crates/mcp`
  MCP transports, protocol, discovery, lifecycle management, auth, bridge registry
- `crates/telemetry`
  Event model, sinks, profiling, test sinks, session trace recording
- `crates/lsp`
  Language server registry and query APIs
- `crates/tui`
  Terminal UI only
- `crates/api`
  HTTP API only
- `crates/compat`
  Temporary compatibility re-exports during migration

The goal is not to copy the exact crate count of the reference baseline. The goal is to copy the boundary discipline.

## Phase Schedule

### Phase 0. Remove Mesh Mode

Estimated duration: 1 day

Scope:

- Remove `MeshManager` and `.amadeus_mesh` lock-file behavior
- Remove auto-discovery and auto-attachment behavior from startup
- Remove mesh indicator state from the TUI footer
- Remove mesh-specific testflow and capture metadata
- Remove the `mesh` feature flag if it has no remaining behavior

Acceptance:

- `cargo run --features full -- --server` does not write `.amadeus_mesh`
- TUI startup in the same working directory does not auto-connect to a server instance
- Footer does not display mesh state
- No recorder, snapshot, or comparison code references mesh

### Phase 1. Workspace Restructure

Estimated duration: 4 to 6 days

Scope:

- Split `core` into `runtime`, `tools`, `commands`, `config`, `permissions`, and `hooks`
- Preserve compatibility through temporary re-exports
- Reduce direct `tui` and `api` dependencies on the monolithic crate

Acceptance:

- `core` is no longer the primary ownership point for unrelated subsystems
- `tui` and `api` depend on narrower crates
- compile and tests still pass through compatibility shims

### Phase 2. Telemetry

Estimated duration: 3 to 4 days

Scope:

- Add a dedicated telemetry crate
- Emit structured events for sessions, prompts, tool runs, hooks, approvals, and worker state transitions
- Add JSONL and memory sinks
- Add request and tool profiling

Acceptance:

- session and tool events are queryable without scraping logs
- tests can assert against in-memory telemetry sinks

### Phase 3. Configuration

Estimated duration: 4 to 5 days

Scope:

- Move config logic into a dedicated crate
- Support:
  - `~/.amadeus/settings.json`
  - `.amadeus/settings.json`
  - `.amadeus/settings.local.json`
- Add deep merge and schema validation
- Introduce typed sections for permissions, hooks, MCP, LSP, telemetry, team, and tools

Acceptance:

- layered config is deterministic and validated
- runtime behavior does not depend on scattered ad hoc config parsing

### Phase 4. Permissions

Estimated duration: 4 to 5 days

Scope:

- Expand to richer modes:
  - `read-only`
  - `workspace-write`
  - `danger-full-access`
  - `prompt`
  - `allow`
- Add rule-based matchers such as `bash(git:*)`, `bash(rm -rf)`, and tool-scoped rules
- Add workspace boundary validation and better read-only heuristics
- Allow hook-driven ask and deny overrides

Acceptance:

- policy decisions are composable and explainable
- dangerous access checks are not limited to a small static blocklist

### Phase 5. Hooks

Estimated duration: 3 to 4 days

Scope:

- Add broader lifecycle events
- Allow conditions based on tool name, command pattern, path scope, permission mode, and success or failure
- Support richer actions such as continue, ask, deny, mutate context or env, and emit telemetry

Acceptance:

- hooks become a general policy and automation layer, not only shell callbacks

### Phase 6. Worker Boot Protocol

Estimated duration: 4 days

Scope:

- Add explicit worker states:
  - `spawning`
  - `trust_required`
  - `ready_for_prompt`
  - `prompt_accepted`
  - `running`
  - `blocked`
  - `finished`
  - `failed`
- Add prompt delivery acknowledgement and misdelivery detection
- Expose worker state transitions through runtime, API, and TUI surfaces

Acceptance:

- multi-agent behavior is observable and stateful rather than implicit

### Phase 7. MCP Lifecycle

Estimated duration: 1.5 to 2 weeks

Scope:

- Promote MCP into a dedicated crate
- Add transport abstraction
- Add JSON-RPC lifecycle handling
- Add server discovery, validation, degraded-mode reporting, reconnect behavior, and auth hooks

Acceptance:

- MCP is a managed subsystem rather than a thin adapter wrapper

### Phase 8. LSP Integration

Estimated duration: 1 to 1.5 weeks

Scope:

- Add a dedicated LSP crate
- Add registry-based language server integration
- Support definition, references, diagnostics, hover, symbols, and completion

Acceptance:

- code understanding can use semantic tooling instead of only lexical search

## Recommended Immediate Work

Start with:

1. Phase 0: remove mesh mode
2. Phase 1: split `core` into narrower crates while preserving compatibility

This is the safest first move because it removes low-value runtime complexity and creates the boundaries needed for the remaining work.
