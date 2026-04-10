// @amadeus-header
// summary: Benchmark pipeline code for case.
// layer: benchmark
// status: active
// feature_flags: none
// provides:
// - module: crate::benchmark::case
// - type: crate::benchmark::case::BenchmarkMode
// - type: crate::benchmark::case::BenchmarkCaseConfig
// - type: crate::benchmark::case::BenchmarkExpectations
// - type: crate::benchmark::case::BenchmarkThresholds
// - type: crate::benchmark::case::BenchmarkCase
// - type: crate::benchmark::case::MockScript
// - type: crate::benchmark::case::MockStep
// uses:
// - module: crate::client::StreamEvent
// - module: crate::error
// - protocol: serde serialization
// - artifact: filesystem paths and files
// invariants:
// - Listed interfaces stay aligned with the implementation in this file.
// side_effects:
// - Reads or writes filesystem state.
// tests:
// - cmd: cargo test --features full
// @end-amadeus-header

use std::fs;
use std::path::Path;

use regex::Regex;
use serde::{Deserialize, Serialize};
use walkdir::WalkDir;

use crate::client::StreamEvent;
use crate::error::{AgentError, Result};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum BenchmarkMode {
    #[default]
    Mock,
    Live,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BenchmarkCaseConfig {
    #[serde(default)]
    pub timeout_seconds: Option<u64>,
    #[serde(default)]
    pub approval_mode: Option<String>,
    #[serde(default)]
    pub session_log_dir: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BenchmarkExpectations {
    #[serde(default)]
    pub required_substrings: Vec<String>,
    #[serde(default)]
    pub forbidden_substrings: Vec<String>,
    #[serde(default)]
    pub required_regex: Vec<String>,
    #[serde(default)]
    pub forbidden_regex: Vec<String>,
    #[serde(default)]
    pub required_tools: Vec<String>,
    #[serde(default)]
    pub forbidden_tools: Vec<String>,
    #[serde(default)]
    pub expect_error: Option<bool>,
    #[serde(default)]
    pub expect_approval: Option<bool>,
    #[serde(default)]
    pub expect_compaction: Option<bool>,
    #[serde(default)]
    pub min_output_len: Option<usize>,
    #[serde(default)]
    pub max_output_len: Option<usize>,
}

impl BenchmarkExpectations {
    pub fn validate(&self) -> Result<()> {
        for pattern in &self.required_regex {
            Regex::new(pattern).map_err(|e| {
                AgentError::Config(format!("Invalid required_regex pattern '{pattern}': {e}"))
            })?;
        }

        for pattern in &self.forbidden_regex {
            Regex::new(pattern).map_err(|e| {
                AgentError::Config(format!("Invalid forbidden_regex pattern '{pattern}': {e}"))
            })?;
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BenchmarkThresholds {
    #[serde(default)]
    pub max_duration_ms: Option<u64>,
    #[serde(default)]
    pub max_tool_calls: Option<usize>,
    #[serde(default)]
    pub max_errors: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkCase {
    pub id: String,
    pub suite: String,
    pub description: String,
    #[serde(default)]
    pub tags: Vec<String>,
    pub prompt: String,
    #[serde(default)]
    pub mode: BenchmarkMode,
    #[serde(default)]
    pub config: BenchmarkCaseConfig,
    #[serde(default)]
    pub mock_script: Option<MockScript>,
    #[serde(default)]
    pub expectations: BenchmarkExpectations,
    #[serde(default)]
    pub thresholds: BenchmarkThresholds,
}

impl BenchmarkCase {
    pub fn validate(&self) -> Result<()> {
        if self.id.trim().is_empty() {
            return Err(AgentError::Config(
                "Benchmark case id cannot be empty".to_string(),
            ));
        }
        if self.suite.trim().is_empty() {
            return Err(AgentError::Config(format!(
                "Benchmark case '{}' must declare a suite",
                self.id
            )));
        }
        if self.prompt.trim().is_empty() {
            return Err(AgentError::Config(format!(
                "Benchmark case '{}' prompt cannot be empty",
                self.id
            )));
        }
        if self.mode == BenchmarkMode::Mock && self.mock_script.is_none() {
            return Err(AgentError::Config(format!(
                "Benchmark case '{}' in mock mode requires mock_script",
                self.id
            )));
        }
        self.expectations.validate()?;
        Ok(())
    }

    pub fn load_file(path: &Path) -> Result<Self> {
        let content = fs::read_to_string(path)?;
        let case: BenchmarkCase = serde_json::from_str(&content)?;
        case.validate()?;
        Ok(case)
    }

    pub fn load_dir(path: &Path) -> Result<Vec<Self>> {
        let mut cases = Vec::new();

        if !path.exists() {
            return Err(AgentError::Config(format!(
                "Benchmark fixtures directory not found: {}",
                path.display()
            )));
        }

        for entry in WalkDir::new(path)
            .into_iter()
            .filter_map(std::result::Result::ok)
        {
            if !entry.file_type().is_file() {
                continue;
            }

            let entry_path = entry.path();
            let is_json = entry_path
                .extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| ext.eq_ignore_ascii_case("json"))
                .unwrap_or(false);

            if !is_json {
                continue;
            }

            cases.push(Self::load_file(entry_path)?);
        }

        cases.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(cases)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MockScript {
    #[serde(default)]
    pub steps: Vec<MockStep>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MockStep {
    #[serde(default)]
    pub delay_ms: Option<u64>,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub events: Vec<MockStreamEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MockStreamEvent {
    TextDelta {
        text: String,
    },
    ThinkingDelta {
        text: String,
    },
    ToolCallStart {
        id: String,
        name: String,
    },
    ToolCallDelta {
        arguments: String,
    },
    ToolCallDone {
        id: String,
    },
    StopReason {
        reason: String,
    },
    TokenUsage {
        input_tokens: u32,
        output_tokens: u32,
    },
}

impl From<MockStreamEvent> for StreamEvent {
    fn from(value: MockStreamEvent) -> Self {
        match value {
            MockStreamEvent::TextDelta { text } => StreamEvent::TextDelta(text),
            MockStreamEvent::ThinkingDelta { text } => StreamEvent::ThinkingDelta(text),
            MockStreamEvent::ToolCallStart { id, name } => StreamEvent::ToolCallStart { id, name },
            MockStreamEvent::ToolCallDelta { arguments } => {
                StreamEvent::ToolCallDelta { arguments }
            }
            MockStreamEvent::ToolCallDone { id } => StreamEvent::ToolCallDone(id),
            MockStreamEvent::StopReason { reason } => StreamEvent::StopReason(reason),
            MockStreamEvent::TokenUsage {
                input_tokens,
                output_tokens,
            } => StreamEvent::TokenUsage {
                input_tokens,
                output_tokens,
            },
        }
    }
}
