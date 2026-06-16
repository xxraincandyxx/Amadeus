# Agents Guide

Project agent definitions and inventories live under `.amadeus/agents/`.

Current behavior:

- user agent root: `~/.amadeus/agents`
- project agent root: `.amadeus/agents`
- markdown files in these roots are included in context inventory reporting

Current inventory discovery:

- direct `*.md` files inside the root
- nested `AGENT.md`
- nested `agent.md`

Recommended layout:

```text
.amadeus/agents/
  reviewer/
    AGENT.md
  planner.md
```

Suggested `AGENT.md` structure:

```md
# Reviewer Agent

Purpose:
- Review code changes for regressions, missing tests, and unsafe behavior.

Focus:
- Bugs first
- Risk assessment
- Minimal change recommendations

Working style:
- Read impacted code before proposing changes
- Prefer concrete reproductions over speculation
```

Current note:

- These markdown files are discoverable as context inventory today.
- A fuller structured agent-definition format is still evolving under the orchestra/runtime roadmap.

Examples:

- [reviewer/AGENT.example.md](/.amadeus/agents/reviewer/AGENT.example.md)
- [planner/agent.example.md](/.amadeus/agents/planner/agent.example.md)
