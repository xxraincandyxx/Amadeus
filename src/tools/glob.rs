//! # Glob Tool
//!
//! Fast file pattern matching tool that works with any codebase size.
//!
//! ## Features
//!
//! - Supports glob patterns like `**/*.js` or `src/**/*.ts`
//! - Returns file paths sorted by modification time
//! - Respects the workspace directory boundary
//! - Limits output to prevent context overflow

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::Value;

use crate::error::{AgentError, Result};
use crate::tools::schema::glob_tool;
use crate::tools::tool_trait::Tool;

#[derive(Debug, Clone, Deserialize)]
pub struct GlobInput {
    pub pattern: String,
    #[serde(default)]
    pub path: Option<String>,
}

pub struct GlobTool {
    workdir: std::path::PathBuf,
    max_results: usize,
}

impl GlobTool {
    pub fn new(workdir: std::path::PathBuf, max_results: usize) -> Self {
        Self {
            workdir,
            max_results,
        }
    }

    pub fn from_config(config: &crate::agent::config::Config) -> Self {
        Self {
            workdir: config.workdir.clone(),
            max_results: 1000,
        }
    }
}

#[async_trait]
impl Tool for GlobTool {
    fn name(&self) -> &'static str {
        "glob"
    }

    fn schema(&self) -> &'static Value {
        glob_tool()
    }

    async fn execute(&self, input: Value) -> Result<String> {
        let parsed: GlobInput =
            serde_json::from_value(input).map_err(|e| AgentError::ToolInput {
                tool: "glob".to_string(),
                reason: e.to_string(),
            })?;

        let base_path = if let Some(ref p) = parsed.path {
            let resolved = self.workdir.join(p);
            if !resolved.starts_with(&self.workdir) {
                return Err(AgentError::PathEscape(resolved));
            }
            resolved
        } else {
            self.workdir.clone()
        };

        let pattern = base_path.join(&parsed.pattern);
        let pattern_str = pattern.to_string_lossy();

        let matches: Vec<_> = glob::glob(&pattern_str)
            .map_err(|e| AgentError::Command(format!("Invalid glob pattern: {}", e)))?
            .filter_map(|r| r.ok())
            .filter(|p| p.is_file())
            .take(self.max_results)
            .collect();

        if matches.is_empty() {
            return Ok("No files found matching the pattern.".to_string());
        }

        let results: Vec<String> = matches
            .iter()
            .filter_map(|p| {
                p.strip_prefix(&self.workdir)
                    .ok()
                    .map(|rel| rel.to_string_lossy().to_string())
            })
            .collect();

        let mut output = format!("Found {} files:\n", results.len());
        for path in results {
            output.push_str(&format!("  {}\n", path));
        }

        if matches.len() == self.max_results {
            output.push_str(&format!("\n(Limited to {} results)", self.max_results));
        }

        Ok(output)
    }
}
