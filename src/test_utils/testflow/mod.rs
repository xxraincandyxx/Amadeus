//! # Testflow
//!
//! Session recording and playback system for human-agent interaction testing.

pub mod recorder;
pub mod types;

pub use recorder::{load_session, SessionRecorder};
pub use types::{RecorderConfig, SessionLog};
