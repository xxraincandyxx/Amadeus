//! # Context Compaction
//!
//! Automatic conversation history summarization to manage context window limits.
//!
//! ## Overview
//!
//! When conversations grow long, they can exceed the model's context window.
//! This module provides automatic compaction that:
//! 1. Monitors token usage in conversation history
//! 2. Triggers summarization when approaching limits
//! 3. Preserves recent messages and important context
//! 4. Uses LLM to generate meaningful summaries
//!
//! ## Usage
//!
//! ```rust,ignore
//! use amadeus::agent::compaction::{ContextCompactor, CompactionConfig};
//!
//! let compactor = ContextCompactor::new(CompactionConfig {
//!     threshold_percent: 80,
//!     target_percent: 40,
//!     preserve_recent: 6,
//!     ..Default::default()
//! });
//!
//! // Check if compaction is needed
//! if compactor.needs_compaction(&history, context_window_size) {
//!     let result = compactor.compact(&mut history, &client).await?;
//!     println!("Saved {} tokens", result.tokens_saved);
//! }
//! ```
//!
//! ## Compaction Strategy
//!
//! When compaction triggers:
//! 1. Recent N messages are preserved (preserve_recent)
//! 2. Older messages are summarized into a single system message
//! 3. Tool results are truncated if they exceed size limits
//! 4. Essential context (errors, decisions) is retained

use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

use crate::agent::messages::{ContentBlock, Message};
use crate::client::LLMClient;
use crate::error::Result;

/// Configuration for context compaction behavior.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactionConfig {
    /// Threshold percentage of context window to trigger compaction.
    /// Default: 75 (trigger when at 75% of context window)
    pub threshold_percent: u8,

    /// Target percentage after compaction.
    /// Default: 30 (compact down to 30% of context window)
    pub target_percent: u8,

    /// Number of recent messages to always preserve.
    /// Default: 6 (typically 3 turns of user/assistant pairs)
    pub preserve_recent: usize,

    /// Whether to use LLM for summarization (vs simple truncation).
    /// Default: true
    pub use_llm_summary: bool,

    /// Maximum characters for the generated summary.
    /// Default: 2000
    pub max_summary_chars: usize,

    /// Minimum messages before compaction is considered.
    /// Default: 10
    pub min_messages: usize,

    /// Maximum characters for tool results before truncation.
    /// Default: 5000
    pub max_tool_result_chars: usize,
}

impl Default for CompactionConfig {
    fn default() -> Self {
        Self {
            threshold_percent: 75,
            target_percent: 30,
            preserve_recent: 6,
            use_llm_summary: true,
            max_summary_chars: 2000,
            min_messages: 10,
            max_tool_result_chars: 5000,
        }
    }
}

/// Result of a compaction operation.
#[derive(Debug, Clone)]
pub struct CompactionResult {
    /// Original message count before compaction.
    pub original_count: usize,

    /// Message count after compaction.
    pub compacted_count: usize,

    /// Estimated original token count before compaction.
    pub original_tokens: usize,

    /// Estimated token count after compaction.
    pub new_tokens: usize,

    /// Estimated tokens saved by compaction.
    pub tokens_saved: usize,

    /// Summary of compacted content (if LLM was used).
    pub summary: Option<String>,

    /// Number of messages that were summarized.
    pub messages_summarized: usize,
}

/// Handles automatic context compaction for conversation history.
pub struct ContextCompactor {
    config: CompactionConfig,
}

impl ContextCompactor {
    /// Create a new compactor with the given configuration.
    pub fn new(config: CompactionConfig) -> Self {
        Self { config }
    }

    /// Create a compactor with default configuration.
    pub fn with_defaults() -> Self {
        Self::new(CompactionConfig::default())
    }

    /// Get the configuration.
    pub fn config(&self) -> &CompactionConfig {
        &self.config
    }

    /// Check if compaction is needed based on current history.
    ///
    /// This uses a simple character-based estimation (~4 chars per token).
    /// For more accurate token counting, use a proper tokenizer.
    pub fn needs_compaction(&self, history: &[Message], context_window_size: u32) -> bool {
        if history.len() < self.config.min_messages {
            return false;
        }

        let estimated_tokens = self.estimate_tokens(history);
        let threshold =
            (context_window_size as f64 * self.config.threshold_percent as f64 / 100.0) as u32;

        estimated_tokens > threshold as usize
    }

