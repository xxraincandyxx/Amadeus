//! # Tool Schemas
//!
//! JSON schemas for tool definitions sent to LLMs.
//!
//! ## Purpose
//!
//! LLMs need to know what tools are available and how to use them.
//! The schemas define:
//!
//! - Tool name and description
//! - Input parameters and their types
//! - Required vs optional fields
//!
//! ## Format
//!
//! Schemas follow the JSON Schema format, compatible with both
//! Anthropic and OpenAI APIs (with slight transformations).
//!
//! ## Example
//!
//! ```rust,ignore
//! use crate::tools::schema::bash_tool;
//!
//! let schema = bash_tool();
//! // Returns JSON like:
//! // {
//! //   "name": "bash",
//! //   "description": "Execute shell command...",
//! //   "input_schema": {
//! //     "type": "object",
//! //     "properties": {
//! //       "command": { "type": "string", "description": "..." }
//! //     },
//! //     "required": ["command"]
//! //   }
//! // }
//! ```

/*
 * ============================================================================
 * IMPORTS
 * ============================================================================
 */

// Value is an enum that represents any JSON value
// It can be:
// - Null
// - Bool(bool)
// - Number(Number)
// - String(String)
// - Array(Vec<Value>)
// - Object(Map<String, Value>)
//
// This is a "dynamic" JSON type - unlike structs, it can hold any JSON shape
// Useful when you need to build JSON programmatically rather than derive it
use serde_json::Value;

/*
 * ============================================================================
 * BASH TOOL SCHEMA
 * ============================================================================
 *
 * This function generates the JSON schema for the bash tool.
 * The schema tells the LLM:
 * 1. The tool's name and what it does
 * 2. What inputs it accepts
 * 3. Which inputs are required
 */

/// Generate the bash tool schema.
///
/// This schema describes the bash tool to the LLM, including:
/// - What the tool does (description)
/// - What input it accepts (command string)
/// - How to use it effectively (usage patterns)
///
/// # Returns
///
/// A `serde_json::Value` representing the tool schema.
///
/// # Schema Structure
///
/// ```json
/// {
///   "name": "bash",
///   "description": "Execute shell command...",
///   "input_schema": {
///     "type": "object",
///     "properties": {
///       "command": {
///         "type": "string",
///         "description": "The shell command to execute"
///       }
///     },
///     "required": ["command"]
///   }
/// }
/// ```
pub fn bash_tool() -> Value {
    // -------------------------------------------------------------------------
    // THE json! MACRO
    // -------------------------------------------------------------------------

    // `serde_json::json!` is a macro that lets you write JSON-like syntax
    // directly in Rust code. It converts your JSON-like expression into
    // a serde_json::Value at compile time.
    //
    // Why use a macro instead of just writing JSON strings?
    // 1. Type-safe: Catches errors at compile time, not runtime
    // 2. Interpolation: Can embed Rust values with {variable}
    // 3. No string parsing overhead at runtime

    serde_json::json!({
        // ---------------------------------------------------------------------
        // TOOL NAME
        // ---------------------------------------------------------------------

        // The name of the tool
        // The LLM uses this name when calling the tool
        "name": "bash",

        // ---------------------------------------------------------------------
        // TOOL DESCRIPTION
        // ---------------------------------------------------------------------

        // A detailed description of what the tool does
        // This helps the LLM understand when and how to use it
        //
        // Note: Multi-line strings in Rust can span multiple lines
        // The \n characters are literal newlines in the string
        "description": "Execute shell command. Common patterns:\n\
                        - Read: cat/head/tail, grep/find/rg/ls, wc -l\n\
                        - Write: echo 'content' > file, sed -i 's/old/new/g' file\n\
                        - Subagent: For complex subtasks, spawn a subagent to keep context clean:\n\
                          cargo run -- 'task description' (spawns isolated agent, returns summary)",

        // ---------------------------------------------------------------------
        // INPUT SCHEMA
        // ---------------------------------------------------------------------

        // Defines the structure of the input object
        // This is a JSON Schema (https://json-schema.org/)
        //
        // "type": "object" means the input is a JSON object (like a Rust struct)
        "input_schema": {
            "type": "object",

            // "properties" defines each field of the object
            // This is like defining struct fields
            "properties": {
                // The "command" field
                "command": {
                    // It's a string type
                    "type": "string",

                    // Description helps the LLM understand what to put here
                    "description": "The shell command to execute"
                }
            },

            // "required" lists fields that MUST be present
            // If a field is not in "required", it's optional
            //
            // ["command"] means the "command" field is mandatory
            "required": ["command"]
        }
    })
    // Note: No semicolon needed after json! because it's the return value
    // The function returns the Value created by json!
}
