//! # Tools Module
//!
//! Tool implementations for the agent.

pub mod bash;
pub mod file;
pub mod peer;
pub mod registry;
pub mod schema;
pub mod tool_trait;

pub use bash::BashTool;
pub use file::{EditFileTool, FileTools, ReadFileTool, WriteFileTool};
#[cfg(feature = "supervisor")]
pub use peer::PeerTool;
pub use registry::ToolRegistry;
pub use tool_trait::Tool;
