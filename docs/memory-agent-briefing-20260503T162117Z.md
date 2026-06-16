# MemoryAgent Briefing — 2026-05-03

## Overview

MemoryAgent is a persistent-memory AI agent system with semantic retrieval (RAG), LLM-callable memory tools, and multi-provider support. It spans a Rust server (Axum + Tokio) and a Python SDK (httpx + asyncio), with a modular RAG crate for embedding-based retrieval.

## Rust Architecture

### Crates

| Crate | Purpose | Key Types |
|-------|---------|-----------|
| `context` | Memory trait and JSON persistence | `MemoryProvider`, `MemoryEntry`, `JsonFileMemoryProvider` |
| `core` | Agent loop, LLM clients, tools | `Agent`, `AgentBuilder`, `LLMClient`, `Tool`, `ToolRegistry` |
| `rag` | RAG retrieval | `EmbeddingClient`, `VectorMemoryProvider`, `RagTool`, chunker |
| `api` | HTTP API (Axum) | `AppState`, handlers (`/rag/*`, `/memory/*`, `/chat`) |
| `config` | Configuration | `Config` (with RAG fields via `apply_json`/`merge`) |

### Memory System

**`MemoryProvider` trait** (`crates/context/src/memory.rs`):
- `name() -> &str` — provider identifier
- `load() -> Vec<MemoryEntry>` — load all entries
- `store(key, content, source, metadata) -> MemoryEntry` — upsert
- `delete(key) -> bool` — remove by key
- `writable() -> bool` — whether writes are supported

**`JsonFileMemoryProvider`**: Persists to `.amadeus/memory.json`. Thread-safe with `Arc<Mutex<HashMap>>`. Auto-creates file on first write.

**`MemoryTool`** (`crates/core/src/tools/memory.rs`): LLM-callable tool with operations:
- `remember(key, content, source?)` — store a fact
- `recall(key)` — retrieve by key
- `forget(key)` — delete an entry
- `search(query)` — substring match across keys and content

Permissions: `ReadOnly` in ToolRegistry (alongside `read_file`, `glob`, `grep`, `web_fetch`, `rag`, `todo`).

### RAG System

**`EmbeddingClient`** (`crates/rag/src/embedding.rs`):
- OpenAI-compatible `/v1/embeddings` client
- Batches inputs into groups of 32
- Uses `Client::builder().no_proxy()` to avoid macOS system proxy interference
- 60s request timeout, 10s connect timeout

**`VectorMemoryProvider`** (`crates/rag/src/vector_store.rs`):
- Implements `MemoryProvider` trait (name: `"vector_rag"`, writable: true)
- Persists to `.amadeus/rag_index.json`
- Stores entries with embeddings as `VectorEntry { entry: MemoryEntry, embedding: Vec<f32>, metadata: ChunkMetadata }`
- Key methods: `ingest_chunks()`, `search(query_embedding, top_k) -> Vec<(MemoryEntry, f32)>`, `list_documents()`, `delete_document()`
- Cosine similarity computed in pure Rust (no ML deps)

**`RagTool`** (`crates/rag/src/tool.rs`):
- Implements `Tool` trait — callable by LLM
- 4 operations: `ingest` (from text/file/URL), `query` (semantic search), `list_documents`, `delete_document`
- Schema via `OnceLock<Value>`

**Chunker** (`crates/rag/src/chunker.rs`):
- Sliding window over text, character-based
- Prefers natural breaks: `\n\n` > `. ` > `\n` > ` `
- Defaults: 1200 chars/chunk, 200 overlap

### Config

Six RAG fields in `Config` (`crates/config/src/lib.rs`):

| Field | Default | JSON Key |
|-------|---------|----------|
| `rag_enabled` | `false` | `"rag_enabled"` |
| `embedding_model` | same as `model` | `"embedding_model"` |
| `embedding_base_url` | same as `base_url` | `"embedding_base_url"` |
| `rag_chunk_size` | `1200` | `"rag_chunk_size"` |
| `rag_chunk_overlap` | `200` | `"rag_chunk_overlap"` |
| `rag_top_k` | `5` | `"rag_top_k"` |

### HTTP Endpoints

RAG routes on the Axum server:

