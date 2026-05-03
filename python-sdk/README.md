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
| `amadeus_sdk` | `MemoryAgent` | Persistent-memory agent with conversation recording |
| `amadeus_sdk` | `AmadeusClient` | Low-level HTTP client |
| `amadeus_sdk` | `UADebugRecorder` | Record/replay UA conversations to JSON |
| `amadeus_sdk.prompts` | `PromptBuilder` | Build custom system prompts |
| `amadeus_sdk.tools` | `ToolRegistry` | Inspect tool catalog |
| `amadeus_sdk.memory` | `MemoryManager` | Manage memory providers |
| `amadeus_sdk.compaction` | `CompactionManager` | Compaction configuration |

## MemoryAgent

The `MemoryAgent` provides persistent memory across sessions with conversation
recording and replay. Memory is injected into the system prompt and the LLM can
explicitly store/recall via the `memory` tool.

```python
import asyncio
from amadeus_sdk import MemoryAgent, UADebugRecorder

async def main():
    async with MemoryAgent("http://localhost:3000", debug_log_dir="./logs") as agent:
        # Store persistent memories
        await agent.remember("project_db", "PostgreSQL on port 5432")
        await agent.remember("api_url", "https://api.example.com/v2")

        # The LLM sees memory in its system prompt and can use the memory tool
        turn = await agent.ask("What database does this project use?")
        print(turn.text)  # "We use PostgreSQL running on port 5432."

        # Search memories client-side
        results = await agent.search_memories("database")
        for e in results:
            print(f"{e.key}: {e.content}")

        # Forget a memory
        await agent.forget("api_url")

        # Export conversation to a debug JSON file
        agent.save_debug_log("ua_debug_demo.json")

        # Load a debug log for inspection
        log = MemoryAgent.load_debug_log("ua_debug_demo.json")
        for t in log.turns:
            print(f"Turn {t.index}: {t.user} -> {t.assistant.text}")

        # Seed agent from a debug log (restore memory + history)
        agent.load_debug_seed("previous_session.json")

        # Clear conversation history (fresh slate)
        await agent.clear_history()

asyncio.run(main())
```

### Memory Operations

| Method | Description |
|--------|-------------|
| `remember(key, content)` | Store a persistent memory entry |
| `recall(key)` | Recall a memory entry by key |
| `search_memories(query)` | Search memory entries by keyword |
| `list_memories()` | List all stored memory entries |
| `forget(key)` | Delete a memory entry by key |
| `memory_context()` | Build a textual context block for prompts |

### Debug Recording

The `UADebugRecorder` captures full User-Assistant dialogue as structured JSON:

```python
recorder = UADebugRecorder()
recorder.model = "claude-sonnet-4-6"
recorder.system_prompt = "You are a helpful assistant."
recorder.record_turn(
    user_msg="What is 2+2?",
    assistant_text="2+2 = 4",
    tool_calls=[{"name": "bash", "input": {"cmd": "echo $((2+2))"}, "output": "4"}],
    stop_reason="end_turn",
)
recorder.save("ua_debug.json")

# Load and extract messages for test fixtures
messages = UADebugRecorder.load_as_messages("ua_debug.json")
# [{"role": "user", "content": "What is 2+2?"}, {"role": "assistant", "content": "2+2 = 4"}]
```

### UA Debug JSON Format

```json
{
  "version": "1",
  "timestamp": "2026-05-03T10:30:00Z",
  "model": "claude-sonnet-4-6",
  "system_prompt": "You are a helpful assistant.",
  "memory_snapshot": [
    {"key": "project_db", "content": "PostgreSQL on port 5432", "source": "user"}
  ],
  "turns": [
    {
      "index": 0,
      "user": "What database do we use?",
      "assistant": {
        "text": "We use PostgreSQL running on port 5432.",
        "tool_calls": [
          {
            "name": "memory",
            "input": {"operation": "recall", "key": "project_db"},
            "output": "PostgreSQL on port 5432"
          }
        ],
        "stop_reason": "end_turn"
      }
    }
  ],
  "stats": {
    "turns": 1,
    "tool_calls": 1
  }
}
```

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
- `GET /memory/providers`, `GET /memory/entries`, `POST /memory/entries`, `DELETE /memory/entries/:key`
- `GET /tools/catalog`
- `GET /agents`, `POST /agents`, `GET /agents/:id`, `DELETE /agents/:id`, `POST /agents/:id/switch`, `POST /agents/:id/chat`
- `GET /approvals`, `POST /approvals/:id`
