//! # Message Types
//!
//! Types for representing conversation messages and content blocks.
//!
//! ## Message Structure
//!
//! Messages follow the Anthropic Messages API format:
//!
//! ```json
//! {
//!   "role": "user" | "assistant",
//!   "content": [ContentBlock, ...]
//! }
//! ```
//!
//! ## Content Blocks
//!
//! Each message contains an array of content blocks:
//!
//! - `Text`: Plain text content
//! - `ToolUse`: A tool call request from the assistant
//! - `ToolResult`: The result of a tool call (in user message)
//!
//! ## Example Flow
//!
//! ```text
//! User: "List files in src/"
//!   → Message { role: "user", content: [Text { text: "List files in src/" }] }
//!
//! Assistant calls tool:
//!   → Message { role: "assistant", content: [ToolUse { name: "bash", input: { command: "ls src/" } }] }
//!
//! Tool result:
//!   → Message { role: "user", content: [ToolResult { tool_use_id: "...", content: "file1.rs\nfile2.rs" }] }
//!
//! Assistant responds:
//!   → Message { role: "assistant", content: [Text { text: "I found 2 files..." }] }
//! ```

/*
 * ============================================================================
 * IMPORTS
 * ============================================================================
 *
 * Serde is Rust's most popular serialization framework.
 * It converts Rust types to/from formats like JSON, YAML, etc.
 */

// `Serialize` - Trait for converting Rust types TO other formats (e.g., to JSON)
// `Deserialize` - Trait for converting FROM other formats (e.g., from JSON) to Rust types
//
// These traits are "derived" (auto-implemented) by the serde_derive crate
// which is included with the `serde` crate when you use `features = ["derive"]`
// in Cargo.toml.
use serde::{Deserialize, Serialize};

/*
 * ============================================================================
 * CONTENT BLOCK ENUM
 * ============================================================================
 *
 * An enum that represents different types of content in a message.
 *
 * This enum uses "tagged union" serialization - each variant gets a
 * "type" field in JSON indicating which variant it is.
 */

// `#[derive(Debug)]` - Auto-generates debug output for printing/inspection
// `#[derive(Clone)]` - Allows creating copies of this enum (.clone() method)
// `#[derive(Serialize)]` - Auto-generates JSON serialization code
// `#[derive(Deserialize)]` - Auto-generates JSON deserialization code
// `#[derive(PartialEq)]` - Allows comparing two ContentBlocks with ==
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
// `#[serde(tag = "type")]` is a SERDE ATTRIBUTE that changes JSON format.
//
// WITHOUT this attribute, JSON would look like:
//   { "Text": { "text": "hello" } }
//
// WITH this attribute, JSON looks like:
//   { "type": "text", "text": "hello" }
//
// The "type" field's value is determined by the `#[serde(rename = "...")]`
// attribute on each variant (see below).
#[serde(tag = "type")]
pub enum ContentBlock {
    // -------------------------------------------------------------------------
    // TEXT VARIANT
    // -------------------------------------------------------------------------

    // `#[serde(rename = "text")]` sets the "type" field value to "text"
    // when this variant is serialized to JSON.
    //
    // Example JSON output:
    //   { "type": "text", "text": "Hello, world!" }
    #[serde(rename = "text")]
    Text {
        // The actual text content. String is an owned, heap-allocated string.
        // This is NOT a &str (string slice) because:
        // 1. This struct needs to OWN the data (not borrow it)
        // 2. The data comes from JSON parsing (not from elsewhere in memory)
        text: String,
    },

    // -------------------------------------------------------------------------
    // TOOL USE VARIANT
    // -------------------------------------------------------------------------

    // Represents the LLM requesting to call a tool.
    // This variant appears in ASSISTANT messages.
    //
    // `#[serde(rename = "tool_use")]` sets "type": "tool_use" in JSON.
    //
    // Example JSON:
    //   {
    //     "type": "tool_use",
    //     "id": "toolu_123",
    //     "name": "bash",
    //     "input": { "command": "ls -la" }
    //   }
    #[serde(rename = "tool_use")]
    ToolUse {
        // Unique identifier for this specific tool call.
        // Used to match tool results back to their requests.
        id: String,

        // Name of the tool to call (e.g., "bash")
        name: String,

        // Input parameters for the tool.
        // ToolInput is a struct defined below.
        input: ToolInput,
    },

    // -------------------------------------------------------------------------
    // TOOL RESULT VARIANT
    // -------------------------------------------------------------------------

    // Represents the OUTPUT of a tool execution.
    // This variant appears in USER messages (we send results back to the LLM).
    //
    // `#[serde(rename = "tool_result")]` sets "type": "tool_result" in JSON.
    //
    // Example JSON:
    //   {
    //     "type": "tool_result",
    //     "tool_use_id": "toolu_123",
    //     "content": "file1.txt\nfile2.txt\nfile3.txt"
    //   }
    #[serde(rename = "tool_result")]
    ToolResult {
        // The ID of the ToolUse this result corresponds to.
        // This links the result back to the original tool call.
        tool_use_id: String,

        // The output from executing the tool.
        // For bash, this is combined stdout + stderr.
        content: String,
    },
}

