# MemoryAgent Architecture

## Overview

MemoryAgent is a persistent-memory AI agent system. It combines two retrieval paradigms:

- **Key-value memory** — the LLM explicitly stores and recalls facts by key (via `memory` tool)
- **Semantic RAG** — text is chunked, embedded, and retrieved by meaning (via `rag` tool)

Both are available to the LLM as tools, exposed as REST endpoints, and wrapped in a Python SDK.

```
┌─────────────────────────────────────────────────┐
│                    Python SDK                   │
│  MemoryAgent  ─┬─ MemoryManager (CRUD)          │
│                └─ RAGManager    (ingest/query)  │
└──────────────────────┬──────────────────────────┘
                       │ HTTP (httpx)
┌──────────────────────▼──────────────────────────┐
│                Axum HTTP API                    │
│  /memory/providers  /memory/entries             │
│  /rag/ingest        /rag/query                  │
│  /rag/documents     /rag/documents/:id          │
└──────────────────────┬──────────────────────────┘
                       │
┌──────────────────────▼──────────────────────────┐
│                  AppState                       │
│  memory_provider: Arc<JsonFileMemoryProvider>   │
│  rag_provider:    Arc<VectorMemoryProvider>     │
│  embedding_client: Arc<EmbeddingClient>         │
└──────┬──────────────────────┬───────────────────┘
       │                      │
       ▼                      ▼
┌──────────────┐    ┌─────────────────────────────┐
│  MemoryTool  │    │  RagTool                    │
│  (core)      │    │  (rag crate)                │
│              │    │                             │
│  store       │    │  ingest (text/file/url)      │
│  recall      │    │  query  (semantic search)   │
│  search      │    │  list_documents             │
│  list        │    │  delete_document            │
│  delete      │    │                             │
└──────┬───────┘    └──────┬──────────────────────┘
       │                   │
       ▼                   ▼
┌────────────────┐    ┌──────────────────────────────┐
│ MemoryRegistry │    │  VectorMemoryProvider        │
│ (context)      │    │  (rag crate)                 │
│                │    │                              │
│ providers[]    │    │  entries: Mutex<Vec<         │
│  ├─ JsonFile   │    │    VectorEntry {             │
│  │  Provider   │    │      entry: MemoryEntry,     │
│  └─ File       │    │      embedding: Vec<f32>,    │
│     Provider   │    │      metadata: ChunkMetadata │
│                │    │    }                         │
│                │    │  >                           │
└──────┬─────────┘    │  search(embedding, top_k)    │
       │              │  cosine similarity           │
       ▼              └──────┬───────────────────────┘
┌──────────────┐             │
│ .amadeus/    │             ▼
│ memory.json  │    ┌─────────────────────────────┐
└──────────────┘    │  EmbeddingClient            │
                    │  POST /v1/embeddings        │
                    │  { model, input: [str] }    │
                    └──────┬──────────────────────┘
                           │
                           ▼
                    ┌──────────────┐
                    │ .amadeus/    │
                    │ rag_index.   │
                    │ json         │
                    └──────────────┘
```

## Rust Crates

| Crate | Purpose | Key types |
|-------|---------|-----------|
| `context` | Memory trait + JSON persistence | `MemoryProvider`, `MemoryEntry`, `MemoryRegistry`, `JsonFileMemoryProvider` |
| `core` | Agent loop, LLM clients, tool system | `Agent`, `AgentBuilder`, `Tool`, `ToolRegistry`, `MemoryTool` |
| `rag` | Embedding + chunking + vector store | `EmbeddingClient`, `VectorMemoryProvider`, `RagTool`, `chunk_text()` |
| `api` | Axum HTTP server + handlers | `AppState`, handlers for `/memory/*`, `/rag/*` |
| `config` | Layered configuration | `Config` with RAG/memory fields |

Dependency direction: `rag` → `core` (for `Tool` trait). `core` does NOT depend on `rag` — instead, `RagTool` is passed via `AgentBuilder::with_rag(Box<dyn Tool>)` to avoid a circular dependency.

