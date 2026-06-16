// @amadeus-header
// summary: Module root for the test utils subsystem and its exports.
// layer: infra
// status: active
// feature_flags:
// - test-utils
// provides:
// - module: crate::test_utils
// uses: none
// invariants:
// - Module exports stay aligned with child modules and re-exports.
// side_effects: none
// tests:
// - tests/mod.rs
// @end-amadeus-header

//! # Test Utilities
//!
//! Testing infrastructure inspired by gemini-cli's TestRig pattern.

pub mod assertions;
pub mod file_system;
pub mod fixtures;
pub mod frame_text;
pub mod scenario;
pub mod testflow;

pub use file_system::{
    create_tmp_dir, file_content_equals, files_equal, sample_rust_project, FileSystemStructure,
    TmpDirBuilder, TmpDirWithStructure,
};
pub use fixtures::{FixtureLoader, GoldenFile, GoldenFileManager, GoldenResponse};

pub use testflow::{load_session, RecorderConfig, SessionLog, SessionRecorder};
