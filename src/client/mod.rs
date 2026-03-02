//! # LLM Client Abstraction
//!
//! This module provides a trait-based abstraction for LLM providers,
//! enabling easy swapping between Anthropic and OpenAI APIs.
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────┐
//! │   Agent     │
//! └──────┬──────┘
//!        │ uses
//!        ▼
//! ┌─────────────┐
//! │ LLMClient   │  (trait)
//! └──────┬──────┘
//!        │ implemented by
//!   ┌────┴────┐
//!   ▼         ▼
//! Anthropic  OpenAI
//! ```
//!
//! ## Usage
//!
//! The `LLMClient` trait defines two main methods:
//!
//! - `create_message`: Non-streaming request, returns complete response
//! - `create_message_stream`: Streaming request, returns event stream
//!
//! ```rust,ignore
//! use crate::client::LLMClient;
//!
//! // Non-streaming
//! let (stop_reason, content) = client.create_message(&system, &messages, &tools, 8000).await?;
//!
//! // Streaming
//! let mut stream = client.create_message_stream(&system, &messages, &tools, 8000).await?;
//! while let Some(event) = stream.next().await {
//!     match event? {
//!         StreamEvent::TextDelta(text) => print!("{}", text),
//!         StreamEvent::ToolCallStart { id, name } => { /* handle */ }
//!         // ...
//!     }
//! }
//! ```

/*
 * ============================================================================
 * IMPORTS
 * ============================================================================
 */

// ContentBlock and Message types for request/response data
use crate::agent::messages::{ContentBlock, Message};

// Our Result type (wraps AgentError)
use crate::error::Result;

// async_trait - a crate that enables async methods in traits
//
// WHY IS THIS NEEDED?
// Rust doesn't natively support async in traits (yet - it's being worked on)
// The `async_trait` attribute macro transforms the trait definition
//
// Without async_trait, this wouldn't compile:
//   trait Foo { async fn bar(&self); }
//
// With async_trait, it works:
//   #[async_trait]
//   trait Foo { async fn bar(&self); }
//
// The macro rewrites the async method to return Pin<Box<dyn Future>>
use async_trait::async_trait;

// Stream trait for streaming responses
// A Stream is like an async Iterator - you call .next() and get futures
use futures::Stream;

// Pin - a type that "pins" a value in memory
//
// WHY IS PIN NEEDED?
// Self-referential futures (futures that reference themselves) need to
// not move in memory. Pin prevents the value from being moved.
//
// For streaming, we return Pin<Box<dyn Stream>> because:
// 1. Box puts the stream on the heap
// 2. Pin prevents it from being moved
// 3. dyn Stream is a trait object (dynamic dispatch)
use std::pin::Pin;

/*
 * ============================================================================
 * STREAM EVENT ENUM
 * ============================================================================
 *
 * Events that can occur during a streaming response.
 * As the LLM generates text, it emits these events incrementally.
 */

/// Events emitted during streaming responses.
///
/// These events represent the incremental updates received from the LLM
/// as it generates a response. The stream is consumed using `StreamExt::next()`.
#[derive(Debug, Clone)]
pub enum StreamEvent {
    /// A chunk of text content has been generated
    ///
    /// Example: The LLM says "Hello, " then "world!" in separate chunks
    /// You'd receive two TextDelta events: "Hello, " and "world!"
    TextDelta(String),

    /// A tool call is starting (provides tool ID and name)
    ///
    /// This event fires when the LLM decides to call a tool
    /// The `id` is unique to this specific tool call
    /// The `name` is the tool name (e.g., "bash")
    ToolCallStart {
        /// Unique identifier for this tool call
        id: String,
        /// Name of the tool being called
        name: String,
    },

    /// Additional arguments for the current tool call
    ///
    /// Tool arguments come in pieces (streaming)
    /// Example: For bash tool, you might get {"command": "ls"}, then more
    ToolCallDelta {
        /// JSON string containing tool arguments
        /// This is a string, not a parsed object, because it might be partial
        arguments: String,
    },

    /// The tool call has completed (provides tool ID)
    ///
    /// Signals that all arguments have been received
    /// Now you can execute the tool
    ToolCallDone(String),

    /// The response has finished with a stop reason
    ///
    /// Stop reasons include:
    /// - "end_turn" - LLM finished its response
    /// - "tool_use" - LLM is waiting for tool execution
    StopReason(String),

    /// Token usage information received from the API
    ///
    /// This is typically sent at the start and end of the message
    /// to provide input/output token counts.
    TokenUsage {
        /// Number of tokens in the input/prompt
        input_tokens: u32,
        /// Number of tokens in the output/completion
        output_tokens: u32,
    },
}