## MemoryProvider Trait

Defined in `crates/context/src/memory.rs`. All memory storage backends implement this.

```rust
pub trait MemoryProvider: Send + Sync + fmt::Debug {
    fn name(&self) -> &'static str;
    fn load(&self) -> Vec<MemoryEntry>;
    fn store(&self, entry: MemoryEntry) -> Result<(), MemoryError> { /* read-only by default */ }
    fn delete(&self, key: &str) -> Result<(), MemoryError> { /* read-only by default */ }
    fn writable(&self) -> bool { false }
}
```

`name()` and `load()` are required. `store()` and `delete()` default to `WriteFailed` error. `writable()` defaults to `false`.

### MemoryEntry

```rust
pub struct MemoryEntry {
    pub key: String,
    pub content: String,
    pub source: String,  // "user", "llm", "file", "session", "compaction", "dynamic"
}
```

### MemoryRegistry

A collector that holds multiple providers and merges their results:

```rust
pub struct MemoryRegistry {
    providers: Vec<Arc<dyn MemoryProvider>>,
}
```

- `register(provider)` — add a backend
- `load_all() -> Vec<MemoryEntry>` — merge entries from all providers
- `build_memory_content() -> Option<String>` — render entries as `## key\n\ncontent` (capped at 8000 chars), newest first. Injected into the LLM system prompt as `## Persistent Memory`.

## Memory Backends

### JsonFileMemoryProvider

Location: `crates/context/src/memory_json.rs`
Storage: `.amadeus/memory.json`

- `name()` → `"json_file"`
- `writable()` → `true`
- Thread safety: `Mutex<Vec<MemoryEntry>>`
- Auto-creates file on first write. Flushes to disk after every `store()` or `delete()`.
- File format: JSON array of `{key, content, source}` objects.

### VectorMemoryProvider

Location: `crates/rag/src/vector_store.rs`
Storage: `.amadeus/rag_index.json`

Also implements `MemoryProvider`, but only the `load()` path is used by the memory system. Its primary API is consumed by `RagTool`.

Internal structure:
```rust
struct VectorEntry {
    entry: MemoryEntry,       // key = "rag:{doc_id}:chunk_{i}", source = "rag:{doc_id}"
    embedding: Vec<f32>,      // from EmbeddingClient
    metadata: ChunkMetadata,  // document_id, chunk_index, original_path, ingested_at
}
```

Key methods (outside the trait):
- `ingest_chunks(doc_id, path, chunks, embeddings) -> Result<usize>`
- `search(query_embedding, top_k) -> Vec<(MemoryEntry, f32)>` — cosine similarity
- `list_documents() -> Vec<DocumentInfo>` — grouped by document_id
- `delete_document(doc_id) -> Result<usize>` — removes all chunks for a document

### FileMemoryProvider

Location: `crates/context/src/memory.rs`
Read-only. Loads content from `CLAUDE.md`, `CONTEXT.md`, or `.amadeus/context.md` — presented as memory entries with source `"file"`.

## Tool System

### MemoryTool

Location: `crates/core/src/tools/memory_tool.rs`
Tool name: `"memory"`
Permission: `ReadOnly`

Registered in `AgentBuilder::build()` via:
```rust
let shared = Arc::new(std::sync::RwLock::new(memory_registry.clone()));
tools.register(Box::new(MemoryTool::new(shared)));
```

Operations:

| Operation | Input | Action |
|-----------|-------|--------|
| `store` | `key`, `content` | Writes to all writable providers with source `"llm"` |
| `recall` | `key` | Exact key match across all providers |
| `search` | `query` | Case-insensitive substring match on keys and content |
| `list` | — | Lists all entry keys |
| `delete` | `key` | Removes from all writable providers |

### RagTool

Location: `crates/rag/src/tool.rs`
Tool name: `"rag"`
Permission: `ReadOnly`

