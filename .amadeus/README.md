# `.amadeus` Guide

This directory is the project-scoped configuration root for Amadeus.

Current precedence:
1. `~/.amadeus/settings.json`
2. `.amadeus/settings.json`
3. `.amadeus/settings.local.json`

User-wide preferences, including the TUI language, belong in `~/.amadeus/settings.json` so they follow you across workspaces.
Keep provider, model, and workspace-specific runtime settings in the project
`.amadeus/settings.json` files.

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
- `compact_prompt` for an inline automatic-compaction system prompt
- `compact_prompt_file` for a prompt file resolved relative to the settings file; `compact_prompt` takes precedence when both are set
- `hooks.files`
- `hooks.enabled`, `hooks.timeout_seconds`, `hooks.max_output_bytes`, and `hooks.sandbox`
- `telemetry.enabled` and `telemetry.jsonl_path`
- `permissions.mode`
- `permissions.allow`
- `permissions.ask`
- `permissions.deny`
- `permissions.rules`
- `permissions.additionalDirectories`
- `tui.language` (`en` or `zh-CN`)

Prompt profiles can customize the built-in system prompt through `builtin_sections`.
Use a string value to replace a section and `null` to remove it. Stable section IDs
are `core_loop`, `security`, `context_efficiency`, `engineering_standards`,
`task_management`, and `tool_usage`. Custom `sections` and `files` are still merged
according to the profile's `mode`.

```json
{
  "prompts": {
    "active_profile": "focused",
    "profiles": {
      "focused": {
        "builtin_sections": {
          "core_loop": "You are the implementation agent for this workspace.",
          "task_management": null
        }
      }
    }
  }
}
```

Important current limitation:

- MCP support exists in core runtime code, but MCP server configuration is not yet loaded from `settings.json`. That integration is still part of the roadmap.
