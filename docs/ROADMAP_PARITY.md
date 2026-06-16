# Amadeus Parity Roadmap

This document captures the current architectural gaps between Amadeus and the `claw-code-parity` reference baseline, the execution order for closing them, and the concrete first phases to implement.

## Current Gap Summary

### 0. Architecture

- Amadeus now uses a multi-crate workspace with a root compatibility facade plus dedicated crates for API, TUI, runtime models, config, commands, permissions, hooks, skills, telemetry, messages, events, and related support code.
- The remaining architecture gap is not crate count. `crates/core` still owns too many live runtime responsibilities: the agent loop, provider clients, tool registry, built-in tools, MCP adapter, policy, hooks integration, telemetry integration, and orchestration surface.
- The reference baseline has sharper subsystem ownership and fewer cross-cutting dependencies through the central runtime crate.

### 1. Telemetry and Observability

- Amadeus has a telemetry crate with JSONL and in-memory sinks, plus structured runtime events in parts of the agent and orchestration paths.
- The remaining gap is complete event coverage and request/tool profiling across sessions, prompts, hooks, approvals, streaming, and worker state transitions.
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

- Amadeus has a dedicated permissions crate with modes and rule parsing, plus core enforcement around dangerous operations.
- The remaining gap is consistent workspace boundary validation, explainable decisions across all tool paths, and deeper hook-driven overrides.
- The reference baseline has more permission modes, rule-based matching, hook-driven overrides, and stronger workspace validation heuristics.

### 6. Configuration

- Amadeus loads structured settings from global, workspace, and workspace-local settings files, with typed sections for several subsystems.
- The remaining gap is stronger schema validation, clearer ownership between `crates/config` and `crates/core`, and deeper typed coverage for MCP, LSP, worker lifecycle, and tools.
- The reference baseline supports a deeper 3-tier merge model with validation and richer typed sections.

### 7. Worker Boot Protocol

- Amadeus currently has no explicit worker lifecycle state machine.
- The reference baseline models worker boot and execution as explicit states with trust handling and prompt delivery validation.

## Execution Order

The work should be done in this order:

1. Finish workspace ownership cleanup around `crates/core`
2. Complete telemetry coverage across agent, tool, hook, approval, and worker flows
3. Harden configuration validation and typed subsystem settings
4. Expand permission enforcement into a consistent rule and boundary system
5. Expand hooks into a richer, configurable lifecycle system
6. Add a worker boot and lifecycle protocol
7. Replace the current MCP adapter-only approach with a full MCP lifecycle stack
8. Add LSP integration

## Why This Order

- Workspace ownership cleanup should happen first because many target crates already exist, but `core` still owns live implementations that should move behind narrower interfaces.
- Telemetry coverage should come before deeper MCP and worker lifecycle work so those systems produce structured diagnostics from the start.
- Configuration and permissions should stabilize before hook expansion, MCP auth, and worker boot because those subsystems depend on policy and settings resolution.
- MCP should come before LSP if the primary parity target is tool/runtime lifecycle depth rather than editor assistance first.

## Target Workspace Shape

The current workspace already has many of the target crates. The remaining target is to move implementation ownership out of `crates/core` where a narrower crate already exists, while keeping the root compatibility facade stable.

- `crates/runtime`
  Live runtime ownership for sessions, team coordination, worker lifecycle, approval routing, and eventually the agent loop
- `crates/tools`
  Future home for the tool trait, tool registry, built-in tools, and tool bridge integration
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
  Optional compatibility layer if the root crate becomes too large as a facade

The goal is not to copy the exact crate count of the reference baseline. The goal is to copy the boundary discipline.

## Phase Schedule

### Phase 0. Workspace Ownership Cleanup

Estimated duration: 4 to 6 days

Scope:

- Move live implementations from `core` into existing narrower crates where the crate boundary already exists.
- Create missing workspace crates only where no suitable crate exists yet, such as `tools`, `mcp`, or `lsp`.
- Preserve compatibility through root and core re-exports during migration.
- Reduce direct `tui` and `api` dependencies on broad `core` modules.

Acceptance:

- `core` is no longer the primary ownership point for unrelated subsystems.
- `tui` and `api` depend on narrower crates where practical.
- Compile and tests still pass through compatibility shims.

### Phase 1. Telemetry Coverage

Estimated duration: 3 to 4 days

Scope:

- Emit structured events for all sessions, prompts, tool runs, hooks, approvals, streams, and worker state transitions.
- Add request and tool profiling where missing.
- Ensure API, TUI, and tests consume the same telemetry model.

Acceptance:

- Session and tool events are queryable without scraping logs.
- Tests can assert against in-memory telemetry sinks.

### Phase 2. Configuration Validation

Estimated duration: 4 to 5 days

Scope:

- Keep config ownership in `crates/config` and remove duplicated parsing behavior from `core`.
- Add stronger schema validation and user-facing diagnostics.
- Complete typed sections for permissions, hooks, MCP, LSP, telemetry, team, worker lifecycle, and tools.

Acceptance:

- Layered config is deterministic and validated.
- Runtime behavior does not depend on scattered ad hoc config parsing.

### Phase 3. Permissions

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

### Phase 4. Hooks

Estimated duration: 3 to 4 days

Scope:

- Add broader lifecycle events
- Allow conditions based on tool name, command pattern, path scope, permission mode, and success or failure
- Support richer actions such as continue, ask, deny, mutate context or env, and emit telemetry

Acceptance:

- hooks become a general policy and automation layer, not only shell callbacks

### Phase 5. Worker Boot Protocol

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

### Phase 6. MCP Lifecycle

Estimated duration: 1.5 to 2 weeks

Scope:

- Promote MCP into a dedicated crate
- Add transport abstraction
- Add JSON-RPC lifecycle handling
- Add server discovery, validation, degraded-mode reporting, reconnect behavior, and auth hooks

Acceptance:

- MCP is a managed subsystem rather than a thin adapter wrapper

### Phase 7. LSP Integration

Estimated duration: 1 to 1.5 weeks

Scope:

- Add a dedicated LSP crate
- Add registry-based language server integration
- Support definition, references, diagnostics, hover, symbols, and completion

Acceptance:

- code understanding can use semantic tooling instead of only lexical search

## Recommended Immediate Work

Start with:

1. Phase 0: move one live subsystem out of `core` behind compatibility re-exports.
2. Phase 1: fill telemetry gaps around the moved subsystem before starting the next extraction.

This is the safest first move because the workspace shape already exists; the next value comes from making those boundaries real one subsystem at a time.
