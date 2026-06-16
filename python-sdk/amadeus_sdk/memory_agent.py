"""MemoryAgent — a persistent-memory agent built on the Amadeus Python SDK.

The MemoryAgent creates a dedicated agent session on the server, maintaining
conversation history across turns. Memory is injected into the system prompt
and the LLM can explicitly store/recall via the memory tool.
"""

from __future__ import annotations

import json
import time
from datetime import datetime, timezone
from pathlib import Path
from typing import Optional

from .agent import AgentTurn
from .client import AmadeusClient
from .compaction import CompactionManager
from .memory import MemoryManager
from .prompts import PromptBuilder
from .tools import ToolRegistry
from .types import (
    AgentChatResponse,
    AgentInfo,
    MemoryEntryInfo,
    ToolCall,
)
from .ua_debug import UADebugRecorder


class MemoryAgent:
    """A persistent-memory agent with conversation history and debug recording.

    Usage::

        async with MemoryAgent("http://localhost:3000") as agent:
            await agent.remember("project_db", "PostgreSQL on port 5432")
            turn = await agent.ask("What database does this project use?")
            print(turn.text)
            agent.save_debug_log("ua_debug_demo.json")
    """

    def __init__(
        self,
        base_url: str = "http://localhost:3000",
        timeout: float = 120.0,
        debug_log_dir: Optional[str] = None,
    ) -> None:
        self._base_url = base_url
        self._timeout = timeout
        self._agent_id: Optional[str] = None
        self._debug_log_dir = debug_log_dir
        self._debug = UADebugRecorder()

    # ------------------------------------------------------------------
    # Async context manager
    # ------------------------------------------------------------------

    async def __aenter__(self) -> "MemoryAgent":
        self._client = AmadeusClient(self._base_url, self._timeout)
        await self._client.__aenter__()

        # Create a dedicated agent session for scoped conversation
        info = await self._client.create_agent(name="memory_agent")
        self._agent_id = info.id

        # Capture model name for debug
        try:
            config = await self._client.get_config()
            self._debug.model = config.model
        except Exception:
            self._debug.model = "unknown"

        return self

    async def __aexit__(self, *args: object) -> None:
        try:
            # Save debug log if a directory was configured
            if self._debug_log_dir and self._debug.turns:
                ts = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%S")
                path = Path(self._debug_log_dir) / f"ua_debug_{ts}.json"
                self.save_debug_log(str(path))

            # Kill the agent session
            if self._agent_id:
                await self._client.kill_agent(self._agent_id)
        except Exception:
            pass
        finally:
            if hasattr(self, "_client"):
                await self._client.__aexit__(None, None, None)

    # ------------------------------------------------------------------
    # Sub-resources (lazy)
    # ------------------------------------------------------------------

    @property
    def client(self) -> AmadeusClient:
        return self._client

    @property
    def prompts(self) -> PromptBuilder:
        if not hasattr(self, "_prompts"):
            self._prompts = PromptBuilder(self._client)
        return self._prompts

    @property
    def tools(self) -> ToolRegistry:
        if not hasattr(self, "_tools"):
            self._tools = ToolRegistry(self._client)
        return self._tools

    @property
    def memory(self) -> MemoryManager:
        if not hasattr(self, "_memory"):
            self._memory = MemoryManager(self._client)
        return self._memory

    @property
    def compaction(self) -> CompactionManager:
        if not hasattr(self, "_compaction"):
            self._compaction = CompactionManager(self._client)
        return self._compaction

    @property
    def rag(self) -> "RAGManager":
        if not hasattr(self, "_rag"):
            from .rag import RAGManager
            self._rag = RAGManager(self._client)
        return self._rag

    @property
    def debug(self) -> UADebugRecorder:
        return self._debug

    @property
    def agent_id(self) -> Optional[str]:
        return self._agent_id

    # ------------------------------------------------------------------
    # Conversation
    # ------------------------------------------------------------------

    async def ask(self, prompt: str, timeout_secs: int = 300) -> AgentTurn:
        """Send a prompt to the agent session. History is maintained server-side."""
        if not self._agent_id:
            raise RuntimeError("Agent session not created. Use `async with MemoryAgent(...)`")

        t0 = time.monotonic()
        data = await self._client.agent_chat(self._agent_id, prompt, timeout_secs=timeout_secs)
        elapsed_ms = int((time.monotonic() - t0) * 1000)

        tool_calls = [tc.__dict__ if hasattr(tc, "__dict__") else tc for tc in data.tool_calls]

        # Record in debug
        self._debug.record_turn(
            user_msg=prompt,
            assistant_text=data.content,
            tool_calls=tool_calls,
            stop_reason=data.stop_reason,
        )

        return AgentTurn(
            text=data.content,
            tool_calls=tool_calls,
            stop_reason=data.stop_reason,
        )

    async def run(self, prompt: str, max_turns: int = 10) -> list[AgentTurn]:
        """Multi-turn ReAct loop.

        Each turn goes through the agent. The server handles tool execution
        internally (including the memory tool). Returns collected turns.
        """
        turns: list[AgentTurn] = []
        current = prompt
        for _ in range(max_turns):
            turn = await self.ask(current)
            turns.append(turn)
            if turn.stop_reason == "end_turn":
                break
            # The next "user" message is the tool results synthesized by server
            current = "(tool results processed)"
        return turns

    # ------------------------------------------------------------------
    # Memory operations (client-side convenience)
    # ------------------------------------------------------------------

    async def remember(self, key: str, content: str, source: str = "user") -> dict:
        """Store a persistent memory entry."""
        return await self._client.store_memory_entry(key, content, source)

    async def recall(self, key: str) -> Optional[str]:
        """Recall a memory entry by key."""
        entries = await self._client.load_memory_entries()
        for e in entries.entries:
            if e.key == key:
                return e.content
        return None

    async def search_memories(self, query: str) -> list[MemoryEntryInfo]:
        """Search memory entries by keyword."""
        entries = await self._client.load_memory_entries()
        q = query.lower()
        return [e for e in entries.entries if q in e.key.lower() or q in e.content.lower()]

    async def forget(self, key: str) -> dict:
        """Delete a memory entry by key."""
        return await self._client.delete_memory_entry(key)

    async def list_memories(self) -> list[MemoryEntryInfo]:
        """List all stored memory entries."""
        resp = await self._client.load_memory_entries()
        return resp.entries

    async def memory_context(self) -> str:
        """Build a textual memory context block for prompt injection."""
        return await self._memory.build_context_block()

    # ------------------------------------------------------------------
    # Session management
    # ------------------------------------------------------------------

    async def clear_history(self) -> None:
        """Kill and recreate the agent session for a fresh conversation slate."""
        if self._agent_id:
            await self._client.kill_agent(self._agent_id)
        info = await self._client.create_agent(name="memory_agent")
        self._agent_id = info.id

    async def health(self) -> str:
        """Quick health check."""
        h = await self._client.health()
        return h.status

    # ------------------------------------------------------------------
    # Debug
    # ------------------------------------------------------------------

    def save_debug_log(self, path: str) -> None:
        """Export the full UA conversation to a debug JSON file."""
        self._debug.save(path)

    @staticmethod
    def load_debug_log(path: str):
        """Load a UA debug JSON file for inspection."""
        return UADebugRecorder.load(path)

    def load_debug_seed(self, path: str) -> None:
        """Load a UA debug JSON and seed agent history + memory from it.

        This injects the conversation messages into the agent's history by
        replaying them through the chat endpoint, then stores all memory
        snapshot entries. Useful for reproducing a specific state for testing.
        """
        log = UADebugRecorder.load(path)

        # Store memory snapshot entries
        for entry in log.memory_snapshot:
            key = entry.get("key", "")
            content = entry.get("content", "")
            source = entry.get("source", "user")
            if key and content:
                # Fire-and-forget — best effort seeding
                try:
                    self._client.store_memory_entry(key, content, source)
                except Exception:
                    pass

        self._debug = UADebugRecorder()
        self._debug.model = log.model
        self._debug.system_prompt = log.system_prompt

    # ------------------------------------------------------------------
    # Summarize
    # ------------------------------------------------------------------

    async def summarize(self, text: str, prompt: Optional[str] = None) -> str:
        """Summarize text using the agent's LLM."""
        resp = await self._client.summarize(text, prompt=prompt)
        return resp.summary
