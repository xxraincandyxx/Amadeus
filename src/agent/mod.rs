//! # Agent Module
//!
//! Core agent components including the main loop, configuration, and message types.
//!
//! ## Components
//!
//! - **`config`**: Configuration loading from environment variables
//! - **`messages`**: Message and content block types for LLM communication
//! - **`loop_agent`**: The main agent loop that drives conversation

/*
 * ============================================================================
 * MODULE DECLARATIONS
 * ============================================================================
 *
 * Each `pub mod` declares a submodule within this module.
 * Since agent/ is a directory, this mod.rs file declares what's inside.
 *
 * The `pub` keyword makes these modules accessible from outside.
 */

// The config module - contains Config struct and Provider enum
// Looks for src/agent/config.rs
pub mod config;

// The messages module - contains Message, ContentBlock, ToolInput types
// Looks for src/agent/messages.rs
pub mod messages;

// The loop_agent module - contains the Agent struct and loop logic
// Looks for src/agent/loop_agent.rs
pub mod loop_agent;
