# Amadeus Python SDK

Async Python client for the [Amadeus](https://github.com/amadeus) AI agent framework.

## Quick Start

Start the Amadeus server:

```bash
cargo run --features full -- --server 3000
```

Use the SDK:

```python
import asyncio
from amadeus_sdk import Agent

async def main():
    async with Agent("http://localhost:3000") as agent:
        # Health check
        print(await agent.health())

        # Inspect tools
        tools = await agent.tools.list_tools()
        for t in tools:
            print(f"  {t.name}: {t.description}")

        # Send a prompt
        turn = await agent.send("What is the current directory?")
        print(turn.text)

asyncio.run(main())
```

Run the example:

```bash
python examples/basic_agent.py
```

## Modules

| Module | Class | Purpose |
|--------|-------|---------|
| `amadeus_sdk` | `Agent` | High-level agent wrapper |
| `amadeus_sdk` | `AmadeusClient` | Low-level HTTP client |
| `amadeus_sdk.prompts` | `PromptBuilder` | Build custom system prompts |
| `amadeus_sdk.tools` | `ToolRegistry` | Inspect tool catalog |
| `amadeus_sdk.memory` | `MemoryManager` | Manage memory providers |
| `amadeus_sdk.compaction` | `CompactionManager` | Compaction configuration |

## API Coverage

- `GET /health` — health check
- `POST /chat` — send chat messages
- `POST /execute` — direct bash execution
- `GET /config`, `PATCH /config` — configuration
- `GET /sessions`, `GET /sessions/:id`, `POST /sessions/:id/restore` — sessions
- `GET /history` — conversation history
- `GET /skills` — available skills
- `POST /summarize` — text summarization
- `GET /compaction/config`, `PATCH /compaction/config`, `GET /compaction/triggers`
- `GET /prompts/sections`, `POST /prompts/build`
- `GET /memory/providers`, `GET /memory/entries`
- `GET /tools/catalog`
- `GET /agents`, `POST /agents`, `GET /agents/:id`, `DELETE /agents/:id`, `POST /agents/:id/switch`, `POST /agents/:id/chat`
- `GET /approvals`, `POST /approvals/:id`