    /// Estimate token count for a slice of messages.
    ///
    /// Uses a simple heuristic of ~4 characters per token.
    /// This is a rough approximation that works reasonably well for English text.
    pub fn estimate_tokens(&self, history: &[Message]) -> usize {
        let total_chars: usize = history.iter().map(|m| self.message_chars(m)).sum();
        let tokens = total_chars.div_ceil(4);
        debug!(
            message_count = history.len(),
            total_chars = total_chars,
            estimated_tokens = tokens,
            "Token estimation"
        );
        // Rough estimate: ~4 chars per token (works for English, conservative for code)
        tokens
    }

    /// Count characters in a message (including all content blocks).
    fn message_chars(&self, message: &Message) -> usize {
        message
            .content
            .iter()
            .map(|block| match block {
                ContentBlock::Text { text } => text.len(),
                ContentBlock::ToolUse { name, input, .. } => name.len() + input.to_string().len(),
                ContentBlock::ToolResult { content, .. } => content.len(),
            })
            .sum()
    }

    /// Calculate context usage percentage.
    pub fn context_usage_percent(&self, history: &[Message], context_window_size: u32) -> u8 {
        let tokens = self.estimate_tokens(history);
        let percent = (tokens as f64 / context_window_size as f64 * 100.0) as u8;
        percent.min(100)
    }

    /// Validate that an LLM-generated summary is reasonable.
    ///
    /// Checks:
    /// 1. Minimum length (at least 50 characters)
    /// 2. Contains some technical terms from the original messages
    /// 3. Doesn't contain obvious hallucination patterns
    fn is_valid_summary(&self, summary: &str, original_messages: &[Message]) -> bool {
        // Check minimum length
        if summary.len() < 50 {
            debug!("Summary too short: {} chars", summary.len());
            return false;
        }

        // Extract key terms from original messages (files, tools, etc.)
        let original_terms = self.extract_technical_terms(original_messages);

        // Check if summary contains at least some of these terms
        let summary_lower = summary.to_lowercase();
        let matching_terms: Vec<_> = original_terms
            .iter()
            .filter(|term| summary_lower.contains(&term.to_lowercase()))
            .collect();

        // Require at least 1 matching technical term
        if matching_terms.is_empty() {
            debug!("Summary contains no technical terms from original messages");
            return false;
        }

        // Check for obvious hallucination patterns
        let hallucination_indicators = [
            "once upon a time",
            "in a galaxy far far away",
            "chapter 1",
            "the end",
            "to be continued",
            "part 1",
        ];

        for indicator in &hallucination_indicators {
            if summary_lower.contains(indicator) {
                debug!("Summary contains hallucination indicator: {}", indicator);
                return false;
            }
        }

        true
    }

    /// Extract technical terms from messages for validation.
    fn extract_technical_terms(&self, messages: &[Message]) -> Vec<String> {
        let mut terms = std::collections::HashSet::new();

        for message in messages {
            for block in &message.content {
                match block {
                    ContentBlock::Text { text } => {
                        // Extract file extensions and paths
                        for word in text.split_whitespace() {
                            let word_lower = word.to_lowercase();
                            // Check for file extensions
                            if word_lower.ends_with(".rs")
                                || word_lower.ends_with(".ts")
                                || word_lower.ends_with(".js")
                                || word_lower.ends_with(".py")
                                || word_lower.ends_with(".json")
                                || word_lower.ends_with(".toml")
                                || word_lower.ends_with(".md")
                            {
                                let cleaned = word.trim_matches(|c: char| {
                                    c == '`'
                                        || c == '"'
                                        || c == '\''
                                        || c == ','
                                        || c == '.'
                                        || c == '('
                                        || c == ')'
                                        || c == '\\'
                                });
                                if cleaned.len() > 3 {
                                    terms.insert(cleaned.to_string());
                                }
                            }
                            // Check for paths
                            if word.contains('/') && word.len() > 5 {
                                let cleaned = word.trim_matches(|c: char| {
                                    c == '`'
                                        || c == '"'
                                        || c == '\''
                                        || c == ','
                                        || c == '.'
                                        || c == '('
                                        || c == ')'
                                        || c == '\\'
                                });
                                if cleaned.len() > 5 {
                                    terms.insert(cleaned.to_string());
                                }
                            }
                        }
                    }
                    ContentBlock::ToolUse { name, input, .. } => {
                        terms.insert(name.clone());
                        // Extract paths from input
                        if let Some(path) = input.get("file_path").and_then(|v| v.as_str()) {
                            terms.insert(path.to_string());
                        }
                        if let Some(path) = input.get("path").and_then(|v| v.as_str()) {
                            terms.insert(path.to_string());
                        }
                    }
                    _ => {}
                }
            }
        }

        terms.into_iter().collect()
    }