/*
 * ============================================================================
 * LLM CLIENT TRAIT
 * ============================================================================
 *
 * A trait defines a shared behavior that types can implement.
 * Think of it like an interface in other languages.
 *
 * Any type that implements LLMClient can be used by the Agent.
 * This allows swapping between Anthropic, OpenAI, or other providers.
 */

/// Trait for LLM API clients.
///
/// This trait abstracts the differences between LLM providers (Anthropic, OpenAI)
/// behind a common interface. Implementations handle:
///
/// - API authentication and request formatting
/// - Response parsing and normalization
/// - Streaming via Server-Sent Events (SSE)
///
/// # Thread Safety
///
/// Implementations must be `Send + Sync` to work with async runtimes
/// and shared across threads.
///
/// # Type Parameters
///
/// This trait has no type parameters (not generic)
/// It uses `Self` to refer to the implementing type
//
// #[async_trait] - enables async methods in this trait
// Without this, Rust would reject `async fn` in a trait
#[async_trait]
pub trait LLMClient: Send + Sync {
    // -----------------------------------------------------------------
    // NON-STREAMING METHOD
    // -----------------------------------------------------------------

    /// Create a message and get a complete response.
    ///
    /// # Arguments
    ///
    /// * `system` - System prompt/instructions for the model
    /// * `messages` - Conversation history as message array
    /// * `tools` - Available tools in JSON schema format
    /// * `max_tokens` - Maximum tokens in the response
    ///
    /// # Returns
    ///
    /// A tuple of (stop_reason, content_blocks) where:
    /// - `stop_reason`: Why generation stopped ("end_turn", "tool_use", etc.)
    /// - `content_blocks`: The generated content (text and/or tool calls)
    // `async fn` - This is an async method
    // &self - This is a method (takes &self), not a static function
    //
    // Parameter breakdown:
    // - &self: Borrows self immutably
    // - system: &str: Borrows a string slice (cheap)
    // - messages: &[Message]: Borrows a slice of Messages (cheap)
    // - tools: &[serde_json::Value]: Borrows a slice of JSON Values (cheap)
    // - max_tokens: u32: An unsigned 32-bit integer (cheap, Copy type)
    //
    // Return type: Result<(String, Vec<ContentBlock>)>
    // - Result: Our custom Result type (Ok or Err)
    // - (String, Vec<ContentBlock>): A tuple
    //   - String: The stop reason
    //   - Vec<ContentBlock>: The content blocks
    async fn create_message(
        &self,
        system: &str,
        messages: &[Message],
        tools: &[serde_json::Value],
        max_tokens: u32,
    ) -> Result<(String, Vec<ContentBlock>)>;

    // -----------------------------------------------------------------
    // STREAMING METHOD
    // -----------------------------------------------------------------

    /// Create a message with streaming response.
    ///
    /// Returns a stream of `StreamEvent` values that can be consumed
    /// incrementally as the model generates the response.
    ///
    /// # Arguments
    ///
    /// Same as `create_message`.
    ///
    /// # Returns
    ///
    /// A pinned `Stream` of `Result<StreamEvent>` values.
    /// The stream ends when `None` is returned from `next()`.
    // Return type is complex, let's break it down:
    //
    // Result<...> - Our Result type (Ok or Err)
    //
    // Pin<Box<dyn Stream<Item = Result<StreamEvent>> + Send>>
    //
    // Starting from inside:
    // - Stream: A trait for async iteration (like Iterator but async)
    // - Item = Result<StreamEvent>: Each item in the stream is a Result
    // - + Send: The stream can be sent between threads
    // - dyn: Dynamic dispatch (runtime polymorphism)
    // - Box: Heap allocation (needed for dynamic dispatch)
    // - Pin: Prevents the stream from being moved in memory
    //
    // Why this complex type?
    // 1. Streams need to be pinned (they're often self-referential)
    // 2. We return a trait object (don't know concrete type at compile time)
    // 3. Trait objects need to be behind a pointer (Box)
    // 4. The stream needs to be Send for async runtimes
    async fn create_message_stream(
        &self,
        system: &str,
        messages: &[Message],
        tools: &[serde_json::Value],
        max_tokens: u32,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamEvent>> + Send>>>;
}

/*
 * ============================================================================
 * MODULE DECLARATIONS AND RE-EXPORTS
 * ============================================================================
 */

// Declare submodules
// This tells Rust to look for src/client/anthropic.rs and src/client/openai.rs
pub mod anthropic;
pub mod openai;

// Re-export the client types
// This allows users to import them directly:
//   use crate::client::{AnthropicClient, OpenAIClient};
//
// Instead of:
//   use crate::client::anthropic::AnthropicClient;
//   use crate::client::openai::OpenAIClient;
pub use anthropic::AnthropicClient;
pub use openai::OpenAIClient;
