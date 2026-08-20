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

When a node suspends for approval or external input, the run returns a
`WorkflowAgentCheckpoint`. Pass it back to `WorkflowAgentRegistry::resume`; the registry routes
it to the agent that owns it and preserves the original run identifier.

## Current Compatibility Boundary

The existing `amadeus::Agent<C>` remains the production ReAct implementation. It is not yet an
alias for `WorkflowAgent`. The migration will extract model calls, tools, approvals, memory,
compaction, and event streaming into reusable nodes, then express the existing agent as a ReAct
workflow preset without changing its public behavior.
