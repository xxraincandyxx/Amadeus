//! # Agent Module
//!
//! Core agent components including the main loop, configuration, and message types.
//!
//! ## Components
//!
//! - **`config`**: Configuration loading from environment variables
//! - **`messages`**: Message and content block types for LLM communication
//! - **`loop_agent`**: The main agent loop that drives conversation

pub mod config;
pub mod loop_agent;
pub mod messages;

pub use loop_agent::{Agent, RunResult, ToolCall};