Passed into `AgentBuilder` externally (avoids `core` → `rag` circular dep):
```rust
AgentBuilder::with_rag(Box::new(RagTool::new(store, embedder, chunk_size, overlap, top_k)))
```

Operations:

| Operation | Input | Action |
|-----------|-------|--------|
| `ingest` | `path` / `url` / `text`, optional `document_id`, `chunk_size`, `chunk_overlap` | Read source → `chunk_text()` → `embedder.embed()` → `store.ingest_chunks()` |
| `query` | `query_text`, optional `top_k` | `embedder.embed_single()` → `store.search()` → scored results |
| `list_documents` | — | Calls `store.list_documents()` |
| `delete_document` | `document_id` | Calls `store.delete_document()` |

## EmbeddingClient

Location: `crates/rag/src/embedding.rs`

OpenAI-compatible `/v1/embeddings` client:

```
POST {base_url}/embeddings
Authorization: Bearer {api_key}
Content-Type: application/json

{
  "model": "BAAI/bge-small-en-v1.5",
  "input": ["text chunk 1", "text chunk 2", ...]
}
```

Response: `{"data": [{"embedding": [f32, ...]}, ...]}`

- Batches inputs in groups of 32
- Uses `Client::builder().no_proxy()` to bypass macOS system proxy
- 60s request timeout, 10s connect timeout
- Respects `AMADEUS_NO_PROXY` env var

## Chunker

Location: `crates/rag/src/chunker.rs`

```rust
pub fn chunk_text(text: &str, chunk_size: usize, overlap: usize) -> Vec<String>
```

- Sliding window, character-based (no tokenizer)
- Prefers natural breaks near chunk boundaries: `\n\n` > `. ` > `\n` > ` `
- Defaults: 1200 chars per chunk, 200 overlap (~300 tokens)

## Config

Location: `crates/config/src/lib.rs`

```rust
pub struct Config {
    // RAG
    pub rag_enabled: bool,              // default: false
    pub embedding_model: Option<String>, // default: None (falls back to model)
    pub embedding_base_url: Option<String>, // default: None (falls back to base_url)
    pub rag_chunk_size: usize,          // default: 1200
    pub rag_chunk_overlap: usize,       // default: 200
    pub rag_top_k: usize,              // default: 5

    // paths
    pub workdir: PathBuf,               // all file paths relative to this
}
```

Layered config: global `~/.amadeus/settings.json` → workspace `.amadeus/settings.json` → `.amadeus/settings.local.json`.

Example workspace config:
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

## HTTP API

All routes are mounted on the Axum server.

