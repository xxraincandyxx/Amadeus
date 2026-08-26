# Getting started

Amadeus is a local workspace for working with AI agents. A session keeps one conversation, its tool activity, approvals, and context together.

## Start a session

Select **New session**, give the session a useful task name, and enter a request in the composer. A good first request names the outcome, relevant files or area, and the verification you expect.

```text
Inspect the session API, fix the failing history refresh, and run the focused web tests.
```

Press Enter to send. Use Shift + Enter for a new line. The stop button cancels the active turn without deleting the session.

## Read the workspace

- The sidebar lists coordinators and delegated sub-agents.
- The status dot shows idle, running, completed, approval-required, or failed state.
- The header identifies the selected session and its parent when it is a sub-agent.
- Details shows role, parent, children, message count, tool count, and token usage.

## Keep work separated

Create a new session when the task needs independent context or ownership. Continue an existing session when the next request depends on its conversation or tool results.
