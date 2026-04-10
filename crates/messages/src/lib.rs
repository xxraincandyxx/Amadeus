// @amadeus-header
// summary: Shared conversation message model types reused across runtime surfaces.
// layer: core
// status: active
// feature_flags: none
// provides:
// - module: crate
// - type: crate::ContentBlock
// - type: crate::Message
// uses:
// - protocol: serde serialization
// - format: JSON values
// invariants:
// - Message and content block serialization stay transport-agnostic.
// side_effects: none
// tests:
// - cmd: cargo test -p messages
// @end-amadeus-header

//! Shared conversation message model types.

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum ContentBlock {
    #[serde(rename = "text")]
    Text { text: String },

    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: Value,
    },

    #[serde(rename = "tool_result")]
    ToolResult {
        tool_use_id: String,
        content: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: Vec<ContentBlock>,
}

impl Message {
    pub fn user(text: &str) -> Self {
        Self {
            role: "user".to_string(),
            content: vec![ContentBlock::Text {
                text: text.to_string(),
            }],
        }
    }

    pub fn assistant(content: Vec<ContentBlock>) -> Self {
        Self {
            role: "assistant".to_string(),
            content,
        }
    }

    pub fn tool_results(results: Vec<ContentBlock>) -> Self {
        Self {
            role: "user".to_string(),
            content: results,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constructors_produce_expected_roles() {
        assert_eq!(Message::user("hi").role, "user");
        assert_eq!(Message::assistant(Vec::new()).role, "assistant");
        assert_eq!(Message::tool_results(Vec::new()).role, "user");
    }
}
