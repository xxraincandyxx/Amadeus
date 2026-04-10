// @amadeus-header
// summary: Core trait contract implemented by all Amadeus tools.
// layer: tools
// status: active
// feature_flags: none
// provides:
// - module: crate::tools::tool_trait
// - trait: crate::tools::tool_trait::Tool
// uses:
// - module: crate::error::Result
// - format: JSON values
// invariants:
// - Declared tool interfaces stay aligned with runtime behavior and schema.
// side_effects: none
// tests:
// - tests/tool_approval_test.rs
// @end-amadeus-header

//! # Tool Trait
//!
//! Defines the common interface for all agent tools.
//!
//! ## Design
//!
//! Each tool implements the `Tool` trait, which provides:
//! - A unique name for identification
//! - A JSON schema for the LLM to understand the tool's interface
//! - An async execute method that takes JSON input and returns a string result
//!
//! ## Example
//!
//! ```rust,ignore
//! use crate::tools::Tool;
//! use serde_json::json;
//!
//! async fn run_tool(tool: &dyn Tool, input: Value) -> Result<String> {
//!     let result = tool.execute(input).await?;
//!     Ok(result)
//! }
//! ```

use async_trait::async_trait;
use serde_json::Value;

use crate::error::Result;

#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &'static str;

    fn schema(&self) -> &'static Value;

    async fn execute(&self, input: Value) -> Result<String>;
}
