#!/usr/bin/env python3
"""End-to-end example using the Amadeus Python SDK.

Start the Amadeus server first:

    cargo run --features full -- --server 3000

Then run this example:

    python examples/basic_agent.py
"""

import asyncio
import sys
from pathlib import Path

# Allow running from the repo root or python-sdk/ directory
sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from amadeus_sdk import Agent


async def main() -> None:
    base_url = sys.argv[1] if len(sys.argv) > 1 else "http://localhost:3000"

    async with Agent(base_url) as agent:
        # 1. Health check
        print("=== Health Check ===")
        status = await agent.health()
        print(f"  Server: {status}")
        print()

        # 2. Inspect current config
        print("=== Configuration ===")
        config = await agent.client.get_config()
        print(f"  Model: {config.model}")
        print(f"  Workdir: {config.working_dir}")
        print(f"  Context window: {config.context_window_size}")
        print()

        # 3. Tool catalog
        print("=== Tool Catalog ===")
        tools = await agent.tools.list_tools()
        for t in tools:
            print(f"  {t.name} [{t.permission_mode}] — {t.description[:80]}")
        print()

        # 4. Prompt sections
        print("=== System Prompt Sections ===")
        sections = await agent.prompts.list_current_sections()
        for s in sections:
            print(f"  [{s.priority:3}] {s.id:30s} {'dynamic' if s.dynamic else 'static'}")
        print()

        # 5. Memory entries
        print("=== Memory ===")
        providers = await agent.memory.list_providers()
        for p in providers:
            print(f"  Provider: {p.name} (writable={p.writable}, entries={p.entry_count})")
        entries = await agent.memory.load_entries()
        for e in entries:
            print(f"  {e.source}/{e.key}: {e.content[:100]}...")
        print()

        # 6. Compaction config
        print("=== Compaction Config ===")
        comp = await agent.compaction.get_config()
        print(f"  Auto: {comp.auto_compact}")
        print(f"  Threshold: {comp.threshold_percent}%")
        print(f"  Min messages: {comp.min_messages}")
        triggers = await agent.compaction.get_triggers()
        print(f"  Available triggers: {triggers.available}")
        print()

        # 7. Build a custom prompt
        print("=== Custom System Prompt ===")
        prompt_builder = agent.prompts
        prompt_builder.add_section(
            "custom_role",
            "You are a helpful assistant that always responds in haiku format.",
            priority=15,
        )
        built = await prompt_builder.build()
        print(f"  Sections: {built.section_count}")
        print(f"  Prompt length: {len(built.prompt)} chars")
        print(f"  Preview (first 300 chars): {built.prompt[:300]}...")
        print()

        # 8. Send a chat message
        print("=== Chat ===")
        turn = await agent.send("What is 2 + 2? Answer concisely.")
        print(f"  Response: {turn.text}")
        print(f"  Tool calls: {len(turn.tool_calls)}")
        print()

        # 9. Direct command execution
        print("=== Execute ===")
        output = await agent.execute("echo 'Hello from Amadeus SDK!'")
        print(f"  Output: {output.strip()}")
        print()

        print("All checks complete.")


if __name__ == "__main__":
    asyncio.run(main())
