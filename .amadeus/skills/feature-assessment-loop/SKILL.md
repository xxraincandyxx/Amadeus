---
name: feature-assessment-loop
description: Assess the Amadeus workspace in read-only mode, using tmux-cli, targeted tests, hooks, and subagents to confirm feature regressions before reporting them.
allowed_tools:
  - bash
  - read_file
  - glob
  - grep
  - web_fetch
  - todo
  - sub_agent
---

You are running a full feature assessment of the Amadeus workspace.

Rules:
- Stay in read-only mode. Do not attempt file edits or destructive commands.
- Base coverage on `docs/TMUX_TEST_FLOW.md`, `docs/TUI_GUIDE.md`, the API/runtime tests, and live `tmux-cli` checks when those checks improve confidence.
- Use `todo` to track packs.
- Split independent packs with `sub_agent` when that improves coverage.
- Prefer confirming behavior with the narrowest command that proves it.
- If a bug is only suspected, label it as `Unconfirmed` and explain what evidence is missing.
- Only list `Confirmed Bugs` when you have a concrete reproduction or directly observed mismatch.

Coverage checklist:
1. Config hierarchy, hooks, skills, and permission mode behavior.
2. TUI startup, shortcuts, completion, session switching, approvals, and tool monitor behavior from `docs/TMUX_TEST_FLOW.md`.
3. API and session-management behavior that shares the same core runtime.
4. Agent-team and subagent coordination surfaces.

Output format:
# Assessment Summary
- Scope covered
- Commands used

## Confirmed Bugs
- One bullet per bug with reproduction, expected result, actual result, and likely code area

## Unconfirmed Findings
- Only if evidence is incomplete

## Clean Areas
- Short list of areas checked with no bug found

Context:
{context}
