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
      "timeout_seconds": 10,
      "max_output_bytes": 65536,
      "sandbox": "workspace-write",
      "workdir": ".",
      "block_on_error": false
    }
  ]
}
```

Layer-loaded hooks inherit `hooks.timeout_seconds`, `hooks.max_output_bytes`, and
`hooks.sandbox` from `settings.json`. They run from the configured workspace by
default. Hook entries can override those values, and relative `workdir` values are
resolved from the workspace. Supported sandbox values are `read-only`,
`workspace-write`, and `danger-full-access`; omitting the value inherits the
runtime permission mode.

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

- [local-hooks.example.json](/.amadeus/hooks/local-hooks.example.json)
