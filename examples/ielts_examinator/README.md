# IELTS Examinator

An Amadeus example that turns the core agent loop into an IELTS practice examiner with both CLI and React web UI entrypoints.

This is for practice feedback only. It does not provide official IELTS scoring and is not affiliated with IELTS, Cambridge, the British Council, IDP, or any exam board.

## What It Does

- Runs IELTS Speaking, Writing, and rubric-review modes on top of `AgentBuilder`.
- Replaces the default coding-agent prompt with an IELTS examiner prompt profile at runtime.
- Uses an empty tool catalog so examiner turns stay conversational and assessment-focused.
- Writes normal Amadeus session logs for inspection and regression checks.
- Serves a React web UI inspired by `tw93/Kami`: warm parchment, ink-blue accent, serif hierarchy, quiet borders, and no hard shadows.

## Quick Start

Run the React web UI against the OpenAI-compatible model endpoint:

```bash
cargo run --example ielts_examinator --features full -- --web \
  --provider openai \
  --base-url http://YOUR_OPENAI_COMPATIBLE_HOST:PORT/v1 \
  --model YOUR_MODEL_ID \
  --api-key empty \
  --max-turns 2
```

Open the printed URL, usually:

```text
http://127.0.0.1:7878
```

Run a CLI writing evaluation:

```bash
cargo run --example ielts_examinator --features full -- writing \
  --provider openai \
  --base-url http://YOUR_OPENAI_COMPATIBLE_HOST:PORT/v1 \
  --model YOUR_MODEL_ID \
  --api-key empty \
  --max-turns 2 \
  --prompt "Evaluate this IELTS Writing Task 2 response and give concise feedback: ..."
```

`--api-key` can be any placeholder for OpenAI-compatible servers that ignore bearer authentication.

## Modes

| Mode | Purpose |
| --- | --- |
| `speaking` | Starts an IELTS Speaking practice exchange, cue card, or follow-up turn. |
| `writing` | Evaluates IELTS Writing Task 1 or Task 2 responses with band-style feedback. |
| `rubric` | Prints the practice scoring rubric used by the example. |

CLI examples:

```bash
cargo run --example ielts_examinator --features full -- speaking
cargo run --example ielts_examinator --features full -- writing --prompt "Evaluate this response: ..."
cargo run --example ielts_examinator --features full -- rubric
```

## Command Options

```text
--web               Serve the React web UI
--port PORT         Web UI port, default 7878
--prompt TEXT       Prompt or candidate response to send
--provider NAME     anthropic or openai
--base-url URL      Provider base URL
--model ID          Model identifier
--api-key KEY       Provider API key, or placeholder for local servers
--log-dir PATH      Session log directory
--max-turns N       Maximum model turns before stopping, default 4
--help, -h          Show help
```

## Directory Map

```text
examples/ielts_examinator/
├── README.md
├── main.rs
└── web/
    ├── index.html
    ├── app.js
    ├── styles.css
    └── css/
        ├── tokens.css
        ├── base.css
        ├── shell.css
        ├── composer.css
        ├── result.css
        └── responsive.css
```

`styles.css` is only an import manifest. Keep component and layout styles in the `web/css/` modules:

- `tokens.css`: Kami palette, font stacks, shared design tokens.
- `base.css`: reset, body typography, global animations.
- `shell.css`: page shell, hero, model card, shared panels.
- `composer.css`: mode cards, prompt editor, turn controls, primary action.
- `result.css`: examiner feedback pane, empty/loading/result states.
- `responsive.css`: tablet and mobile behavior.

## Web API

The example server is intentionally tiny and self-contained. It serves static files plus two JSON endpoints:

```text
GET  /api/health
POST /api/exam
```

`POST /api/exam` accepts:

```json
{
  "mode": "writing",
  "prompt": "Evaluate this IELTS Writing Task 2 response: ...",
  "max_turns": 2
}
```

It returns:

```json
{
  "mode": "writing",
  "model": "YOUR_MODEL_ID",
  "text": "### 1. Overall estimate ...",
  "session_log": "logs/ielts_examinator/session_YYYYMMDD_HHMMSS.json",
  "duration_ms": 10950
}
```

## Session Logs

Session logs are written as readable JSON by default:

```text
logs/ielts_examinator/session_YYYYMMDD_HHMMSS.json
```

Use `--log-dir PATH` to place logs elsewhere. The CLI prints the latest log path after a successful run, and the web UI displays it in the feedback pane.

Useful checks:

```bash
jq '{model, system_has_ielts:(.system_prompt|contains("IELTS practice examiner")), tool_calls:.stats.tool_calls}' \
  logs/ielts_examinator/session_*.json
```

Expected behavior:

- `system_has_ielts` is `true`.
- The default coding-agent prompt is absent.
- `tool_calls` is `0`.

## Verification

```bash
cargo check --example ielts_examinator --features full
cargo test --example ielts_examinator --features full
```

For the web UI, run the server and check the static routes:

```bash
python3 - <<'PY'
from urllib.request import urlopen
for path in [
    "/styles.css",
    "/css/tokens.css",
    "/css/base.css",
    "/css/shell.css",
    "/css/composer.css",
    "/css/result.css",
    "/css/responsive.css",
    "/app.js",
]:
    with urlopen(f"http://127.0.0.1:7878{path}", timeout=5) as res:
        print(path, res.status, res.headers.get("content-type"))
PY
```

## Troubleshooting

If the web page is blank, check browser console errors first. The React UI is loaded from an import map, so network access to the ESM CDN is required unless the assets are vendored locally later.

If the model request fails, verify the endpoint:

```bash
curl http://YOUR_OPENAI_COMPATIBLE_HOST:PORT/v1/models
```

If session logs do not appear, pass an explicit log directory:

```bash
--log-dir logs/ielts_examinator_debug
```
