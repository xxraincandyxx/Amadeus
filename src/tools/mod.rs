//! # Tools Module
//!
//! Tool implementations for the agent.
//!
//! ## Available Tools
//!
//! - **`bash`**: Execute shell commands with timeout support
//! - **`schema`**: JSON schemas for tool definitions
//!
//! ## Tool Architecture
//!
//! Tools are simple structs with an `execute` method that takes
//! input and returns a result string. The agent calls tools based
//! on LLM responses.

/*
 * ============================================================================
 * MODULE DECLARATIONS
 * ============================================================================
 */

// The bash module - contains BashTool for executing commands
// Looks for src/tools/bash.rs
pub mod bash;

// The schema module - contains tool schema definitions
// Looks for src/tools/schema.rs
pub mod schema;
