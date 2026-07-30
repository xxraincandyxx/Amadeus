"""Async HTTP client for the Amadeus REST API."""

from __future__ import annotations

import json
from typing import AsyncIterator, Optional, Union
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
    RagDocumentInfo,
    RagDocumentsResponse,
    RagIngestResponse,
    RagQueryResponse,
    RagSearchResult,
    SessionDetail,
    LiveSession,
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
        if body is not None:
            resp = await self.client.request("DELETE", path, content=json.dumps(body), headers={"Content-Type": "application/json"})
        else:
            resp = await self.client.delete(path)
        return self._handle(resp)

    def _handle(self, resp: httpx.Response) -> dict:
        if resp.is_success:
            data = resp.json()
            # Some endpoints return 200 with error payloads on agent failures
            if isinstance(data, dict) and "error" in data:
                err = ErrorResponse(**data)
                raise AmadeusError(resp.status_code, err)
            return data
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

    async def list_sessions(self) -> list[LiveSession]:
        """List live external sessions."""
        data = await self._get("/v1/sessions")
        return [LiveSession(**s) for s in data.get("sessions", [])]

    async def create_session(self, name: Optional[str] = None, profile: str = "default") -> LiveSession:
        """Create a live external session."""
        body: dict = {"profile": profile}
        if name is not None:
            body["name"] = name
        return LiveSession(**await self._post("/v1/sessions", body))

    async def get_session(self, session_id: str) -> LiveSession:
        """Get live session metadata."""
        return LiveSession(**await self._get(f"/v1/sessions/{session_id}"))

    async def close_session(self, session_id: str) -> dict:
        """Close a live session and abort any active turn."""
        return await self._delete(f"/v1/sessions/{session_id}")

    async def submit_message(self, session_id: str, content: str) -> dict:
        """Start an asynchronous turn in a live session."""
        return await self._post(f"/v1/sessions/{session_id}/messages", {"content": content})

    async def iter_events(self, session_id: str) -> AsyncIterator[dict]:
        """Yield parsed server-sent events for a live session."""
        async with self.client.stream("GET", f"/v1/sessions/{session_id}/events") as response:
            response.raise_for_status()
            event_name = "message"
            data_lines: list[str] = []
            async for line in response.aiter_lines():
                if line.startswith("event:"):
                    event_name = line[6:].strip()
                elif line.startswith("data:"):
                    data_lines.append(line[5:].strip())
                elif not line and data_lines:
                    yield {"event": event_name, "data": json.loads("\n".join(data_lines))}
                    event_name = "message"
                    data_lines = []

    async def cancel_session(self, session_id: str) -> dict:
        """Cancel an active turn without closing its session."""
        return await self._post(f"/v1/sessions/{session_id}/cancel")

    async def get_checkpoint(self, session_id: str) -> dict:
        """Capture a serializable live-session checkpoint."""
        return await self._get(f"/v1/sessions/{session_id}/checkpoint")

    async def restore_checkpoint(self, session_id: str, checkpoint: dict) -> dict:
        """Restore a checkpoint into a live session."""
        resp = await self.client.put(f"/v1/sessions/{session_id}/checkpoint", json=checkpoint)
        return self._handle(resp)

    async def list_saved_sessions(self) -> list[SessionSummary]:
        """List persisted session-log archives."""
        data = await self._get("/sessions")
        return [SessionSummary(**s) for s in data.get("sessions", [])]

    async def get_saved_session(self, session_id: str) -> SessionDetail:
        """Get a persisted session-log archive."""
        return SessionDetail(**await self._get(f"/sessions/{session_id}"))

    # ------------------------------------------------------------------
    # History
    # ------------------------------------------------------------------

    async def get_history(self, session_id: str) -> list[dict]:
        """Get the current history of a live session."""
        data = await self._get(f"/v1/sessions/{session_id}/history")
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
    # RAG
    # ------------------------------------------------------------------

    async def rag_ingest(
        self,
        text: Optional[str] = None,
        path: Optional[str] = None,
        document_id: Optional[str] = None,
        chunk_size: Optional[int] = None,
        chunk_overlap: Optional[int] = None,
    ) -> RagIngestResponse:
        """Ingest text or a file into the RAG vector store."""
        body: dict = {}
        if text is not None:
            body["text"] = text
        if path is not None:
            body["path"] = path
        if document_id is not None:
            body["document_id"] = document_id
        if chunk_size is not None:
            body["chunk_size"] = chunk_size
        if chunk_overlap is not None:
            body["chunk_overlap"] = chunk_overlap
        return RagIngestResponse(**await self._post("/rag/ingest", body))

    async def rag_query(self, query: str, top_k: Optional[int] = None) -> RagQueryResponse:
        """Semantic search over ingested documents."""
        body: dict = {"query": query}
        if top_k is not None:
            body["top_k"] = top_k
        data = await self._post("/rag/query", body)
        data["results"] = [RagSearchResult(**r) for r in data.get("results", [])]
        return RagQueryResponse(**data)

    async def rag_list_documents(self) -> RagDocumentsResponse:
        """List all ingested RAG documents."""
        data = await self._get("/rag/documents")
        data["documents"] = [RagDocumentInfo(**d) for d in data.get("documents", [])]
        return RagDocumentsResponse(**data)

    async def rag_delete_document(self, document_id: str) -> dict:
        """Delete a RAG document and all its chunks."""
        return await self._delete(f"/rag/documents/{document_id}")

    # ------------------------------------------------------------------
    # Multi-agent
    # ------------------------------------------------------------------

    async def list_agents(self) -> list[AgentInfo]:
        """List all agents."""
        sessions = await self.list_sessions()
        return [AgentInfo(id=s.id, name=s.name, profile=s.profile, status=s.status) for s in sessions]

    async def create_agent(self, name: Optional[str] = None, profile: str = "default") -> AgentInfo:
        """Create a new agent."""
        session = await self.create_session(name, profile)
        return AgentInfo(id=session.id, name=session.name, profile=session.profile, status=session.status)

    async def get_agent(self, agent_id: str) -> AgentInfo:
        """Get agent details."""
        session = await self.get_session(agent_id)
        return AgentInfo(id=session.id, name=session.name, profile=session.profile, status=session.status)

    async def kill_agent(self, agent_id: str) -> dict:
        """Kill (remove) an agent."""
        return await self.close_session(agent_id)

    async def switch_agent(self, agent_id: str) -> dict:
        """Switch to a different agent as the active one."""
        raise AmadeusError(410, ErrorResponse(error="Removed", message="Active-session switching is client-local in v1"))

    async def agent_chat(self, agent_id: str, message: str, timeout_secs: int = 300) -> AgentChatResponse:
        """Chat with a specific agent."""
        await self.submit_message(agent_id, message)
        raise AmadeusError(202, ErrorResponse(error="AsyncOnly", message="Consume /v1/sessions/{id}/events for results"))

    # ------------------------------------------------------------------
    # Approvals
    # ------------------------------------------------------------------

    async def list_pending_approvals(self, session_id: str) -> list[dict]:
        """List pending tool approvals."""
        return await self._get(f"/v1/sessions/{session_id}/approvals")

    async def submit_approval(self, session_id: str, approval_id: str, decision: str) -> ApprovalResponse:
        """Submit an approval decision (approve/deny/modify)."""
        data = await self._post(f"/v1/sessions/{session_id}/approvals/{approval_id}", {"decision": decision})
        return ApprovalResponse(success=data["success"], decision=decision)