    /// Perform compaction on history.
    ///
    /// This will:
    /// 1. Preserve recent messages (config.preserve_recent)
    /// 2. Summarize older messages using LLM (if enabled)
    /// 3. Truncate large tool results
    /// 4. Return a summary of what was compacted
    pub async fn compact<C: LLMClient + Clone + 'static>(
        &self,
        history: &mut Vec<Message>,
        client: &C,
        context_window_size: u32,
    ) -> Result<CompactionResult> {
        let original_count = history.len();
        let original_tokens = self.estimate_tokens(history);
        let target_tokens =
            (context_window_size as f64 * self.config.target_percent as f64 / 100.0) as usize;

        if original_count <= self.config.preserve_recent {
            debug!("History too short to compact");
            return Ok(CompactionResult {
                original_count,
                compacted_count: original_count,
                original_tokens,
                new_tokens: original_tokens,
                tokens_saved: 0,
                summary: None,
                messages_summarized: 0,
            });
        }

        info!(
            original_messages = original_count,
            original_tokens = original_tokens,
            target_tokens = target_tokens,
            "Starting context compaction"
        );

        // Split history: messages to summarize vs preserve
        let split_point = original_count.saturating_sub(self.config.preserve_recent);
        let to_summarize: Vec<Message> = history.drain(0..split_point).collect();
        let messages_summarized = to_summarize.len();

        // Generate summary if enabled
        let summary = if self.config.use_llm_summary && !to_summarize.is_empty() {
            match self.summarize_messages(&to_summarize, client).await {
                Ok(s) => {
                    // Validate summary is reasonable, fallback if not
                    if self.is_valid_summary(&s, &to_summarize) {
                        Some(s)
                    } else {
                        warn!("LLM summary failed validation, using extract-based compaction");
                        Some(self.extract_key_points(&to_summarize))
                    }
                }
                Err(e) => {
                    warn!(error = %e, "Failed to generate summary, using extract-based compaction");
                    Some(self.extract_key_points(&to_summarize))
                }
            }
        } else if !to_summarize.is_empty() {
            Some(self.extract_key_points(&to_summarize))
        } else {
            None
        };

        // Add summary as a system message at the start
        if let Some(ref summary_text) = summary {
            let summary_message = Message {
                role: "user".to_string(),
                content: vec![ContentBlock::Text {
                    text: format!(
                        "[Context Summary - Earlier conversation has been compacted]\n{}",
                        summary_text
                    ),
                }],
            };
            history.insert(0, summary_message);
        }

        // Truncate large tool results in remaining history
        self.truncate_tool_results(history);

        let compacted_count = history.len();
        let new_tokens = self.estimate_tokens(history);
        let tokens_saved = original_tokens.saturating_sub(new_tokens);

        info!(
            original_messages = original_count,
            compacted_messages = compacted_count,
            tokens_saved = tokens_saved,
            "Context compaction complete"
        );

        Ok(CompactionResult {
            original_count,
            compacted_count,
            original_tokens,
            new_tokens,
            tokens_saved,
            summary,
            messages_summarized,
        })
    }

    /// Generate a summary of messages using LLM.
    async fn summarize_messages<C: LLMClient + Clone + 'static>(
        &self,
        messages: &[Message],
        client: &C,
    ) -> Result<String> {
        // Format messages for summarization
        let conversation = self.format_messages_for_summary(messages);

        let summary_prompt = format!(
            r#"Summarize the following conversation history concisely. Focus on:
1. Key tasks and objectives discussed
2. Important decisions made
3. Files modified or created
4. Any errors encountered and how they were resolved
5. Current state and any pending work

Keep the summary under {} characters. Be specific about file names, code changes, and technical details.

Conversation to summarize:
{}
"#,
            self.config.max_summary_chars, conversation
        );

        // Create a minimal history for the summarization request
        let summary_request = vec![Message::user(&summary_prompt)];

        // Use the client to generate a summary
        let tool_schemas: Vec<serde_json::Value> = vec![];
        let mut stream = client
            .create_message_stream(
                "You are a helpful assistant that summarizes conversation history concisely and accurately.",
                &summary_request,
                &tool_schemas,
                1000, // Max tokens for summary
            )
            .await?;

        use futures::StreamExt;
        let mut summary_text = String::new();
        while let Some(event) = stream.next().await {
            match event {
                Ok(crate::client::StreamEvent::TextDelta(text)) => {
                    summary_text.push_str(&text);
                }
                Ok(crate::client::StreamEvent::StopReason(_)) => break,
                Err(e) => {
                    warn!(error = %e, "Error during summary generation");
                    break;
                }
                _ => {}
            }
        }

        // Truncate if too long
        if summary_text.len() > self.config.max_summary_chars {
            summary_text = summary_text[..self.config.max_summary_chars].to_string();
            // Try to end at a sentence boundary
            if let Some(last_period) = summary_text.rfind('.') {
                summary_text = summary_text[..last_period + 1].to_string();
            }
        }

        Ok(summary_text)
    }

    /// Extract key points from messages without using LLM.
    ///
    /// This is a fallback when LLM summarization fails or is disabled.
    fn extract_key_points(&self, messages: &[Message]) -> String {
        let mut points: Vec<String> = Vec::new();
        let mut files_mentioned: std::collections::HashSet<String> =
            std::collections::HashSet::new();
        let mut tools_used: std::collections::HashSet<String> = std::collections::HashSet::new();

        for message in messages {
            for block in &message.content {
                match block {
                    ContentBlock::Text { text } => {
                        // Extract file paths
                        for word in text.split_whitespace() {
                            if word.contains('/')
                                || word.contains(".rs")
                                || word.contains(".ts")
                                || word.contains(".js")
                                || word.contains(".py")
                                || word.contains(".json")
                                || word.contains(".toml")
                            {
                                let cleaned = word.trim_matches(|c: char| {
                                    c == '`' || c == '"' || c == '\'' || c == ',' || c == '.'
                                });
                                if cleaned.len() > 3 {
                                    files_mentioned.insert(cleaned.to_string());
                                }
                            }
                        }
                    }
                    ContentBlock::ToolUse { name, input, .. } => {
                        tools_used.insert(name.clone());
                        // Extract file paths from tool inputs
                        if let Some(path) = input.get("file_path").and_then(|v| v.as_str()) {
                            files_mentioned.insert(path.to_string());
                        }
                        if let Some(path) = input.get("path").and_then(|v| v.as_str()) {
                            files_mentioned.insert(path.to_string());
                        }
                    }
                    ContentBlock::ToolResult { content, .. } => {
                        // Check for errors in results
                        if content.contains("error") || content.contains("Error") {
                            points.push(format!(
                                "Error encountered: {}",
                                content.chars().take(200).collect::<String>()
                            ));
                        }
                    }
                }
            }
        }

        let mut summary = String::new();

        if !tools_used.is_empty() {
            summary.push_str(&format!(
                "Tools used: {}\n",
                tools_used.iter().cloned().collect::<Vec<_>>().join(", ")
            ));
        }

        if !files_mentioned.is_empty() {
            let files: Vec<_> = files_mentioned.iter().take(20).cloned().collect();
            summary.push_str(&format!("Files involved: {}\n", files.join(", ")));
        }

        if !points.is_empty() {
            summary.push_str("\nKey events:\n");
            for point in points.iter().take(5) {
                summary.push_str(&format!("- {}\n", point));
            }
        }

        if summary.is_empty() {
            summary = format!(
                "{} messages compacted ({} estimated tokens)",
                messages.len(),
                self.estimate_tokens(messages)
            );
        }

        summary
    }

    /// Format messages for summarization prompt.
    fn format_messages_for_summary(&self, messages: &[Message]) -> String {
        let mut formatted = String::new();
        let max_chars = 10000; // Limit conversation size sent for summarization
        let mut current_chars = 0;

        for message in messages {
            if current_chars >= max_chars {
                formatted.push_str("\n... (earlier messages omitted for brevity)");
                break;
            }

            formatted.push_str(&format!("\n[{}]:\n", message.role));

            for block in &message.content {
                match block {
                    ContentBlock::Text { text } => {
                        let truncated = if text.len() > 500 {
                            format!("{}... (truncated)", &text[..500])
                        } else {
                            text.clone()
                        };
                        formatted.push_str(&truncated);
                        formatted.push('\n');
                        current_chars += truncated.len();
                    }
                    ContentBlock::ToolUse { name, input, .. } => {
                        formatted.push_str(&format!("[Tool: {}]\n", name));
                        formatted.push_str(&input.to_string());
                        formatted.push('\n');
                        current_chars += name.len() + input.to_string().len();
                    }
                    ContentBlock::ToolResult { content, .. } => {
                        let truncated = if content.len() > 300 {
                            format!("{}... (truncated)", &content[..300])
                        } else {
                            content.clone()
                        };
                        formatted.push_str(&format!("[Result: {}]\n", truncated));
                        current_chars += truncated.len();
                    }
                }
            }

            formatted.push('\n');
        }

        formatted
    }

    /// Truncate large tool results in history.
    fn truncate_tool_results(&self, history: &mut [Message]) {
        for message in history.iter_mut() {
            for block in message.content.iter_mut() {
                if let ContentBlock::ToolResult { content, .. } = block {
                    if content.len() > self.config.max_tool_result_chars {
                        let truncated = format!(
                            "{}\n\n... [Output truncated, {} total characters]",
                            &content[..self.config.max_tool_result_chars],
                            content.len()
                        );
                        *content = truncated;
                    }
                }
            }
        }
    }
}

