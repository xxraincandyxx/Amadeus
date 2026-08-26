# Memory System: Short-Term, Mid-Term, Long-Term

> Status: the mid-term crate (`crates/memory`, package `amadeus_memory`) is
> implemented and tested but **not yet wired** into the agent loop, bridge,
> or HTTP API. This document is the contract for its interfaces and storage
> format, plus the planned integration points.

## The Three Tiers

| Tier | What it is | Where it lives | Lifetime |
|------|------------|----------------|----------|
| Short-term | The live context window: conversation history, session logs, compaction summaries | `Agent.history` (`crates/core`), `amadeus_compaction`, session JSON logs | One session / until compaction |
| **Mid-term** | **A record database of what the conversation established: tasks, decisions, files touched, errors and resolutions, state snapshots** | **`crates/memory` (this crate)** | Across sessions, on disk |
| Long-term | Durable user/LLM-stored facts and semantic RAG | `JsonFileMemoryProvider` (`.amadeus/memory.json`), `VectorMemoryProvider` (`.amadeus/rag_index.json`) | Permanent |

The mid tier is filled by a **gate**: a transformer that runs over context
that is leaving the short-term window (typically at compaction time) and
decides what survives, as what kind of record, with what importance.

```
 context (short-term)                      mid-term DB                     long-term
┌─────────────────────┐   ContextGate   ┌──────────────────────┐   future   ┌──────────────────┐
│ Agent.history       │ ──────────────▶ │ JsonMidTermStore     │ ─────────▶ │ memory.json / RAG │
│ compaction summary  │  RuleBasedGate  │ .amadeus/            │ promotion  │ providers         │
│ session logs        │                 │ mid_term_memory.json │            │                  │
└─────────────────────┘                 └──────────────────────┘            └──────────────────┘
```

## Crate Layout

```
crates/memory/src/
  lib.rs      — public API re-exports + usage example
  record.rs   — MemoryKind, MemoryRecord          (WHAT is stored)
  store.rs    — MidTermStore trait, MemoryQuery, JsonMidTermStore  (WHERE/HOW)
  gate.rs     — ContextGate trait, GateInput, RuleBasedGate        (WHAT GETS THROUGH)
```

## 1. What is stored — `MemoryRecord`

The record model mirrors the compaction summarization strategy in
`docs/COMPACTION.md` (key tasks/objectives, decisions, files modified,
errors and resolutions, current state):

```rust
pub enum MemoryKind { Task, Decision, FileState, ErrorResolution, StateSnapshot, Fact }

pub struct MemoryRecord {
    pub id: String,                      // "mt:<session>:<slug>" — stable identity
    pub key: String,                      // upsert key; equal keys replace each other
    pub kind: MemoryKind,
    pub content: String,
    pub session_id: Option<String>,       // provenance
    pub source_message_indices: [usize; 2], // context message range it came from
    pub created_at: u64,                  // unix epoch seconds
    pub updated_at: u64,
    pub access_count: u32,
    pub importance: u8,                   // 0–100, gate-assigned; drives retention
}
```

`MemoryRecord` converts to/from the flat `MemoryEntry`
(`amadeus_context::memory`) with `source = "mid_term:<kind>"`, so any
existing `MemoryRegistry` consumer can read mid-term records unchanged.

## 2. The gate — `ContextGate`

```rust
pub trait ContextGate: Send + Sync + Debug {
    fn name(&self) -> &'static str;
    fn transform(&self, input: &GateInput<'_>) -> Result<Vec<MemoryRecord>, MemoryError>;
}

pub struct GateInput<'a> {
    pub session_id: Option<&'a str>,
    pub messages: &'a [Message],               // context being retired/compacted
    pub compaction: Option<&'a CompactionResult>, // summary → StateSnapshot
}
```

`RuleBasedGate` is the deterministic, LLM-free implementation:

| Source in context | Record kind | Trigger |
|---|---|---|
| User text message | `Task` | text ≥ `min_content_chars` (first sentence becomes the objective) |
| Assistant text | `Decision` | text mentions "decide"/"decision" |
| Tool-use block | `FileState` | tool input has a path-like value (`path`, `file_path`, …) |
| Tool-result block | `ErrorResolution` | content mentions "error" |
| Compaction summary | `StateSnapshot` | `GateInput.compaction` present |

Gate-assigned default importance follows the same strategy's priorities:
`Decision` 80 > `StateSnapshot` 75 > `ErrorResolution` 70 > `Task` 60 >
`FileState` 50 > `Fact` 40 (floored by `GateConfig::default_importance`).
Keys are deduplicated within one transform (later records win), and the
per-transform output is capped by `GateConfig::max_records_per_transform`.

An LLM-backed gate (summarizer behind the same trait) is a designed future
extension point — the trait is the contract, not the heuristics.

## 3. The database — `MidTermStore`

