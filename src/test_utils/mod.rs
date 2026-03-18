//! # Test Utilities
//!
//! Testing infrastructure inspired by gemini-cli's TestRig pattern.

pub mod assertions;
pub mod file_system;
pub mod fixtures;
pub mod testflow;

pub use file_system::{
    create_tmp_dir, file_content_equals, files_equal, sample_rust_project, FileSystemStructure,
    TmpDirBuilder, TmpDirWithStructure,
};
pub use fixtures::{FixtureLoader, GoldenFile, GoldenFileManager, GoldenResponse};

pub use testflow::{load_session, RecorderConfig, SessionLog, SessionRecorder};