/// Event emitted during compaction.
#[derive(Debug, Clone)]
pub enum CompactionEvent {
    /// Compaction started.
    Started {
        message_count: usize,
        estimated_tokens: usize,
    },
    /// Progress update during compaction.
    Progress { stage: String },
    /// Compaction completed.
    Completed { result: CompactionResult },
    /// Compaction failed.
    Failed { error: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_estimate_tokens() {
        let compactor = ContextCompactor::with_defaults();

        let messages = vec![
            Message::user("Hello, this is a test message."),
            Message::assistant(vec![ContentBlock::Text {
                text: "I understand. How can I help you today?".to_string(),
            }]),
        ];

        let tokens = compactor.estimate_tokens(&messages);
        // Rough estimate: ~60 chars / 4 = ~15 tokens
        assert!(tokens > 0);
        assert!(tokens < 100);
    }

    #[test]
    fn test_needs_compaction_below_threshold() {
        let compactor = ContextCompactor::with_defaults();

        let messages = vec![
            Message::user("Hello"),
            Message::assistant(vec![ContentBlock::Text {
                text: "Hi there!".to_string(),
            }]),
        ];

        // Small messages should not trigger compaction
        assert!(!compactor.needs_compaction(&messages, 200_000));
    }

    #[test]
    fn test_needs_compaction_respects_min_messages() {
        let config = CompactionConfig {
            min_messages: 10,
            ..Default::default()
        };
        let compactor = ContextCompactor::new(config);

        // Create 5 large messages (below min_messages but would exceed threshold)
        let large_text = "x".repeat(100_000);
        let messages: Vec<Message> = (0..5).map(|_| Message::user(&large_text)).collect();

        // Should not trigger because below min_messages
        assert!(!compactor.needs_compaction(&messages, 200_000));
    }

    #[test]
    fn test_context_usage_percent() {
        let compactor = ContextCompactor::with_defaults();

        let messages = vec![Message::user("Hello world")];

        let percent = compactor.context_usage_percent(&messages, 100);
        // Small message should be < 100% of 100 token window
        assert!(percent < 100);
    }

    #[test]
    fn test_extract_key_points() {
        let compactor = ContextCompactor::with_defaults();

        let messages = vec![
            Message::user("Read the file src/main.rs"),
            Message::assistant(vec![ContentBlock::ToolUse {
                id: "1".to_string(),
                name: "read_file".to_string(),
                input: serde_json::json!({"file_path": "src/main.rs"}),
            }]),
            Message::tool_results(vec![ContentBlock::ToolResult {
                tool_use_id: "1".to_string(),
                content: "fn main() {}".to_string(),
            }]),
        ];

        let summary = compactor.extract_key_points(&messages);

        assert!(summary.contains("read_file"));
        assert!(summary.contains("src/main.rs"));
    }

    #[test]
    fn test_truncate_tool_results() {
        let config = CompactionConfig {
            max_tool_result_chars: 100,
            ..Default::default()
        };
        let compactor = ContextCompactor::new(config);

        let mut messages = vec![Message::tool_results(vec![ContentBlock::ToolResult {
            tool_use_id: "1".to_string(),
            content: "x".repeat(500),
        }])];

        compactor.truncate_tool_results(&mut messages);

        if let ContentBlock::ToolResult { content, .. } = &messages[0].content[0] {
            assert!(content.len() < 500);
            assert!(content.contains("truncated"));
        } else {
            panic!("Expected ToolResult");
        }
    }
}
