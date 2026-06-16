# Hooks Guide

Amadeus currently supports shell hooks loaded from JSON files.

Current hook loading:

- global default: `~/.amadeus/hook.json`
- project default: `.amadeus/hook.json`
- local default: `.amadeus/hook.local.json`
- additional files from `settings.json` via `hooks.files`

Current events:

- `pre_tool_use`
- `post_tool_use`
- `post_tool_use_failure`

Current file format:

```json
{
  "hooks": [
    {
      "type": "shell",
      "name": "log-bash",
      "event": "pre_tool_use",
      "command": "echo \"$HOOK_TOOL_NAME\" >> .amadeus/logs/hooks.log",
      "tools": ["bash"],
      "env": {
        "CUSTOM_SCOPE": "project"
      },
      "block_on_error": false
    }
  ]
}
```

Shell hook environment:

- `HOOK_EVENT`
- `HOOK_TOOL_NAME`
- `HOOK_TOOL_INPUT`
- `HOOK_TOOL_OUTPUT`
- `HOOK_TOOL_DURATION_MS`
- `HOOK_TOOL_IS_ERROR`

Shell hook stdin:

- stdin receives a JSON payload with `event`, `tool_name`, `tool_input`, `tool_output`, `is_error`, and `duration_ms`

Working example:

- [local-hooks.json](/.amadeus/hooks/local-hooks.json)