/*
 * ============================================================================
 * MESSAGE STRUCT
 * ============================================================================
 *
 * A struct representing one message in the conversation.
 * Messages have a role (who sent it) and content (what was said).
 */

// `#[derive(Debug)]` - Auto-generates debug output
// `#[derive(Clone)]` - Allows creating copies
// `#[derive(Serialize)]` - Allows converting to JSON
// `#[derive(Deserialize)]` - Allows parsing from JSON
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    // Who sent this message: "user" or "assistant"
    //
    // We use String instead of an enum because:
    // 1. The API sends strings, not enums
    // 2. It's simpler for serialization/deserialization
    // 3. We validate role values in other parts of the code
    pub role: String,

    // What was said - an array of content blocks.
    //
    // Why Vec (Vector) instead of array?
    // - Vec can grow/shrink dynamically
    // - Array size must be known at compile time
    // - Messages can have any number of content blocks
    //
    // The `pub` keyword makes this field publicly accessible
    // from outside the module.
    pub content: Vec<ContentBlock>,
}

/*
 * ============================================================================
 * TOOL INPUT STRUCT
 * ============================================================================
 *
 * Input parameters for the bash tool.
 * Currently only has one field, but could be extended for other tools.
 */

// `#[derive(Debug)]` - Debug output
// `#[derive(Clone)]` - Copyable
// `#[derive(Serialize)]` - To JSON
// `#[derive(Deserialize)]` - From JSON
// `#[derive(PartialEq)]` - Comparable with ==
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolInput {
    // The shell command to execute.
    // Example: "ls -la", "cat file.txt", "echo 'hello world'"
    pub command: String,
}

/*
 * ============================================================================
 * MESSAGE IMPLEMENTATION
 * ============================================================================
 *
 * The `impl` block adds methods (functions) to the Message struct.
 * These are like "associated functions" - functions that belong to the type.
 */

impl Message {
    // -------------------------------------------------------------------------
    // USER MESSAGE CONSTRUCTOR
    // -------------------------------------------------------------------------

    /// Create a user message with text content.
    ///
    /// This is a "associated function" (static method) - called like:
    ///   let msg = Message::user("Hello");
    ///
    /// NOT like:
    ///   let msg = some_message.user("Hello");  // Wrong!
    ///
    /// # Arguments
    ///
    /// * `text` - The text content of the message
    ///    Note: `&str` means this borrows a string slice (doesn't take ownership)
    ///
    /// # Returns
    ///
    /// A new Message instance with role "user" and one Text content block.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let msg = Message::user("List all rust files");
    /// assert_eq!(msg.role, "user");
    /// ```

    // `pub` - Public, can be called from outside the module
    // `fn` - Function keyword
    // `user` - Function name
    // `(text: &str)` - Takes one parameter: a string slice (borrowed string)
    // `-> Self` - Returns a Message (Self refers to the type being implemented)
    pub fn user(text: &str) -> Self {
        // Create and return a new Message instance
        Self {
            // Set role to "user"
            // to_string() converts &str to String (allocates on heap)
            //
            // Why convert to String?
            // - The field `role` is type String (owned)
            // - `text` is &str (borrowed)
            // - We need to OWN the data, so we allocate a new String
            role: "user".to_string(),

            // Set content to a vector with one Text block
            //
            // vec![] is a macro that creates a Vec
            // Equivalent to:
            //   let mut v = Vec::new();
            //   v.push(ContentBlock::Text { text: text.to_string() });
            //   v
            content: vec![ContentBlock::Text {
                // Convert the borrowed &str to owned String
                // Same reason as above - we need to own the data
                text: text.to_string(),
            }],
        }
        // No semicolon here - this is the return value (implicit return)
        // Could also write: return Self { ... };
    }

    // -------------------------------------------------------------------------
    // ASSISTANT MESSAGE CONSTRUCTOR
    // -------------------------------------------------------------------------

    /// Create an assistant message with content blocks.
    ///
    /// # Arguments
    ///
    /// * `content` - The content blocks from the assistant
    ///    Note: Takes Vec<ContentBlock> by value (takes ownership)
    ///
    /// # Returns
    ///
    /// A new Message instance with role "assistant".
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let msg = Message::assistant(vec![
    ///     ContentBlock::Text { text: "Done!".to_string() }
    /// ]);
    /// assert_eq!(msg.role, "assistant");
    /// ```

    // Takes `content: Vec<ContentBlock>` by value (not by reference)
    // This means the caller TRANSFERS OWNERSHIP of the Vec to this function
    //
    // Why take by value instead of by reference (&Vec)?
    // - We need to store the Vec in the Message struct
    // - If we took &Vec, we'd need to clone it (extra allocation)
    // - Taking by value is more efficient - no copying needed
    pub fn assistant(content: Vec<ContentBlock>) -> Self {
        Self {
            // Set role to "assistant"
            role: "assistant".to_string(),

            // Use the content that was passed in
            // No .clone() needed because we already own it
            // (the caller gave us ownership)
            content,
        }
    }
}
