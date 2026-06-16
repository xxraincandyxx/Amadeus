# `.amadeus` Guide

This directory is the project-scoped configuration root for Amadeus.

Current precedence:
1. `~/.amadeus/settings.json`
2. `.amadeus/settings.json`
3. `.amadeus/settings.local.json`

Current supported layout:

- `settings.json`
  Shared project settings.
- `settings.local.json`
  Local developer overrides. Do not commit personal secrets here.
- `hooks/`
  Hook configuration files referenced from `settings.json`.
- `skills/`
  Project skills. See [skills/README.md](/.amadeus/skills/README.md).
- `agents/`
  Project agent definitions and markdown inventories. See [agents/README.md](/.amadeus/agents/README.md).
- `mcp/`
  MCP notes and examples. See [mcp/README.md](/.amadeus/mcp/README.md).

Example files in this repo:

- [settings.example.json](/.amadeus/settings.example.json)
- [settings.local.example.json](/.amadeus/settings.local.example.json)
- [hooks/local-hooks.json](/.amadeus/hooks/local-hooks.json)
- [skills/feature-assessment-loop/SKILL.md](/.amadeus/skills/feature-assessment-loop/SKILL.md)
- [skills/code-review/SKILL.example.md](/.amadeus/skills/code-review/SKILL.example.md)
- [agents/reviewer/AGENT.example.md](/.amadeus/agents/reviewer/AGENT.example.md)
- [agents/planner/agent.example.md](/.amadeus/agents/planner/agent.example.md)
- [mcp/servers.example.json](/.amadeus/mcp/servers.example.json)

Current settings sections:

- top-level runtime fields such as `provider`, `model`, `session_log_dir`, and compaction settings
- `hooks.files`
- `telemetry.enabled` and `telemetry.jsonl_path`
- `permissions.mode`
- `permissions.allow`
- `permissions.ask`
- `permissions.deny`
- `permissions.rules`
- `permissions.additionalDirectories`

Important current limitation:

- MCP support exists in core runtime code, but MCP server configuration is not yet loaded from `settings.json`. That integration is still part of the roadmap.