```rust
pub trait MidTermStore: Send + Sync + Debug {
    fn name(&self) -> &'static str;
    fn upsert(&self, record: MemoryRecord) -> Result<(), MemoryError>; // replace by key
    fn get(&self, key: &str) -> Option<MemoryRecord>;
    fn delete(&self, key: &str) -> Result<(), MemoryError>;            // NotFound if absent
    fn query(&self, filter: &MemoryQuery) -> Vec<MemoryRecord>;        // newest first
    fn count(&self) -> usize;
    fn flush(&self) -> Result<(), MemoryError>;
}

pub struct MemoryQuery {                 // all fields ANDed; all optional
    pub kinds: Option<Vec<MemoryKind>>,
    pub session_id: Option<String>,
    pub min_importance: Option<u8>,
    pub key_contains: Option<String>,    // case-insensitive
    pub limit: Option<usize>,
}
```

Upsert preserves `created_at` and `access_count` of the replaced record.

`JsonMidTermStore` implements the trait against a versioned JSON envelope
(write-through on every mutation, matching the `JsonFileMemoryProvider` /
`VectorMemoryProvider` precedent) and additionally implements
`MemoryProvider` (name `"mid_term_json"`, writable) so it can register into
an existing `MemoryRegistry`.

## Storage Format

`.amadeus/mid_term_memory.json`:

```json
{
  "version": 1,
  "updated_at": 1784678400,
  "records": [
    {
      "id": "mt:s1:decision-use-json-envelope",
      "key": "decision:decision we will use a json envelope for the store",
      "kind": "decision",
      "content": "Decision: we will use a JSON envelope for the store.",
      "session_id": "s1",
      "source_message_indices": [3, 3],
      "created_at": 1784678000,
      "updated_at": 1784678400,
      "access_count": 0,
      "importance": 80
    },
    {
      "id": "mt:s1:file-src-lib-rs",
      "key": "file:src-lib-rs",
      "kind": "file_state",
      "content": "File touched: src/lib.rs",
      "session_id": "s1",
      "source_message_indices": [7, 7],
      "created_at": 1784678100,
      "updated_at": 1784678100,
      "access_count": 0,
      "importance": 50
    }
  ]
}
```

Notes:
- `version` allows forward-compatible migrations; loaders start fresh (with
  a warning) on parse failure or unknown shapes.
- `kind` serializes snake_case (`task`, `decision`, `file_state`,
  `error_resolution`, `state_snapshot`, `fact`).
- Timestamps are unix epoch seconds to keep the crate dependency-light.

## How to Use

```rust
use amadeus_memory::gate::{ContextGate, GateInput, RuleBasedGate};
use amadeus_memory::store::{JsonMidTermStore, MemoryQuery, MidTermStore};

// 1. Open the database (loads existing records; creates on first write).
let store = JsonMidTermStore::open(workdir.join(".amadeus/mid_term_memory.json"));

// 2. Run the gate over context that is being compacted away.
let gate = RuleBasedGate::default();
let records = gate.transform(&GateInput {
    session_id: Some(&session_id),
    messages: &retired_messages,
    compaction: Some(&compaction_result),
})?;

// 3. Persist.
for record in records {
    store.upsert(record)?;
}

// 4. Recall later — e.g. high-importance decisions for a session's follow-up.
let key_decisions = store.query(&MemoryQuery {
    kinds: Some(vec![amadeus_memory::MemoryKind::Decision]),
    min_importance: Some(70),
    ..Default::default()
});
```

## Future Wiring (not implemented)

Planned integration points, listed here so the interfaces stay stable:

1. **Compaction hook** — in `crates/core/src/agent/loop_agent.rs` where
   `ContextCompactor::compact` succeeds (~line 1000): run the gate over the
   summarized messages + `CompactionResult`, upsert into a shared
   `JsonMidTermStore`. `SessionMemoryProvider::push_compaction_summary` is
   the in-memory precedent for this hook.
2. **Prompt injection** — register the store into the existing
   `MemoryRegistry` (it already implements `MemoryProvider`); the current
   `## Persistent Memory` system-prompt section picks it up with no changes.
   Relevance-based (importance-filtered, capped) injection would use
   `MemoryQuery` instead of `build_memory_content`.
3. **HTTP surface** — extend `crates/api/src/api/handlers/memory.rs` with
   mid-term query endpoints; the request/response shapes map 1:1 onto
   `MemoryQuery` / `MemoryRecord`.
4. **Long-term promotion** — a background pass promoting high-importance,
   repeatedly-accessed mid-term records into the long-term JSON/RAG
   providers.

## Verification

`cargo test -p memory` covers record serde round-trips, store upsert/query/
persistence/corruption recovery, the `MemoryProvider` adapter, and every
gate extraction rule. (This crate was authored on a machine without a Rust
toolchain — run the suite once `cargo` is available.)

---
*Last updated: 2026-08-26*
