// @amadeus-header
// summary: Mid-term memory crate — context gate and record database interfaces.
// layer: core
// status: active
// feature_flags: none
// provides:
// - module: crate::record
// - module: crate::store
// - module: crate::gate
// uses:
// - crate: amadeus_context (MemoryEntry, MemoryError, MemoryProvider)
// - crate: amadeus_messages (Message)
// - crate: amadeus_compaction (CompactionResult)
// invariants:
// - Short-term memory remains the context layer (history + compaction); this crate only defines the mid tier.
// - Gate output is deterministic for identical input.
// side_effects:
// - JsonMidTermStore writes the configured JSON path on every mutation.
// tests:
// - cmd: cargo test -p memory
// @end-amadeus-header

//! Mid-term memory for Amadeus agents.
//!
//! Amadeus memory is layered:
//!
//! - **Short-term** — the live context window: `Agent.history`, session
//!   logs, and compaction summaries (existing `amadeus_compaction` +
//!   `amadeus_core` machinery).
//! - **Mid-term** — *this crate*. A record database that captures what the
//!   conversation actually established: tasks, decisions, files touched,
//!   errors and their resolutions, and compaction state snapshots. A
//!   [`gate::ContextGate`] transforms raw context into
//!   [`record::MemoryRecord`]s; a [`store::MidTermStore`] persists and
//!   queries them.
//! - **Long-term** — the existing durable providers (`.amadeus/memory.json`
//!   via `JsonFileMemoryProvider`, semantic RAG via `VectorMemoryProvider`).
//!
//! This crate is deliberately unwired: nothing in the agent loop, bridge,
//! or HTTP API depends on it yet. See `docs/MEMORY.md` for the interfaces,
//! storage format, and the planned integration points.
//!
//! # Example
//!
//! ```no_run
//! use amadeus_memory::gate::{ContextGate, GateInput, RuleBasedGate};
//! use amadeus_memory::store::{JsonMidTermStore, MemoryQuery, MidTermStore};
//!
//! # fn main() -> Result<(), amadeus_context::memory::MemoryError> {
//! let store = JsonMidTermStore::open(".amadeus/mid_term_memory.json".into());
//! let gate = RuleBasedGate::default();
//! let history = Vec::new(); // retired context messages
//! for record in gate.transform(&GateInput {
//!     session_id: Some("session-42"),
//!     messages: &history,
//!     compaction: None,
//! })? {
//!     store.upsert(record)?;
//! }
//! let tasks = store.query(&MemoryQuery::all());
//! # Ok(())
//! # }
//! ```

pub mod gate;
pub mod record;
pub mod store;

pub use gate::{ContextGate, GateConfig, GateInput, GateReport, RuleBasedGate};
pub use record::{MemoryKind, MemoryRecord};
pub use store::{JsonMidTermStore, MemoryQuery, MidTermStore};

use std::time::{SystemTime, UNIX_EPOCH};

/// Current unix time in seconds; records stay dependency-light without chrono.
pub(crate) fn now_epoch_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
