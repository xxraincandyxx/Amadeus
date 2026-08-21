# Tools and approvals

Agents use tools to inspect the workspace, edit files, run commands, and access configured services. Tool activity appears inline with the conversation.

## Inspect a tool call

Select a tool row to expand its input and output. File writes and edits show a line diff: removed lines are red and added lines are green. Running tools stay visible until they complete.

## Handle approvals

An approval card explains the tool, action, and input before execution continues.

- **Allow once** approves only this request.
- **Always allow** approves future matching requests for the active policy scope.
- **Deny** rejects the operation and returns the decision to the agent.

Review paths, commands, and network destinations before approval. If a child agent requests approval, open that child from the Agents workspace.

## Stop active work

Use the stop button or `/cancel` to cancel the active turn. Completed file changes are not automatically reverted, so inspect tool results before continuing.
