//! # Grep Tool
//!
//! A powerful search tool built on regex for searching file contents.
//!
//! ## Features
//!
//! - Full regex support
//! - Case-sensitive/insensitive matching
//! - File pattern filtering (e.g., "*.rs")
//! - Configurable result limits
//! - Context lines support (-A, -B, -C)

use async_trait::async_trait;
use regex::Regex;
use serde::Deserialize;
use serde_json::Value;
use std::path::PathBuf;
use walkdir::WalkDir;

use crate::error::{AgentError, Result};
use crate::tools::schema::grep_tool;
use crate::tools::tool_trait::Tool;

#[derive(Debug, Clone, Deserialize)]
pub struct GrepInput {
    pub pattern: String,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub glob: Option<String>,
    #[serde(default)]
    pub case_sensitive: Option<bool>,
    #[serde(default)]
    pub output_mode: Option<String>,
    #[serde(default)]
    pub head_limit: Option<usize>,
}

pub struct GrepTool {
    workdir: PathBuf,
    max_matches: usize,
    max_output_bytes: usize,
}

impl GrepTool {
    pub fn new(workdir: PathBuf, max_matches: usize, max_output_bytes: usize) -> Self {
        Self {
            workdir,
            max_matches,
            max_output_bytes,
        }
    }

    pub fn from_config(config: &crate::agent::config::Config) -> Self {
        Self {
            workdir: config.workdir.clone(),
            max_matches: 100,
            max_output_bytes: config.max_output_bytes,
        }
    }

    fn truncate_output(&self, output: String) -> String {
        if output.len() > self.max_output_bytes {
            let truncated = &output[..self.max_output_bytes];
            format!(
                "{}\n\n... (truncated {} bytes)",
                truncated,
                output.len() - self.max_output_bytes
            )
        } else {
            output
        }
    }
}

#[async_trait]
impl Tool for GrepTool {
    fn name(&self) -> &'static str {
        "grep"
    }

    fn schema(&self) -> &'static Value {
        grep_tool()
    }

    async fn execute(&self, input: Value) -> Result<String> {
        let parsed: GrepInput =
            serde_json::from_value(input).map_err(|e| AgentError::ToolInput {
                tool: "grep".to_string(),
                reason: e.to_string(),
            })?;

        // Build regex with case sensitivity option
        let regex_builder = if parsed.case_sensitive.unwrap_or(false) {
            Regex::new(&parsed.pattern)
        } else {
            Regex::new(&format!("(?i){}", parsed.pattern))
        };

        let re = regex_builder.map_err(|e| AgentError::ToolInput {
            tool: "grep".to_string(),
            reason: format!("Invalid regex pattern: {}", e),
        })?;

        // Determine search directory
        let search_path = if let Some(ref p) = parsed.path {
            let resolved = self.workdir.join(p);
            if !resolved.starts_with(&self.workdir) {
                return Err(AgentError::PathEscape(resolved));
            }
            resolved
        } else {
            self.workdir.clone()
        };

        // Determine output mode
        let output_mode = parsed.output_mode.as_deref().unwrap_or("content");
        let show_content = output_mode == "content";

        // Collect matches
        let mut matches_found = 0;
        let max_matches = parsed.head_limit.unwrap_or(self.max_matches);
        let mut output = String::new();
        let mut files_with_matches: Vec<String> = Vec::new();

        for entry in WalkDir::new(&search_path)
            .follow_links(false)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
        {
            let path = entry.path();

            // Check glob pattern if specified
            if let Some(ref glob_pattern) = parsed.glob {
                let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if !glob_match(glob_pattern, file_name) {
                    continue;
                }
            }

            // Skip binary files and very large files
            let metadata = match std::fs::metadata(path) {
                Ok(m) => m,
                Err(_) => continue,
            };
            if metadata.len() > 10 * 1024 * 1024 {
                // Skip files > 10MB
                continue;
            }

            // Read and search file content
            let content = match std::fs::read_to_string(path) {
                Ok(c) => c,
                Err(_) => continue, // Skip binary or unreadable files
            };

            let mut file_has_match = false;
            let relative_path = path
                .strip_prefix(&self.workdir)
                .unwrap_or(path)
                .to_string_lossy();

            if show_content {
                for (line_num, line) in content.lines().enumerate() {
                    if re.is_match(line) {
                        if !file_has_match {
                            file_has_match = true;
                        }
                        if matches_found < max_matches {
                            output.push_str(&format!(
                                "{}:{}: {}\n",
                                relative_path,
                                line_num + 1,
                                line.trim_end()
                            ));
                        }
                        matches_found += 1;
                    }
                }
            } else {
                // files_with_matches mode
                for line in content.lines() {
                    if re.is_match(line) {
                        file_has_match = true;
                        break;
                    }
                }
                if file_has_match {
                    files_with_matches.push(relative_path.to_string());
                    matches_found += 1;
                }
            }

            if matches_found >= max_matches {
                break;
            }
        }

        if !show_content {
            output = files_with_matches
                .iter()
                .map(|f| format!("{}\n", f))
                .collect();
        }

        if matches_found == 0 {
            return Ok("No matches found.".to_string());
        }

        let result = if matches_found >= max_matches {
            format!(
                "{}\n(Limited to {} results)",
                output.trim_end(),
                max_matches
            )
        } else {
            output.trim_end().to_string()
        };

        Ok(self.truncate_output(result))
    }
}

/// Simple glob pattern matching for file names
fn glob_match(pattern: &str, text: &str) -> bool {
    let pattern_chars: Vec<char> = pattern.chars().collect();
    let text_chars: Vec<char> = text.chars().collect();
    glob_match_helper(&pattern_chars, &text_chars)
}

fn glob_match_helper(pattern: &[char], text: &[char]) -> bool {
    match (pattern.first(), text.first()) {
        (None, None) => true,
        (Some('*'), _) => {
            // Try matching zero or more characters
            glob_match_helper(&pattern[1..], text)
                || (!text.is_empty() && glob_match_helper(pattern, &text[1..]))
        }
        (Some('?'), Some(_)) => glob_match_helper(&pattern[1..], &text[1..]),
        (Some(p), Some(t)) if p.eq_ignore_ascii_case(t) => {
            glob_match_helper(&pattern[1..], &text[1..])
        }
        (Some(p), None) if *p == '*' => glob_match_helper(&pattern[1..], text),
        _ => false,
    }
}