### Memory endpoints

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/memory/providers` | List registered providers with entry counts |
| `GET` | `/memory/entries` | Load all memory entries |
| `POST` | `/memory/entries` | Store a memory entry `{key, content, source}` |
| `DELETE` | `/memory/entries/:key` | Delete a memory entry |

### RAG endpoints

| Method | Path | Description |
|--------|------|-------------|
| `POST` | `/rag/ingest` | Ingest text → chunk → embed → store. Body: `{text?, path?, document_id?, chunk_size?, chunk_overlap?}` |
| `POST` | `/rag/query` | Semantic search. Body: `{query, top_k?}` |
| `GET` | `/rag/documents` | List ingested documents with chunk counts |
| `DELETE` | `/rag/documents/:id` | Delete all chunks for a document |

### AppState

```rust
pub struct AppState<C: LLMClient + Clone + 'static> {
    pub client: C,
    pub config: Arc<Config>,
    pub orchestrator: Arc<RwLock<AgentOrchestrator<C>>>,
    pub memory_provider: Arc<JsonFileMemoryProvider>,
    pub rag_provider: Arc<VectorMemoryProvider>,
    pub embedding_client: Arc<EmbeddingClient>,
    // ...
}
```

Initialization in `run_server()`:
1. Create `JsonFileMemoryProvider` at `{workdir}/.amadeus/memory.json` → register in `MemoryRegistry`
2. Create `VectorMemoryProvider` at `{workdir}/.amadeus/rag_index.json`
3. Create `EmbeddingClient` from config (`embedding_base_url` or fallback to `base_url`, `embedding_model` or fallback to `model`)
4. Build `RagTool` from provider + embedder + config → inject into `AgentBuilder::with_rag()`
5. `MemoryRegistry` injected into `AgentBuilder::with_memory_registry()`

## Python SDK

Package: `amadeus_sdk` (at `python-sdk/amadeus_sdk/`)

### MemoryAgent

```python
class MemoryAgent:
    # Constructor
    def __init__(self, base_url="http://localhost:3000", timeout=120.0,
                 debug_log_dir=None) -> None

    # Lazy sub-resources
    agent.memory      # MemoryManager
    agent.rag         # RAGManager
    agent.prompts     # PromptBuilder
    agent.tools       # ToolRegistry
    agent.compaction  # CompactionManager

    # Core API
    async def ask(prompt, timeout_secs=300) -> AgentTurn  # single turn
    async def run(prompt, max_turns=10) -> list[AgentTurn] # multi-turn ReAct

    # Convenience methods
    async def remember(key, content, source="user") -> dict
    async def recall(key) -> Optional[str]
    async def search_memories(query) -> list[MemoryEntryInfo]
    async def forget(key) -> dict
    async def list_memories() -> list[MemoryEntryInfo]
    async def memory_context() -> str

    # Session
    async def clear_history() -> None
    async def health() -> str
    async def summarize(text, prompt=None) -> str

    # Debug
    def save_debug_log(path) -> None
    static def load_debug_log(path) -> UADebugRecorder
    def load_debug_seed(path) -> None  # replay debug log into agent
```

### RAGManager

```python
class RAGManager:
    async def ingest_text(text, document_id=None, chunk_size=None,
                          chunk_overlap=None) -> RagIngestResponse
    async def query(query, top_k=5) -> list[RagSearchResult]
    async def list_documents() -> list[RagDocumentInfo]
    async def delete_document(document_id) -> dict
```

### Data types

- `RagSearchResult` — `{rank, key, content, source, score: float}`
- `RagIngestResponse` — `{document_id, chunk_count}`
- `RagDocumentInfo` — `{id, chunk_count, ingested_at}`
- `MemoryEntryInfo` — `{key, content, source}`
- `AgentTurn` — `{text, tool_calls, stop_reason}`

### UA Debug

`UADebugRecorder` captures full conversation traces: user messages, assistant responses, tool calls, stop reasons, memory snapshots, system prompt, and model name. Written as JSON for replay and seeding.

## Data Flow: LLM calling memory tools

```
1. User sends prompt via /chat or Python agent.ask()
2. LLM responds with a tool_use block (e.g. memory.store or rag.query)
3. Agent loop extracts tool call → ToolRegistry.execute("memory", input)
4. MemoryTool.execute() dispatches to the right operation
5. Store path:
   MemoryTool → MemoryRegistry → JsonFileMemoryProvider.store()
   → Mutex<Vec<MemoryEntry>>::push → flush to .amadeus/memory.json
6. Next turn: memory_registry.build_memory_content()
   → "## Persistent Memory\n\n## key\ncontent" appended to system prompt
```

```
RAG ingest flow:
1. LLM calls rag.ingest with text/path/url
2. RagTool.do_ingest() → chunk_text() → EmbeddingClient.embed()
3. VectorMemoryProvider.ingest_chunks() → stores chunks + embeddings
4. Persisted to .amadeus/rag_index.json

RAG query flow:
1. LLM calls rag.query with query_text
2. RagTool.do_query() → EmbeddingClient.embed_single()
3. VectorMemoryProvider.search() → cosine similarity → top-k results
4. LLM receives scored, relevant chunks as context
```

## File Layout

```
.amadeus/
  memory.json      ← JsonFileMemoryProvider persistence
  rag_index.json   ← VectorMemoryProvider persistence
  settings.json    ← workspace config
  settings.local.json ← local overrides (gitignored)

