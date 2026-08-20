# Agent Architectures

## Hierarchy

Amadeus treats workflows, agents, and runs as separate layers:

```text
Workflow<S, R>
    Reusable architecture and control-flow graph.

WorkflowAgent<S, R>
    One identity + one workflow + one resource set + one runner configuration.

WorkflowAgentRun<S>
    One execution session with independently owned state.

WorkflowAgentRegistry<S, R>
    A collection of agents that may use different workflows and resources.
```

A workflow is a blueprint, not an agent. An agent binds that blueprint to a role and its
runtime dependencies. Starting an agent creates a run; it does not mutate the workflow.

## Choosing An API

| API | Use it when |
| --- | --- |
| `Agent<C>` | You want the current production ReAct agent with built-in model, tool, approval, compaction, and event behavior. |
| `Workflow<S, R>` | You are defining reusable control flow over typed state and resources. |
| `WorkflowAgent<S, R>` | You want to bind one workflow to a stable identity, capabilities, resources, and runner settings. |
| `WorkflowAgentRegistry<S, R>` | You need to hold and explicitly address multiple workflow-backed agents. |

`WorkflowAgent` is additive during the migration. It does not replace or wrap the existing
`Agent<C>` yet.

## Sharing And Variation

The following configurations are all supported:

- two agents using different workflow graphs
- two agents sharing one workflow but using different resource values
- one agent executing multiple isolated runs
- a suspended run resuming through its original agent

One registry has a common `S` state type and `R` resource type. This gives every registered
agent the same task and result contract while allowing each agent to use a different graph.
For more heterogeneous systems, use enums for state variants and traits inside the resource
bundle.

## Ownership And Lifecycle

The layers have deliberately different ownership rules:

- `Workflow<S, R>` owns the graph and its nodes. It is validated when built.
- `WorkflowAgent<S, R>` owns or shares one workflow and one resource value.
- `WorkflowAgentRun<S>` owns the state for one execution.
- `WorkflowAgentCheckpoint<S>` owns suspended state until it is resumed.
- `WorkflowAgentRegistry<S, R>` stores agents behind `Arc`, so retrieved agents can run without
  holding a mutable registry borrow.

`WorkflowAgent::new` accepts owned values and shares them internally. Use
`WorkflowAgent::from_shared` when several agents should reuse the same `Arc<Workflow<S, R>>` or
resource value. The workflow association cannot be changed after agent construction.

Run identifiers are monotonically assigned within one agent. They are preserved across
suspension and resume, but they are not globally unique without the accompanying `AgentId`.

## Workflow Execution

Each node receives `&R` and `&mut S`, then returns one transition:

| Transition | Effect |
| --- | --- |
| `Transition::next(node)` | Continue at a declared destination. |
| `Transition::suspend(reason)` | Return an agent-bound checkpoint and owned state. |
| `Transition::Complete` | Return the final state successfully. |
| `Transition::fail(message)` | End the run with an architecture failure. |

Every possible `next` destination must be listed by `Node::destinations`. Workflow construction
rejects missing entry nodes, duplicate node identifiers, and unknown declared destinations. The
runner also rejects dynamic transitions to unknown nodes and stops workflows that exceed their
configured transition limit.

## Typical Construction

Define the shared state and resource contract first:

```rust
struct TaskState {
    request: String,
    result: Option<String>,
}

struct AgentResources {
    models: ModelCatalog,
    tools: ToolCatalog,
}
```

Build distinct workflow architectures using nodes that implement `Node<TaskState,
AgentResources>`:

```rust
let research_workflow = Workflow::builder("search")
    .node("search", search_node)?
    .node("synthesize", synthesize_node)?
    .build()?;

let coding_workflow = Workflow::builder("plan")
    .node("plan", plan_node)?
    .node("implement", implement_node)?
    .node("review", review_node)?
    .build()?;
```

Bind each architecture to an agent:

```rust
let researcher = WorkflowAgent::new(
    WorkflowAgentIdentity::new("researcher").capability("research"),
    research_workflow,
    research_resources,
);

let coder = WorkflowAgent::new(
    WorkflowAgentIdentity::new("coder").capability("rust"),
    coding_workflow,
    coding_resources,
);
```

Register and execute them independently:

```rust
let researcher_id = researcher.identity().id();
let coder_id = coder.identity().id();

let mut agents = WorkflowAgentRegistry::new();
agents.register(researcher)?;
agents.register(coder)?;

let research_run = agents.run(researcher_id, research_state).await?;
let coding_run = agents.run(coder_id, coding_state).await?;
```

Registration and execution routing use `AgentId`, not names or capabilities. Capabilities are
metadata for a caller-provided router or a future supervisor component; the registry does not
automatically select an agent.

## Handling Results And Checkpoints

Match the returned status to recover final state or retain a checkpoint:

```rust
let run = agents.run(researcher_id, initial_state).await?;

match run.into_status() {
    WorkflowAgentRunStatus::Completed {
        state,
        transitions_completed,
    } => {
        use_result(state, transitions_completed);
    }
    WorkflowAgentRunStatus::Suspended(mut checkpoint) => {
        apply_external_input(checkpoint.state_mut());
        let resumed = agents.resume(checkpoint).await?;
        handle_resumed_run(resumed);
    }
}
```

When a node suspends for approval or external input, the run returns a
`WorkflowAgentCheckpoint`. Pass it back to `WorkflowAgentRegistry::resume`; the registry routes
it to the agent that owns it and preserves the original run identifier.

Calling `WorkflowAgent::resume` directly with another agent's checkpoint returns
`WorkflowAgentError::CheckpointAgentMismatch`. Calling the registry after the owning agent was
removed returns `WorkflowAgentError::UnknownAgent`.

## Concurrent Runs

Retrieve shared agents before spawning independent runs:

```rust
let researcher = agents.get(researcher_id).ok_or(missing_agent)?;
let coder = agents.get(coder_id).ok_or(missing_agent)?;

let (research, coding) = tokio::join!(
    researcher.run(research_state),
    coder.run(coding_state),
);
```

Each call owns its state and receives an agent-local run identifier. Concurrent execution also
requires the application's state, resources, and node implementations to satisfy the normal
Tokio `Send` and `Sync` requirements for the way tasks are spawned.

## Error Surface

| Error | Meaning |
| --- | --- |
| `DuplicateAgent` | The registry already contains the stable `AgentId`. |
| `UnknownAgent` | Execution or resume targeted an unregistered agent. |
| `CheckpointAgentMismatch` | A checkpoint was passed directly to an agent that did not create it. |
| `Workflow` | Workflow validation or node execution failed. |

Node errors retain the node identifier through `WorkflowError::NodeExecution`, while deliberate
architecture failures use `WorkflowError::ArchitectureFailure`.

## Current Limitations

- The registry requires one common `S` and `R` type.
- Registry routing is explicit by `AgentId`; capability selection is not implemented here.
- Checkpoints are owned in memory and are not yet a durable serialized format.
- Fork, join, supervisor, model-call, tool-call, approval, memory, and compaction nodes are not
  yet provided as built-in components.
- `WorkflowAgent` does not yet emit the legacy `AgentEvent` stream.

## Current Compatibility Boundary

The existing `amadeus::Agent<C>` remains the production ReAct implementation. It is not yet an
alias for `WorkflowAgent`. The migration will extract model calls, tools, approvals, memory,
compaction, and event streaming into reusable nodes, then express the existing agent as a ReAct
workflow preset without changing its public behavior.
