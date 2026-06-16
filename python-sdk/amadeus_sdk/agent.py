"""Simple agent loop using the Amadeus HTTP API.

Provides a minimalist ReAct-style agent that:
1. Sends a user prompt
2. Receives the response (text + tool calls)
3. Displays the result

For a full agent loop with streaming, use the server's SSE endpoints.
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Optional

from .client import AmadeusClient
from .compaction import CompactionManager
from .memory import MemoryManager
from .prompts import PromptBuilder
from .tools import ToolRegistry
from .types import ChatResponse


@dataclass
class AgentTurn:
    """Result of a single agent turn."""
    text: str
    tool_calls: list[dict]
    stop_reason: str


class Agent:
    """High-level agent that wraps the Amadeus REST API.

    Usage::

        async with Agent("http://localhost:3000") as agent:
            # Inspect available tools
            tools = await agent.tools.list_tools()

            # Check memory
            entries = await agent.memory.load_entries()

            # Send a prompt
            turn = await agent.send("What files are in the src directory?")
            print(turn.text)
    """

    def __init__(self, base_url: str = "http://localhost:3000", timeout: float = 120.0) -> None:
        self._base_url = base_url
        self._timeout = timeout

    async def __aenter__(self) -> "Agent":
        self._client = AmadeusClient(self._base_url, self._timeout)
        await self._client.__aenter__()
        return self

    async def __aexit__(self, *args: object) -> None:
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

    # ------------------------------------------------------------------
    # Core operations
    # ------------------------------------------------------------------

    async def send(self, message: str, timeout_secs: int = 300) -> AgentTurn:
        """Send a message and return the agent's turn."""
        resp: ChatResponse = await self._client.chat(message, timeout_secs=timeout_secs)
        return AgentTurn(
            text=resp.content,
            tool_calls=[tc.__dict__ for tc in resp.tool_calls],
            stop_reason=resp.stop_reason,
        )

    async def execute(self, command: str, timeout_secs: int = 30) -> str:
        """Execute a bash command directly (no LLM involvement)."""
        resp = await self._client.execute(command, timeout_secs=timeout_secs)
        return resp.output

    async def health(self) -> str:
        """Quick health check."""
        h = await self._client.health()
        return h.status

    async def summarize(self, text: str, prompt: Optional[str] = None) -> str:
        """Summarize text using the agent's LLM."""
        resp = await self._client.summarize(text, prompt=prompt)
        return resp.summary

    # ------------------------------------------------------------------
    # Session helpers
    # ------------------------------------------------------------------

    async def list_sessions(self):
        """List saved sessions."""
        return await self._client.list_sessions()

    async def restore_session(self, session_id: str, clear: bool = False):
        """Restore a saved session."""
        return await self._client.restore_session(session_id, clear_history=clear)
