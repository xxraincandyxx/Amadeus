#!/usr/bin/env python3
"""MemoryAgent example — persistent memory with conversation recording.

Start the Amadeus server first:

    cargo run --features full -- --server 3000

Then run this example:

    python examples/memory_agent.py [http://localhost:3000]
"""

import asyncio
import sys
from pathlib import Path

from amadeus_sdk import MemoryAgent, UADebugRecorder


async def main() -> None:
    base_url = sys.argv[1] if len(sys.argv) > 1 else "http://localhost:3000"

    async with MemoryAgent(base_url, debug_log_dir="./debug_logs") as agent:
        # 1. Health check
        print("=== Health ===")
        status = await agent.health()
        print(f"  Server status: {status}")
        print()

        # 2. Inspect config
        print("=== Config ===")
        cfg = await agent.client.get_config()
        print(f"  Model: {cfg.model}")
        print(f"  Working dir: {cfg.working_dir}")
        print()

        # 3. Store some persistent memories
        print("=== Storing Memories ===")
        await agent.remember("project_name", "Amadeus — Rust AI agent SDK")
        print('  Stored "project_name"')
        await agent.remember("database", "PostgreSQL 16 on port 5432, host db.internal")
        print('  Stored "database"')
        await agent.remember(
            "auth_provider",
            "OAuth2 via Keycloak, realm=prod, client_id=amadeus-app",
        )
        print('  Stored "auth_provider"')
        print()

        # 4. List all memories
        print("=== All Memories ===")
        entries = await agent.list_memories()
        for e in entries:
            print(f"  [{e.source}] {e.key}: {e.content[:80]}")
        print()

        # 5. Search memories
        print("=== Search 'database' ===")
        results = await agent.search_memories("database")
        for e in results:
            print(f"  {e.key}: {e.content}")
        print()

        # 6. Recall a specific memory
        print("=== Recall 'auth_provider' ===")
        content = await agent.recall("auth_provider")
        print(f"  {content}")
        print()

        # 7. Chat — LLM can see memory in system prompt and use memory tool
        print("=== Chat: What database does this project use? ===")
        turn = await agent.ask("What database does this project use?")
        print(f"  Response: {turn.text}")
        print(f"  Tool calls: {len(turn.tool_calls)}")
        print(f"  Stop reason: {turn.stop_reason}")
        print()

        # 8. Memory context block (for manual prompt injection)
        print("=== Memory Context Block (preview) ===")
        ctx = await agent.memory_context()
        print(ctx[:500])
        print()

        # 9. Inspect tools
        print("=== Available Tools ===")
        tools = await agent.tools.list_tools()
        for t in tools:
            mark = " [MEM]" if t.name == "memory" else ""
            print(f"  {t.name}{mark}: {t.description[:80]}")
        print()

        # 10. Save debug log
        debug_path = "./ua_debug_demo.json"
        agent.save_debug_log(debug_path)
        print(f"=== Debug log saved to {debug_path} ===")
        file_size = Path(debug_path).stat().st_size
        print(f"  Size: {file_size} bytes")
        print()

        # 11. Load debug log back
        print("=== Load debug log ===")
        log = MemoryAgent.load_debug_log(debug_path)
        print(f"  Version: {log.version}")
        print(f"  Model: {log.model}")
        print(f"  Turns: {len(log.turns)}")
        print(f"  Stats: {log.stats}")
        for t in log.turns:
            print(f"  Turn {t.index}: user='{t.user[:50]}' -> assistant='{t.assistant.text[:50]}'")
        print()

        # 12. Extract messages from debug log
        print("=== Extract messages from debug log ===")
        msgs = UADebugRecorder.load_as_messages(debug_path)
        for m in msgs:
            print(f"  [{m['role']}] {m['content'][:80]}")
        print()

        # 13. Forget a memory
        print("=== Forget 'auth_provider' ===")
        result = await agent.forget("auth_provider")
        print(f"  Result: {result}")

        remaining = await agent.list_memories()
        print(f"  Remaining entries: {len(remaining)}")
        for e in remaining:
            print(f"    {e.key}")
        print()

        # 14. Clear conversation history
        await agent.clear_history()
        print("=== History cleared (fresh agent session) ===")
        print()

        print("All MemoryAgent operations complete.")


if __name__ == "__main__":
    asyncio.run(main())
