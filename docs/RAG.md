# RAG: Pluggable Embedding Backends, Local Baseline, and Quantized Storage

Functional indicator 4 (edge deployment + Galaxy Kylin embedding SDK +
lightweighting) is implemented as a software layer that works without the
Kylin hardware, with clean swap points for the real SDK.

## Architecture

```
                 ┌────────────────────────┐
                 │ Embedder (trait)       │   crates/rag/src/embedding.rs
                 │  name() / dimension()  │
                 │  embed() / embed_single│
                 └───────────┬────────────┘
        ┌────────────────────┼─────────────────────┐
        │                    │                     │
 EmbeddingClient      LocalHashEmbedder      KylinEmbedder
 "remote_http"        "local_hash"           "kylin_sdk"
 OpenAI-compatible    pure Rust, zero deps   Kylin SDK preset
 HTTP /v1/embeddings  FNV-1a feature hash    (local HTTP service)
                      CJK bigrams + words
```

Consumers (`RagTool`, the HTTP `/rag` handlers) hold `Arc<dyn Embedder>` —
the backend is chosen once at construction time by
`embedder_from_config(&EmbedderConfig)` and is interchangeable.

## Configuration

| Key | Default | Meaning |
|-----|---------|---------|
| `rag_enabled` | `false` | Register the `rag` tool in the agent loop (TUI `build_agent` and `spawn_new_session`) |
| `embedding_backend` | `"remote"` | `remote` \| `local` \| `kylin` (unknown values warn and fall back to `remote`) |
| `embedding_dimension` | `384` | Vector dimension for the `local` backend |
| `embedding_base_url` / `embedding_model` | falls back to `base_url` / `model` | Remote endpoint override |
| `kylin_embedding_endpoint` | `http://127.0.0.1:18080/v1` | Kylin SDK local service (`kylin` backend) |
| `rag_quantization` | `"none"` | `none` \| `int8` — int8 stores 4x smaller vectors |
| `rag_chunk_size` / `rag_chunk_overlap` / `rag_top_k` | chunker defaults | Chunking and retrieval depth |

### The `local` backend (edge baseline)

`LocalHashEmbedder` is a zero-dependency feature-hashing embedder: text is
tokenized into lowercase Latin words and CJK character bigrams, each token is
FNV-1a hashed, accumulated into a fixed-dimension vector (sign from a hash
bit), and L2-normalized. It is deterministic, offline, and microsecond-fast;
retrieval is lexical (shared terms), not semantic. It exists so the full
ingest → store → search pipeline runs on-device with no model file and no
network, and so CI can exercise the entire path.

### The `kylin` backend (Galaxy Kylin SDK)

`KylinEmbedder` is a deliberately thin preset over `EmbeddingClient` that
targets the Kylin AI SDK's local OpenAI-compatible HTTP service. When real
Kylin hardware arrives, the upgrade paths are:

1. **SDK exposes HTTP (expected)** — configuration-only: set
   `embedding_backend = "kylin"` and `kylin_embedding_endpoint`.
2. **SDK is a C FFI library** — rewrite only `KylinEmbedder`'s internals;
   the `Embedder` trait, `RagTool`, and HTTP API stay untouched.
3. **Different embedding dimension** — vectors are not comparable across
   models: delete `.amadeus/rag_index.json` and re-ingest. (Ingest rejects
   dimension mismatches explicitly instead of silently corrupting results.)

## Storage format (`.amadeus/rag_index.json`)

Versioned envelope, written in full on every mutation:

```json
{ "version": 2, "quantization": "int8", "dimension": 384, "entries": [...] }
```

- Dense entries store `embedding: Vec<f32>`.
- Quantized entries store `embedding_i8: Vec<i8>` + per-vector `scale` +
  precomputed `norm` (abs-max scalar quantization). Search quantizes the
  query the same way and scores with i32 dot products divided by the stored
  norms, keeping results on the cosine scale.
- Legacy v1 files (bare JSON arrays) are migrated transparently on load and
  rewritten as v2 on the next mutation.
- `VectorMemoryProvider::new(path)` is dense;
  `VectorMemoryProvider::with_quantization(path, Quantization::Int8)` is the
  lightweight mode. Mixed in-memory entries across a mode switch are handled
  per-entry.

## Agent wiring

When `rag_enabled` is true, the composition roots
(`src/main.rs::build_agent`, `crates/tui/src/ui/app.rs::spawn_new_session`)
register a `RagTool` built by `RagTool::open_in_workspace(workdir, config)`.
The vector store is deliberately **not** registered in the `MemoryRegistry`:
the registry injects all entries into the system prompt
(`build_memory_content`), which would dump every document chunk into every
turn. RAG entries are retrieved on demand through the `rag` tool instead.

The HTTP server (`run_server`) always builds the vector store and the
configured embedding backend for the `/rag/ingest`, `/rag/query`,
`/rag/documents` routes; their behavior is unchanged.

## Latency measurement (指标验收)

Measure end-to-end retrieval on the target device:

```rust
let start = std::time::Instant::now();
let results = store.search(&query_embedding, top_k);
let elapsed = start.elapsed();
```

Search is a linear scan (i32 dot products when quantized). At edge data
volumes (thousands of chunks × 384 dimensions) this is well under a
millisecond per query; an ANN index is intentionally not needed at this
scale.

## Tests

- `cargo test -p rag` — hash embedder properties, quantization round-trip,
  v1→v2 migration, dimension-mismatch rejection, f32/int8 top-k equivalence.
- `cargo test --test embedding_indicator_test --features full` — indicator-4
  acceptance: offline determinism, RagTool ingest→query end to end, int8
  persistence across restart, backend selection, agent registration.