| Method | Path | Handler |
|--------|------|---------|
| `POST` | `/rag/ingest` | `rag_ingest` |
| `POST` | `/rag/query` | `rag_query` |
| `GET` | `/rag/documents` | `rag_list_documents` |
| `DELETE` | `/rag/documents/:id` | `rag_delete_document` |

Memory write endpoints also available at `/memory/*`.

## Python SDK

Package: `amadeus_sdk` (at `python-sdk/amadeus_sdk/`)

### `MemoryAgent` class

Async context manager that creates a dedicated agent session on the server:

```python
async with MemoryAgent("http://localhost:3000") as agent:
    await agent.remember("project_db", "PostgreSQL 16 on port 5432")
    turn = await agent.ask("What database does this project use?")
    print(turn.text)
    agent.save_debug_log("debug.json")
```

Key properties (all lazy-loaded):
- `agent.memory` → `MemoryManager` (CRUD for memory entries)
- `agent.rag` → `RAGManager` (ingest/query/delete documents, semantic search)
- `agent.prompts` → `PromptBuilder`
- `agent.tools` → `ToolRegistry`
- `agent.compaction` → `CompactionManager`

Key methods:
- `ask(prompt)` → `AgentTurn` — single turn with tool calls
- `run(prompt, max_turns=10)` → `list[AgentTurn]` — multi-turn ReAct loop
- `remember(key, content)` — store memory entry
- `recall(key)` → `str | None` — recall by key
- `clear_history()` — kill/recreate agent session
- `save_debug_log(path)` / `load_debug_log(path)` / `load_debug_seed(path)` — UA debug recording

### `RAGManager` class

```python
agent.rag.ingest_text(text, document_id="my-doc")    # → RagIngestResponse
agent.rag.query("semantic query", top_k=5)           # → list[RagSearchResult]
agent.rag.list_documents()                            # → list[RagDocumentInfo]
agent.rag.delete_document("my-doc")                   # → dict
```

### UA Debug Recorder

`UADebugRecorder` captures full conversation traces: user messages, assistant responses, tool calls, stop reasons, memory snapshots, system prompt, and model name. Saves to JSON for replay and seeding.

## RAG Eval

**Runner**: `runtime/rag_eval/rag_eval_runner.py`

**Methodology**: LLM-based QA from retrieved chunks, scored against LoCoMo gold answers using openbench's F1 heuristics. Compares RAG (top-5 chunks) vs. full-conversation oracle.

**Latest results** (10 LoCoMo sessions, 189 QA pairs, gemma-4-26b):

| Metric | Score |
|--------|-------|
| RAG accuracy | **39.2%** |
| Full-context accuracy | **54.0%** |
| Retrieval retention | **72.5%** |
| Avg ingest time | 5.85s |
| Avg query time | 0.02s |
| Avg LLM time | 1.08s |

**By category**:

| Category | RAG Acc | Full Acc | Retention |
|----------|---------|----------|-----------|
| Single-Hop (n=96) | 60.4% | 72.9% | 82.9% |
| Multi-Hop (n=40) | 25.0% | 35.0% | 71.4% |
| Temporal (n=41) | 12.2% | 41.5% | 29.4% |
| Open Domain (n=12) | 8.3% | 8.3% | 100% |

## Embedding Proxy

`scripts/embedding_proxy.py` — lightweight OpenAI-compatible `/v1/embeddings` server using fastembed (BAAI/bge-small-en-v1.5, 384-dim). Run on port 1113 when the LLM server doesn't serve embeddings.

## Known Issues

- **LoCoMo ingest failures**: 2/10 sessions fail with "Server disconnected without sending a response" — likely need increased timeout or retry on large embeds (45-75 chunks)
- **Temporal QA**: Retrieval retention only 29.4% — dates are scattered across chunks, chunk boundaries break temporal reasoning
- **Multi-Hop QA**: 71.4% retention but only 25% absolute — some answers require synthesizing facts from 3+ chunks
- **Embedding model**: bge-small-en-v1.5 (384-dim) is the smallest fastembed model; larger models (bge-base, bge-large) may improve retrieval quality

## Configuration Example

`.amadeus/settings.json`:
```json
{
  "provider": "openai",
  "base_url": "http://localhost:1112/v1",
  "model": "gemma-4-26b-a4b-it-fp8",
  "rag_enabled": true,
  "embedding_model": "BAAI/bge-small-en-v1.5",
  "embedding_base_url": "http://localhost:1113/v1"
}
```
