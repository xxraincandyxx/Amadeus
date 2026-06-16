"""Type definitions mirroring the Amadeus REST API responses."""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Optional


# ---------------------------------------------------------------------------
# Chat
# ---------------------------------------------------------------------------

@dataclass
class ToolCall:
    name: str
    input: dict
    output: str


@dataclass
class ChatResponse:
    content: str
    tool_calls: list[ToolCall] = field(default_factory=list)
    stop_reason: str = "end_turn"


# ---------------------------------------------------------------------------
# Execute
# ---------------------------------------------------------------------------

@dataclass
class ExecuteResponse:
    output: str
    exit_code: int
    timed_out: bool


# ---------------------------------------------------------------------------
# Health
# ---------------------------------------------------------------------------

@dataclass
class HealthResponse:
    status: str
    version: str


# ---------------------------------------------------------------------------
# Config
# ---------------------------------------------------------------------------

@dataclass
class ConfigResponse:
    working_dir: str
    model: str
    max_tokens: int
    context_window_size: int
    tool_timeout_secs: int
    require_approval: bool
    shell_profile: Optional[str] = None
    session_log_dir: Optional[str] = None


# ---------------------------------------------------------------------------
# Compaction
# ---------------------------------------------------------------------------

@dataclass
class CompactionConfig:
    auto_compact: bool = True
    threshold_percent: int = 75
    target_percent: int = 50
    preserve_recent: int = 10
    use_llm_summary: bool = True
    max_summary_chars: int = 2000
    min_messages: int = 4
    max_tool_result_chars: int = 5000
    active_trigger: str = "threshold"


@dataclass
class CompactionTriggers:
    available: list[str] = field(default_factory=list)
    active: str = "threshold"


# ---------------------------------------------------------------------------
# Prompts
# ---------------------------------------------------------------------------

@dataclass
class PromptSectionInfo:
    id: str
    title: str
    priority: int
    dynamic: bool
    content_preview: str


@dataclass
class PromptSectionsResponse:
    sections: list[PromptSectionInfo] = field(default_factory=list)


@dataclass
class PromptSectionInput:
    id: str
    content: str
    priority: Optional[int] = None


@dataclass
class BuildPromptResponse:
    prompt: str
    section_count: int


# ---------------------------------------------------------------------------
# Memory
# ---------------------------------------------------------------------------

@dataclass
class MemoryProviderInfo:
    name: str
    writable: bool
    entry_count: int


@dataclass
class MemoryEntryInfo:
    key: str
    content: str
    source: str


@dataclass
class MemoryProvidersResponse:
    providers: list[MemoryProviderInfo] = field(default_factory=list)


@dataclass
class MemoryEntriesResponse:
    entries: list[MemoryEntryInfo] = field(default_factory=list)


@dataclass
class StoreMemoryRequest:
    key: str
    content: str
    source: str = "user"


# ---------------------------------------------------------------------------
# Tools
# ---------------------------------------------------------------------------

@dataclass
class ToolCatalogEntry:
    name: str
    description: str
    permission_mode: str
    level: str


@dataclass
class ToolCatalogResponse:
    tools: list[ToolCatalogEntry] = field(default_factory=list)


# ---------------------------------------------------------------------------
# Session
# ---------------------------------------------------------------------------

@dataclass
class SessionSummary:
    id: str
    timestamp: str
    model: str
    total_tokens: int = 0
    tool_calls: int = 0
    duration_ms: int = 0
    message_count: int = 0
    todo_count: int = 0


@dataclass
class MessageSummary:
    role: str
    content: str


@dataclass
class SessionDetail:
    id: str
    timestamp: str
    model: str
    system_prompt: str = ""
    history: list[MessageSummary] = field(default_factory=list)
    todos: list[dict] = field(default_factory=list)
    stats: dict = field(default_factory=dict)


# ---------------------------------------------------------------------------
# Skills
# ---------------------------------------------------------------------------

@dataclass
class SkillSummary:
    name: str
    description: str


# ---------------------------------------------------------------------------
# Approvals
# ---------------------------------------------------------------------------

@dataclass
class ApprovalResponse:
    success: bool
    decision: str


# ---------------------------------------------------------------------------
# Agent (multi-agent)
# ---------------------------------------------------------------------------

@dataclass
class AgentInfo:
    id: str
    name: str
    profile: str
    status: str
    task_count: int = 0


@dataclass
class AgentChatResponse:
    content: str
    tool_calls: list[ToolCall] = field(default_factory=list)
    stop_reason: str = "end_turn"


# ---------------------------------------------------------------------------
# Summarize
# ---------------------------------------------------------------------------

@dataclass
class SummarizeResponse:
    summary: str
    mechanism: str
    prompt_used: Optional[str] = None


# ---------------------------------------------------------------------------
# UA Debug
# ---------------------------------------------------------------------------

@dataclass
class UADebugToolCall:
    name: str
    input: dict
    output: str


@dataclass
class UADebugAssistant:
    text: str
    tool_calls: list[UADebugToolCall] = field(default_factory=list)
    stop_reason: str = "end_turn"


@dataclass
class UADebugTurn:
    index: int
    user: str
    assistant: UADebugAssistant


@dataclass
class UADebugLog:
    version: str = "1"
    timestamp: str = ""
    model: str = ""
    system_prompt: str = ""
    memory_snapshot: list[dict] = field(default_factory=list)
    turns: list[UADebugTurn] = field(default_factory=list)
    stats: dict = field(default_factory=dict)


# ---------------------------------------------------------------------------
# RAG
# ---------------------------------------------------------------------------

@dataclass
class RagSearchResult:
    rank: int
    key: str
    content: str
    source: str
    score: float


@dataclass
class RagDocumentInfo:
    id: str
    chunk_count: int
    ingested_at: str


@dataclass
class RagIngestResponse:
    document_id: str
    chunk_count: int


@dataclass
class RagQueryResponse:
    query: str
    results: list[RagSearchResult] = field(default_factory=list)


@dataclass
class RagDocumentsResponse:
    documents: list[RagDocumentInfo] = field(default_factory=list)


# ---------------------------------------------------------------------------
# Error
# ---------------------------------------------------------------------------

@dataclass
class ErrorResponse:
    error: str
    message: str
    tool: Optional[str] = None
    retry_after: Optional[int] = None
