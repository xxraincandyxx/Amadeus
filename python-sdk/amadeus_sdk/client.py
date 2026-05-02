"""Async HTTP client for the Amadeus REST API."""

from __future__ import annotations

from typing import Optional, Union
from urllib.parse import urljoin

import httpx

from .types import (
    AgentChatResponse,
    AgentInfo,
    ApprovalResponse,
    BuildPromptResponse,
    ChatResponse,
    CompactionConfig,
    CompactionTriggers,
    ConfigResponse,
    ErrorResponse,
    ExecuteResponse,
    HealthResponse,
    MemoryEntriesResponse,
    MemoryEntryInfo,
    MemoryProviderInfo,
    MemoryProvidersResponse,
    PromptSectionInfo,
    PromptSectionInput,
    PromptSectionsResponse,
    SessionDetail,
    SessionSummary,
    SkillSummary,
    SummarizeResponse,
    ToolCall,
    ToolCatalogEntry,
    ToolCatalogResponse,
)


class AmadeusError(Exception):
    """Raised when the API returns an error response."""

    def __init__(self, status_code: int, error: ErrorResponse) -> None:
        self.status_code = status_code
        self.error = error
        super().__init__(f"[{status_code}] {error.error}: {error.message}")


class AmadeusClient:
    """Async HTTP client for the Amadeus agent API.

    Usage::

        async with AmadeusClient("http://localhost:3000") as client:
            health = await client.health()
            resp = await client.chat("List files in the project")
    """

    def __init__(self, base_url: str = "http://localhost:3000", timeout: float = 120.0) -> None:
        self.base_url = base_url.rstrip("/")
        self.timeout = timeout
        self._client: Optional[httpx.AsyncClient] = None

    @property
    def client(self) -> httpx.AsyncClient:
        if self._client is None:
            raise RuntimeError("Use AmadeusClient as an async context manager: `async with AmadeusClient(...)`")
        return self._client

    async def __aenter__(self) -> "AmadeusClient":
        self._client = httpx.AsyncClient(timeout=self.timeout, base_url=self.base_url, trust_env=False)
        return self

    async def __aexit__(self, *args: object) -> None:
        if self._client:
            await self._client.aclose()
            self._client = None

    # ------------------------------------------------------------------
    # Helpers
    # ------------------------------------------------------------------

    async def _get(self, path: str) -> dict:
        resp = await self.client.get(path)
        return self._handle(resp)

    async def _post(self, path: str, body: Optional[dict] = None) -> dict:
        resp = await self.client.post(path, json=body)
        return self._handle(resp)

    async def _patch(self, path: str, body: Optional[dict] = None) -> dict:
        resp = await self.client.patch(path, json=body)
        return self._handle(resp)

    async def _delete(self, path: str, body: Optional[dict] = None) -> dict:
        resp = await self.client.delete(path, json=body)
        return self._handle(resp)

    def _handle(self, resp: httpx.Response) -> dict:
        if resp.is_success:
            return resp.json()
        try:
            err = ErrorResponse(**resp.json())
        except Exception:
            err = ErrorResponse(error="Unknown", message=resp.text)
        raise AmadeusError(resp.status_code, err)

    # ------------------------------------------------------------------
    # Health
    # ------------------------------------------------------------------

    async def health(self) -> HealthResponse:
        """Check server health."""
        return HealthResponse(**await self._get("/health"))

    # ------------------------------------------------------------------
    # Chat
    # ------------------------------------------------------------------

    async def chat(self, message: str, timeout_secs: int = 300, stream: bool = False) -> ChatResponse:
        """Send a stateless chat message to the agent."""
        data = await self._post("/chat", {"message": message, "timeout_secs": timeout_secs, "stream": stream})
        data["tool_calls"] = [ToolCall(**tc) for tc in data.get("tool_calls", [])]
        return ChatResponse(**data)

    # ------------------------------------------------------------------
    # Execute
    # ------------------------------------------------------------------

    async def execute(self, command: str, timeout_secs: int = 30) -> ExecuteResponse:
        """Execute a bash command directly."""
        return ExecuteResponse(**await self._post("/execute", {"command": command, "timeout_secs": timeout_secs}))

    # ------------------------------------------------------------------
    # Config
    # ------------------------------------------------------------------

    async def get_config(self) -> ConfigResponse:
        """Get current server configuration."""
        return ConfigResponse(**await self._get("/config"))

    async def update_config(self, **kwargs: Union[str, int, bool, None]) -> ConfigResponse:
        """Update configuration. Returns the merged result."""
        resp = await self._patch("/config", {k: v for k, v in kwargs.items() if v is not None})
        return ConfigResponse(**resp["config"])

    # ------------------------------------------------------------------
    # Sessions
    # ------------------------------------------------------------------

    async def list_sessions(self) -> list[SessionSummary]:
        """List saved conversation sessions."""
        data = await self._get("/sessions")
        return [SessionSummary(**s) for s in data.get("sessions", [])]

    async def get_session(self, session_id: str) -> SessionDetail:
        """Get full session details."""
        return SessionDetail(**await self._get(f"/sessions/{session_id}"))

    async def restore_session(self, session_id: str, clear_history: bool = False) -> dict:
        """Restore a session into the active context."""
        return await self._post(f"/sessions/{session_id}/restore", {"clear_history": clear_history})

    # ------------------------------------------------------------------
    # History
    # ------------------------------------------------------------------

    async def get_history(self) -> list[dict]:
        """Get conversation history."""
        data = await self._get("/history")
        return data.get("messages", [])

    # ------------------------------------------------------------------
    # Skills
    # ------------------------------------------------------------------

    async def list_skills(self) -> list[SkillSummary]:
        """List available skills."""
        data = await self._get("/skills")
        return [SkillSummary(**s) for s in data.get("skills", [])]

    # ------------------------------------------------------------------
    # Summarize
    # ------------------------------------------------------------------

    async def summarize(
        self, text: str, prompt: Optional[str] = None, mechanism: str = "llm", max_chars: int = 2000
    ) -> SummarizeResponse:
        """Summarize text using the agent's LLM."""
        return SummarizeResponse(
            **await self._post("/summarize", {
                "text": text, "prompt": prompt, "mechanism": mechanism, "max_summary_chars": max_chars
            })
        )

    # ------------------------------------------------------------------
    # Compaction
    # ------------------------------------------------------------------

    async def get_compaction_config(self) -> CompactionConfig:
        """Get current compaction configuration."""
        return CompactionConfig(**await self._get("/compaction/config"))

    async def update_compaction_config(self, **kwargs: Union[bool, int, str, None]) -> CompactionConfig:
        """Update compaction configuration."""
        body = {k: v for k, v in kwargs.items() if v is not None}
        return CompactionConfig(**await self._patch("/compaction/config", body))

    async def get_compaction_triggers(self) -> CompactionTriggers:
        """List available compaction triggers."""
        return CompactionTriggers(**await self._get("/compaction/triggers"))

    # ------------------------------------------------------------------
    # Prompts
    # ------------------------------------------------------------------

    async def list_prompt_sections(self) -> PromptSectionsResponse:
        """List current system prompt sections."""
        data = await self._get("/prompts/sections")
        data["sections"] = [PromptSectionInfo(**s) for s in data.get("sections", [])]
        return PromptSectionsResponse(**data)

    async def build_prompt(
        self,
        workdir: Optional[str] = None,
        include_sub_agent: bool = True,
        extra_sections: Optional[list[PromptSectionInput]] = None,
    ) -> BuildPromptResponse:
        """Build a custom system prompt."""
        body: dict = {"include_sub_agent_tool": include_sub_agent}
        if workdir is not None:
            body["workdir"] = workdir
        if extra_sections is not None:
            body["extra_sections"] = [
                {"id": s.id, "content": s.content, "priority": s.priority}
                for s in extra_sections
            ]
        return BuildPromptResponse(**await self._post("/prompts/build", body))

    # ------------------------------------------------------------------
    # Memory
    # ------------------------------------------------------------------

    async def list_memory_providers(self) -> MemoryProvidersResponse:
        """List registered memory providers."""
        data = await self._get("/memory/providers")
        data["providers"] = [MemoryProviderInfo(**p) for p in data.get("providers", [])]
        return MemoryProvidersResponse(**data)

    async def load_memory_entries(self) -> MemoryEntriesResponse:
        """Load all memory entries from all providers."""
        data = await self._get("/memory/entries")
        data["entries"] = [MemoryEntryInfo(**e) for e in data.get("entries", [])]
        return MemoryEntriesResponse(**data)

    async def store_memory_entry(self, key: str, content: str, source: str = "user") -> dict:
        """Store a memory entry. Returns the API response."""
        return await self._post("/memory/entries", {"key": key, "content": content, "source": source})

    async def delete_memory_entry(self, key: str) -> dict:
        """Delete a memory entry by key."""
        return await self._delete(f"/memory/entries/{key}")

    # ------------------------------------------------------------------
    # Tools
    # ------------------------------------------------------------------

    async def get_tool_catalog(self) -> ToolCatalogResponse:
        """Get the full tool catalog."""
        data = await self._get("/tools/catalog")
        data["tools"] = [ToolCatalogEntry(**t) for t in data.get("tools", [])]
        return ToolCatalogResponse(**data)

    # ------------------------------------------------------------------
    # Multi-agent
    # ------------------------------------------------------------------

    async def list_agents(self) -> list[AgentInfo]:
        """List all agents."""
        data = await self._get("/agents")
        return [AgentInfo(**a) for a in data.get("agents", [])]

    async def create_agent(self, name: Optional[str] = None, profile: str = "default") -> AgentInfo:
        """Create a new agent."""
        body: dict = {"profile": profile}
        if name:
            body["name"] = name
        data = await self._post("/agents", body)
        return AgentInfo(**data["agent"])

    async def get_agent(self, agent_id: str) -> AgentInfo:
        """Get agent details."""
        return AgentInfo(**await self._get(f"/agents/{agent_id}"))

    async def kill_agent(self, agent_id: str) -> dict:
        """Kill (remove) an agent."""
        return await self._delete(f"/agents/{agent_id}")

    async def switch_agent(self, agent_id: str) -> dict:
        """Switch to a different agent as the active one."""
        return await self._post(f"/agents/{agent_id}/switch", {"agent_id": agent_id})

    async def agent_chat(self, agent_id: str, message: str, timeout_secs: int = 300) -> AgentChatResponse:
        """Chat with a specific agent."""
        data = await self._post(f"/agents/{agent_id}/chat", {"message": message, "timeout_secs": timeout_secs})
        data["tool_calls"] = [ToolCall(**tc) for tc in data.get("tool_calls", [])]
        return AgentChatResponse(**data)

    # ------------------------------------------------------------------
    # Approvals
    # ------------------------------------------------------------------

    async def list_pending_approvals(self) -> list[dict]:
        """List pending tool approvals."""
        return await self._get("/approvals")

    async def submit_approval(self, approval_id: str, decision: str, reason: Optional[str] = None) -> ApprovalResponse:
        """Submit an approval decision (approve/deny/modify)."""
        body: dict = {"decision": decision}
        if reason:
            body["reason"] = reason
        return ApprovalResponse(**await self._post(f"/approvals/{approval_id}", body))
