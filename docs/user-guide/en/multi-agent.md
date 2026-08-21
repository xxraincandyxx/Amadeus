# Multi-agent work

The Agents workspace makes delegated work visible. A root session is a **coordinator**. When it delegates a focused task, Amadeus creates a child session with its own conversation, tools, status, and context.

## Follow delegation

Open **Agents** from the sidebar to see the hierarchy. Each row shows the agent role, delegated task, parent, child count, and current status. Select any row to open that agent's conversation.

## Understand ownership

- Coordinators own the overall objective and combine delegated results.
- Sub-agents own a focused task and report their result to the parent.
- Nested sub-agents appear one level deeper in the hierarchy.
- A child waiting for approval can be opened and resolved without losing the parent conversation.

## Choose between a session and delegation

Create a new coordinator for independent work. Let a coordinator delegate when several focused tasks contribute to one shared outcome.

## Design a workflow

Open **Workflows** from the sidebar or run `/workflow` to create a visual agent architecture. Drag Trigger, Agent, Tool, Condition, Approval, and Output nodes onto the canvas, connect their handles, then configure the selected node in the inspector.

Workflow definitions are validated and saved locally. Use Import and Export to move the versioned JSON definition between clients. Running a custom graph is not available yet because the core runtime does not expose serialized node implementations through the HTTP API.

## Watch status

Running and approval-required agents need attention first. Completed agents remain available as evidence. Failed agents retain their conversation so the failure can be inspected or retried from the parent task.