crates/
  context/src/
    memory.rs       ← MemoryProvider trait, MemoryEntry, MemoryRegistry
    memory_json.rs  ← JsonFileMemoryProvider
  core/src/
    tools/
      memory_tool.rs ← MemoryTool (LLM-callable)
      registry.rs    ← ToolRegistry (permissions, catalog)
    agent/
      loop_agent.rs  ← AgentBuilder (wiring: with_memory_registry, with_rag)
  rag/src/
    embedding.rs    ← EmbeddingClient
    chunker.rs      ← chunk_text()
    vector_store.rs ← VectorMemoryProvider
    tool.rs         ← RagTool
  config/src/
    lib.rs          ← Config (rag_enabled, embedding_model, etc.)
  api/src/api/
    http.rs         ← AppState, run_server, router
    handlers/
      memory.rs     ← /memory/* handlers
      rag.rs        ← /rag/* handlers

python-sdk/amadeus_sdk/
  memory_agent.py   ← MemoryAgent class
  rag.py            ← RAGManager class
  client.py         ← AmadeusClient (HTTP methods)
  types.py          ← Dataclass types
  memory.py         ← MemoryManager class

runtime/
  rag_eval/
    rag_eval_runner.py ← RAG retrieval quality eval
  locomo/
    memory_agent_runner.py ← MemoryAgent LoCoMo eval

scripts/
  embedding_proxy.py ← Local fastembed embedding server (port 1113)
```

## Embedding Proxy

`scripts/embedding_proxy.py` — standalone Python server providing OpenAI-compatible `/v1/embeddings` using fastembed (BAAI/bge-small-en-v1.5, 384-dim). Used when the LLM server does not serve embeddings.

```bash
python scripts/embedding_proxy.py --port 1113 --model BAAI/bge-small-en-v1.5
```

## Eval Results (LoCoMo, 10 sessions, 189 QA pairs)

LLM-based QA from retrieved chunks vs. full-conversation oracle:

| Category | RAG Accuracy | Full Accuracy | Retention |
|----------|-------------|---------------|-----------|
| Single-Hop (n=96) | 60.4% | 72.9% | 82.9% |
| Multi-Hop (n=40) | 25.0% | 35.0% | 71.4% |
| Temporal (n=41) | 12.2% | 41.5% | 29.4% |
| Open Domain (n=12) | 8.3% | 8.3% | 100% |
| **Overall** | **39.2%** | **54.0%** | **72.5%** |

With only 5 chunks (~6% of the full conversation), RAG preserves 72.5% of the full-context QA capability. Single-hop questions work well; temporal reasoning suffers most from chunk boundaries scattering date references.

## Key Design Decisions

1. **Trait-based storage**: `MemoryProvider` in `context` is the single abstraction. Both `JsonFileMemoryProvider` and `VectorMemoryProvider` implement it. This allows the memory system to treat key-value and vector backends uniformly.

2. **Circular dependency avoidance**: `rag` depends on `core` (for `Tool` trait). `core` does NOT depend on `rag`. `RagTool` is passed into `AgentBuilder` as `Box<dyn Tool>` — a dependency inversion via trait object.

3. **`std::sync::Mutex` over `tokio::sync::Mutex`**: Both `JsonFileMemoryProvider` and `VectorMemoryProvider` use `std::sync::Mutex` because lock hold times are short (in-memory Vec operations) and the code never holds the lock across `.await` points.

4. **Write-through persistence**: Both providers flush to disk on every write. Simple and correct for the current scale. A future optimization could batch writes.

5. **EmbeddingClient is standalone**: Not behind a trait — it's a concrete type shared via `Arc`. If alternative embedding backends are needed, a trait abstraction would slot in naturally at the `ApiState` / `RagTool` boundary.

6. **Python SDK mirrors Rust architecture**: `MemoryManager` / `RAGManager` / `MemoryAgent` follow the same patterns as their Rust counterparts, delegating via HTTP.
