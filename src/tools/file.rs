//! # File Tools
//!
//! File operations: read, write, and edit files safely.
//!
//! ## Security
//!
//! All paths are validated to ensure they stay within the workspace directory.
//! This prevents path traversal attacks (e.g., `../../../etc/passwd`).
//!
//! ## Tools
//!
//! - **read_file**: Read file contents with optional line limit
//! - **write_file**: Create or overwrite files (creates parent dirs)
//! - **edit_file**: Make surgical changes using exact string matching

use std::path::{Component, PathBuf};

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::Value;

use crate::agent::config::Config;
use crate::error::{AgentError, Result};
use crate::tools::schema::{edit_file_tool, read_file_tool, write_file_tool};
use crate::tools::tool_trait::Tool;

#[derive(Debug, Clone, Deserialize)]
pub struct ReadFileInput {
    pub path: String,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WriteFileInput {
    pub path: String,
    pub content: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EditFileInput {
    pub path: String,
    pub old_text: String,
    pub new_text: String,
    #[serde(default)]
    pub replace_all: bool,
}

#[derive(Clone)]
pub struct FileTools {
    workdir: PathBuf,
    max_output_bytes: usize,
}

impl FileTools {
    pub fn from_config(config: &Config) -> Self {
        Self {
            workdir: config.workdir.clone(),
            max_output_bytes: config.max_output_bytes,
        }
    }

    pub fn new(workdir: PathBuf, max_output_bytes: usize) -> Self {
        Self {
            workdir,
            max_output_bytes,
        }
    }

    fn safe_path(&self, p: &str) -> Result<PathBuf> {
        let path = self.workdir.join(p);

        let mut cleaned = PathBuf::new();
        for component in path.components() {
            match component {
                Component::ParentDir => {
                    if !cleaned.pop() {
                        return Err(AgentError::PathEscape(PathBuf::from(p)));
                    }
                }
                Component::CurDir => {}
                Component::Normal(c) => cleaned.push(c),
                Component::RootDir | Component::Prefix(_) => {
                    return Err(AgentError::PathEscape(PathBuf::from(p)));
                }
            }
        }

        if !cleaned.starts_with(&self.workdir) {
            return Err(AgentError::PathEscape(PathBuf::from(p)));
        }

        Ok(cleaned)
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

    pub async fn read(&self, path: &str, limit: Option<usize>) -> Result<String> {
        let fp = self.safe_path(path)?;

        let text = tokio::fs::read_to_string(&fp).await.map_err(|e| {
            AgentError::Io(std::io::Error::other(format!(
                "Failed to read {}: {}",
                path, e
            )))
        })?;

        let mut lines: Vec<&str> = text.lines().collect();

        if let Some(lim) = limit {
            if lim < lines.len() {
                lines = lines[..lim].to_vec();
            }
        }

        let result = lines.join("\n");
        Ok(self.truncate_output(result))
    }

    pub async fn write(&self, path: &str, content: &str) -> Result<String> {
        let fp = self.safe_path(path)?;

        if let Some(parent) = fp.parent() {
            tokio::fs::create_dir_all(parent).await.map_err(|e| {
                AgentError::Io(std::io::Error::other(format!(
                    "Failed to create dirs: {}",
                    e
                )))
            })?;
        }

        tokio::fs::write(&fp, content).await.map_err(|e| {
            AgentError::Io(std::io::Error::other(format!(
                "Failed to write {}: {}",
                path, e
            )))
        })?;

        Ok(format!("Wrote {} bytes to {}", content.len(), path))
    }

    pub async fn edit(
        &self,
        path: &str,
        old_text: &str,
        new_text: &str,
        replace_all: bool,
    ) -> Result<String> {
        let fp = self.safe_path(path)?;

        let content = tokio::fs::read_to_string(&fp).await.map_err(|e| {
            AgentError::Io(std::io::Error::other(format!(
                "Failed to read {}: {}",
                path, e
            )))
        })?;

        if !content.contains(old_text) {
            return Err(AgentError::TextNotFound {
                path: path.to_string(),
                snippet: if old_text.len() > 50 {
                    format!("{}...", &old_text[..50])
                } else {
                    old_text.to_string()
                },
            });
        }

        let new_content = if replace_all {
            content.replace(old_text, new_text)
        } else {
            content.replacen(old_text, new_text, 1)
        };

        tokio::fs::write(&fp, &new_content).await.map_err(|e| {
            AgentError::Io(std::io::Error::other(format!(
                "Failed to write {}: {}",
                path, e
            )))
        })?;

        Ok(format!("Edited {}", path))
    }
}

pub struct ReadFileTool(FileTools);
pub struct WriteFileTool(FileTools);
pub struct EditFileTool(FileTools);

impl ReadFileTool {
    pub fn new(tools: FileTools) -> Self {
        Self(tools)
    }
}

impl WriteFileTool {
    pub fn new(tools: FileTools) -> Self {
        Self(tools)
    }
}

impl EditFileTool {
    pub fn new(tools: FileTools) -> Self {
        Self(tools)
    }
}

#[async_trait]
impl Tool for ReadFileTool {
    fn name(&self) -> &'static str {
        "read_file"
    }

    fn schema(&self) -> &'static Value {
        read_file_tool()
    }

    async fn execute(&self, input: Value) -> Result<String> {
        let parsed: ReadFileInput =
            serde_json::from_value(input).map_err(|e| AgentError::ToolInput {
                tool: "read_file".to_string(),
                reason: e.to_string(),
            })?;

        self.0.read(&parsed.path, parsed.limit).await
    }
}

#[async_trait]
impl Tool for WriteFileTool {
    fn name(&self) -> &'static str {
        "write_file"
    }

    fn schema(&self) -> &'static Value {
        write_file_tool()
    }

    async fn execute(&self, input: Value) -> Result<String> {
        let parsed: WriteFileInput =
            serde_json::from_value(input).map_err(|e| AgentError::ToolInput {
                tool: "write_file".to_string(),
                reason: e.to_string(),
            })?;

        self.0.write(&parsed.path, &parsed.content).await
    }
}

#[async_trait]
impl Tool for EditFileTool {
    fn name(&self) -> &'static str {
        "edit_file"
    }

    fn schema(&self) -> &'static Value {
        edit_file_tool()
    }

    async fn execute(&self, input: Value) -> Result<String> {
        let parsed: EditFileInput =
            serde_json::from_value(input).map_err(|e| AgentError::ToolInput {
                tool: "edit_file".to_string(),
                reason: e.to_string(),
            })?;

        self.0
            .edit(
                &parsed.path,
                &parsed.old_text,
                &parsed.new_text,
                parsed.replace_all,
            )
            .await
    }
}
