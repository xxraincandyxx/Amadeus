# Agent Architecture Runtime Migration

## Status

Accepted for incremental implementation on `codex/agent-architecture-runtime`.

## Motivation

Amadeus currently exposes a configurable ReAct agent. Callers can replace the model,
tools, prompts, memory providers, hooks, and policy, but the control algorithm remains
embedded in the `Agent` streaming loop. Multi-agent orchestration selects a worker and
then delegates to that same loop.

The target is a framework in which an agent is assembled from a reusable architecture:

```text
Agent = Architecture + State + Resources + Runtime
```

The model is a resource used by workflow nodes. It does not own the workflow.

## Design Goals

1. Make control flow a public, testable abstraction.
2. Keep model, tool, memory, approval, and telemetry services injectable.
3. Support sequential, branching, suspended, and completed execution.
4. Preserve the existing `Agent<C>` API while it becomes a ReAct compatibility preset.
5. Keep the execution kernel transport-agnostic and independent of provider clients.
6. Allow deterministic unit tests without invoking an LLM or tool process.
7. Establish checkpoint and resume semantics before adding durable persistence.

## Layer Model

### Runtime kernel

The `runtime` crate owns generic execution semantics:

- workflow node identifiers
- typed workflow state
- node execution
- transitions between nodes
- workflow validation
- run completion and suspension
- execution limits

It must not depend on `core`, provider clients, concrete tools, or frontends.

### Resources

Resources provide effects required by an architecture:

- model clients
- tool catalogs and executors
- memory providers
- approval channels
- event and telemetry sinks
- clocks and checkpoint stores

The first kernel slice keeps resources generic. Core-specific resource composition will
be introduced when the current ReAct loop is decomposed.

### Components

Reusable nodes will wrap individual operations such as:

- model invocation
- tool routing and execution
- approval gates
- memory reads and writes
- context compaction
- retries and stop conditions
- fork, join, and worker selection

### Architectures

Architectures compose components into executable workflows. Planned presets include:

- ReAct
- plan and execute
- planner, worker, and reviewer
- reflection
- router and experts
- supervisor team

## Initial Public Contract

The first implementation introduces these concepts in `amadeus_runtime`:

- `NodeId`: stable identifier within a workflow.
- `Node<S, R>`: asynchronous state transition over state `S` and resources `R`.
- `Transition`: continue, suspend, complete, or fail.
- `Workflow<S, R>`: validated node collection with a single entry point.
- `WorkflowRunner`: deterministic executor with a transition limit.
- `RunStatus`: completed or suspended result carrying final state.

Nodes emit their next node explicitly. This keeps branching logic in architecture code
and avoids requiring a configuration language before the Rust API has stabilized.

## Compatibility Strategy

The migration follows a strangler pattern:

1. Add the workflow kernel without changing `Agent<C>`.
2. Extract model-turn, tool-execution, approval, and compaction operations from the
   current loop behind focused core components.
3. Implement `ReAct` using those components and the workflow kernel.
4. Route `Agent<C>` through the ReAct architecture while preserving events and results.
5. Replace orchestra-specific execution loops with supervisor and scheduling components.
6. Deprecate duplicated control-flow code only after parity tests pass.

Every stage must compile and test independently. Existing public paths remain valid
through compatibility re-exports.

## Checkpoint Semantics

A suspended workflow returns:

- the current node identifier
- the owned state value
- a typed suspension reason
- the number of completed transitions

The caller may resume from that node after satisfying the external condition. Durable
serialization is intentionally deferred because workflow state can be application-specific.

## Failure Semantics

Workflow construction errors and execution errors remain distinct:

- construction rejects missing entry nodes, duplicate identifiers, and unknown targets
- node failures are returned with the node that failed
- transition-limit exhaustion is an execution failure, not a suspension

Core will translate runtime failures into `crate::error::Result<T>` at its boundary.

## Non-Goals For The First Slice

- A YAML or JSON workflow language
- Dynamic plugin loading
- Distributed execution
- Durable checkpoint storage
- Replacing the current agent loop immediately
- Encoding provider-specific messages in the runtime crate

## Incremental Delivery

### Phase 1: Kernel

- Add typed workflow primitives and validation.
- Add deterministic runner tests for linear, branching, suspended, and bounded runs.
- Document the public contract in the architecture guide.

### Phase 2: Core components

- Extract one model turn from `loop_agent.rs`.
- Extract policy, approval, and tool execution as reusable operations.
- Preserve the existing event stream contract.

### Phase 3: ReAct compatibility architecture

- Compose the extracted operations into `ReAct`.
- Run existing integration and golden workflow tests unchanged.
- Switch `Agent<C>` internals to the architecture runtime.

### Phase 4: Multi-agent composition

- Model worker selection, delegation, queueing, and review as components.
- Unify `AgentOrchestrator` and `OrchestraRuntime` around shared execution semantics.

### Phase 5: Extension surface

- Publish additional architecture presets.
- Add visualization and declarative loading only after the Rust API is stable.

## Verification Gates

Each implementation commit must run:

```text
cargo check --features full
cargo test --features full
python3 scripts/check_source_headers.py
```

The final migration gate remains `./verify.sh`.
