//! # Tools Module
//!
//! Tool implementations for the agent.

pub mod bash;
pub mod file;
pub mod glob;
pub mod grep;
pub mod peer;
pub mod registry;
pub mod schema;
pub mod sub_agent;
pub mod todo;
pub mod tool_trait;
pub mod web;

pub use bash::BashTool;
pub use file::{EditFileTool, FileTools, ReadFileTool, WriteFileTool};
pub use glob::GlobTool;
pub use grep::GrepTool;
#[cfg(feature = "supervisor")]
pub use peer::PeerTool;
pub use registry::ToolRegistry;
pub use sub_agent::SubAgnetTool;
pub use todo::{TodoItem, TodoManager, TodoStatus, TodoTool};
pub use tool_trait::Tool;
pub use web::WebFetchTool;
