// @amadeus-header
// summary: Public API module code for prelude.
// layer: api
// status: active
// feature_flags:
// - api
// provides:
// - module: crate::api::prelude
// uses: none
// invariants:
// - Listed interfaces stay aligned with the implementation in this file.
// side_effects: none
// tests:
// - tests/agent_integration_test.rs
// @end-amadeus-header

//! # API Prelude
//!
//! Convenient import for all commonly used public types.
//!
//! ## Usage
//!
//! ```rust,ignore
//! // Import all public types
//! use claude_agent::api::prelude::*;
//!
//! // Now you have access to:
//! // - Agent, Config, Provider
//! // - Message, ContentBlock, ToolInput
//! // - LLMClient, StreamEvent
//! // - AnthropicClient, OpenAIClient
//! // - BashTool, bash_tool
//! // - AgentError, Result
//! ```
//!
//! ## What's Included
//!
//! | Category | Types |
//! |----------|-------|
//! | Agent | `Agent` |
//! | Config | `Config`, `Provider` |
//! | Messages | `Message`, `ContentBlock`, `ToolInput` |
//! | Clients | `LLMClient`, `AnthropicClient`, `OpenAIClient`, `StreamEvent` |
//! | Tools | `BashTool`, `bash_tool` |
//! | Errors | `AgentError`, `Result` |
//!
//! ## When to Use
//!
//! Use the prelude when you want quick access to all SDK types without
//! specifying each import individually.
//!
//! For more selective imports, use the full path:
//!
//! ```rust,ignore
//! use claude_agent::api::{Agent, Config, Message};
//! ```

/*
 * ============================================================================
 * PRELUDE RE-EXPORTS
 * ============================================================================
 *
 * The `pub use *` syntax re-exports ALL public items from the parent module.
 * This is the idiomatic way to create a prelude in Rust.
 *
 * When users write `use claude_agent::api::prelude::*;`, they get all
 * the types listed in the parent module's re-exports.
 */

// Re-export everything from the parent api module
//
// This includes:
// - All re-exports from other modules (Agent, Config, Message, etc.)
// - All public items defined in api module itself
//
// Using `*` here means we don't need to manually list each type,
// and new types added to api will automatically be included.
pub use crate::api::*;
