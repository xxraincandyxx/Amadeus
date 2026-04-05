// @amadeus-header
// summary: Module root for the testflow subsystem and its exports.
// layer: infra
// status: active
// feature_flags:
// - test-utils
// provides:
// - module: crate::test_utils::testflow
// uses: none
// invariants:
// - Module exports stay aligned with child modules and re-exports.
// side_effects: none
// tests:
// - tests/mod.rs
// @end-amadeus-header

//! # Testflow
//!
//! Session recording and playback system for human-agent interaction testing.

pub mod recorder;
pub mod types;

pub use recorder::{load_session, SessionRecorder};
pub use types::{RecorderConfig, SessionLog};
