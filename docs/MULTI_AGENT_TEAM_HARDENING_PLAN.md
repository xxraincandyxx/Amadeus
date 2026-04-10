# Multi-Agent Team Hardening Plan

## Goal

Upgrade Amadeus multi-agent collaboration from capability-based delegation into a more robust team workflow with:

- richer shared task state
- mailbox-style teammate coordination
- safer worker selection
- priority-aware queueing
- actual retry behavior

This change stays inside the core SDK/runtime crates and preserves the existing `amadeus::...` compatibility surface.

## Constraints

- Follow `docs/AGENT_WORKFLOW_CHECKLIST.md`
- Keep orchestration logic in runtime/core crates
- Avoid breaking existing `Supervisor`, `Manager`, `team`, and `worker` compatibility paths
- Keep the first implementation incremental and testable

## Current Problems

1. `TeamTask` is a thin status record and cannot model real collaboration state.
2. `call_peer` blocks synchronously on a single response and does not carry coordination metadata.
3. Worker selection does not exclude the requester or use task priority.
4. Queue processing ignores retry settings already present in config.
5. The runtime has no mailbox/event model for review requests, handoffs, or status updates.

## Implementation Scope

This implementation will deliver the first robust layer:

1. richer team task metadata and transitions
2. mailbox primitives in runtime state
3. requester-aware peer dispatch
4. priority-aware queued scheduling
5. retry handling with bounded attempts
6. focused tests for the new behavior

It will not yet add disk persistence or a separate lead-planner agent abstraction. The code will be shaped so those can be added without reworking the new task and mailbox contracts.

## Architecture Changes

### 1. Team Model

Extend runtime team state to support:

- `Ready`, `Blocked`, `InProgress`, `Review`, `Completed`, `Failed`
- dependency tracking
- attempt counting
- owned-file hints
- artifact references
- lease timestamps for claims

New registry operations should support:

- queueing a task with metadata
- claiming only when dependencies are satisfied
- recording artifacts and messages
- moving a task to review or failure without losing history

### 2. Mailbox Model

Add lightweight mailbox events at the runtime level:

- direct message
- review request
- artifact publication
- status update

Mailbox events are stored on the team registry so a future persistent store can serialize them directly.

### 3. Scheduler

Add selection and queue improvements:

- sort queued tasks by priority before dispatch
- exclude the requester from peer-routing candidates by default
- preserve capability filtering
- use retry settings from config instead of dropping failed work immediately

### 4. Core Runtime

Update `OrchestraRuntime` so that:

- peer requests carry requester context
- failed queued tasks can be retried up to `max_retries`
- mailbox and task status are recorded in the shared registry path where applicable
- existing `execute()` and `call_peer` flows remain compatible

### 5. Compatibility

Compatibility rules for this change:

- keep public re-exports working
- do not remove existing task constructors
- additive fields should have sensible defaults/builders
- tests that depend on simple success/failure flows must continue to pass

## File-Level Plan

### Runtime contracts

- `crates/runtime/src/team.rs`
  - extend `TeamTaskStatus`
  - add mailbox/event types
  - add richer `TeamTask` fields
  - add dependency-safe claim and update methods

- `crates/runtime/src/worker.rs`
  - extend `Task` metadata/builders for dependency and ownership hints
  - extend `HelpRequest` with retry-safe routing metadata

- `crates/runtime/src/scheduler.rs`
  - add requester exclusion support
  - add priority-aware candidate handling helpers

- `crates/runtime/src/lib.rs`
  - re-export new runtime team/mailbox types

### Core orchestration

- `crates/core/src/agent/orchestra.rs`
  - implement queue priority ordering
  - implement bounded retries
  - apply requester exclusion during peer dispatch
  - preserve telemetry behavior

- `crates/core/src/tools/peer.rs`
  - send richer peer request context
  - keep `call_peer` contract stable

### Tests

- `tests/p2p_test.rs`
  - verify requester exclusion
  - verify peer routing still succeeds

- `tests/e2e_product_flow.rs`
  - keep narrative flow intact under new runtime behavior

- runtime unit tests
  - verify dependency-aware claims
  - verify mailbox recording
  - verify retry and priority behavior

## Execution Order

1. Extend runtime types and registry APIs.
2. Update runtime scheduler helpers.
3. Update core orchestra runtime to consume the new contracts.
4. Update peer tool request shape.
5. Add and adjust tests.
6. Run `cargo check --features full`.
7. Run targeted tests for runtime, p2p, and e2e orchestra flows.
8. Run `gitnexus_detect_changes()`.

## Success Criteria

The implementation is acceptable if:

- peer requests cannot route back to the requester by default
- queued tasks dispatch by priority when multiple tasks are pending
- failed queued work retries up to configured limits
- team state can represent blocked/review/in-progress work
- mailbox events can be recorded for coordination
- existing public compatibility surfaces still compile
