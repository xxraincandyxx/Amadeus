//! TUI Snapshot Testing Infrastructure
//!
//! Complete visual regression testing for the terminal UI.

pub mod capture;
pub mod comparison;
pub mod harness;
pub mod scenarios;

// Re-exports for convenience
pub use capture::{TuiCapture, TuiFrameSnapshot};
pub use comparison::{compare, format_diff, FrameDiff};
pub use harness::{InputSequence, TuiTestHarness, run_scenario};
