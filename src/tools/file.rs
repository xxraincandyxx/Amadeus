// @amadeus-header
// summary: Tool implementation and support code for file.
// layer: tools
// status: active
// feature_flags: none
// provides:
// - module: crate::tools::file
// - type: crate::tools::file::ReadFileInput
// - type: crate::tools::file::WriteFileInput
// - type: crate::tools::file::EditFileInput
// - type: crate::tools::file::FileTools
// - type: crate::tools::file::ReadFileTool
// - type: crate::tools::file::WriteFileTool
// - type: crate::tools::file::EditFileTool
// uses:
// - module: crate::agent::config::Config
// - module: crate::concurrency::FileLockManager
// - module: crate::core::id::AgentId
// - module: crate::error
// - module: crate::tools::schema
// - module: crate::tools::tool_trait::Tool
// - protocol: serde serialization
// - format: JSON values
// invariants:
// - Declared tool interfaces stay aligned with runtime behavior and schema.
// side_effects:
// - Reads or writes filesystem state.
// tests:
// - tests/tool_approval_test.rs
// @end-amadeus-header

//! # File Tools
//!
//! File operations: read, write, and edit files safely with concurrency control.
//!
//! ## Security
//!
//! All paths are validated to ensure they stay within the workspace directory.
//! This prevents path traversal attacks (e.g., `../../../etc/passwd`).
//!
//! ## Concurrency Control
//!
//! When a FileLockManager is provided:
//! - Read operations acquire shared locks (multiple readers allowed)
//! - Write/edit operations acquire exclusive locks (blocks all readers/writers)
//! - Read cache tracks modification times to detect external changes
//! - Write/edit operations fail if file was modified since last read
//!
//! ## Tools
//!
//! - **read_file**: Read file contents with optional line limit
//! - **write_file**: Create or overwrite files (creates parent dirs)
//! - **edit_file**: Make surgical changes using exact string matching

use std::path::{Component, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::Value;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use crate::agent::config::Config;
use crate::concurrency::FileLockManager;
use crate::core::id::AgentId;
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

/// Compute a simple hash of file content for cache validation.
fn compute_content_hash(content: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    content.hash(&mut hasher);
    hasher.finish()
}

#[derive(Clone)]
pub struct FileTools {
    workdir: PathBuf,
    max_output_bytes: usize,
    /// Optional file lock manager for concurrent access control.
    file_lock_manager: Option<Arc<FileLockManager>>,
    /// Agent ID for this agent (used for file lock tracking).
    agent_id: Option<AgentId>,
}

impl FileTools {
    pub fn from_config(config: &Config) -> Self {
        Self {
            workdir: config.workdir.clone(),
            max_output_bytes: config.max_output_bytes,
            file_lock_manager: None,
            agent_id: None,
        }
    }

    /// Create FileTools with file locking enabled.
    pub fn from_config_with_locks(
        config: &Config,
        file_lock_manager: Option<Arc<FileLockManager>>,
        agent_id: Option<AgentId>,
    ) -> Self {
        Self {
            workdir: config.workdir.clone(),
            max_output_bytes: config.max_output_bytes,
            file_lock_manager,
            agent_id,
        }
    }

    pub fn new(workdir: PathBuf, max_output_bytes: usize) -> Self {
        Self {
            workdir,
            max_output_bytes,
            file_lock_manager: None,
            agent_id: None,
        }
    }

    /// Create with file locking.
    pub fn new_with_locks(
        workdir: PathBuf,
        max_output_bytes: usize,
        file_lock_manager: Arc<FileLockManager>,
        agent_id: AgentId,
    ) -> Self {
        Self {
            workdir,
            max_output_bytes,
            file_lock_manager: Some(file_lock_manager),
            agent_id: Some(agent_id),
        }
    }

    fn safe_path(&self, p: &str) -> Result<PathBuf> {
        let path = self.workdir.join(p);

        let mut cleaned = PathBuf::new();
        let mut first = true;
        for component in path.components() {
            match component {
                Component::ParentDir => {
                    if !cleaned.pop() {
                        return Err(AgentError::PathEscape(PathBuf::from(p)));
                    }
                }
                Component::CurDir => {}
                Component::Normal(c) => cleaned.push(c),
                Component::RootDir => {
                    if first {
                        cleaned.push(component);
                    } else {
                        return Err(AgentError::PathEscape(PathBuf::from(p)));
                    }
                }
                Component::Prefix(_) => {
                    if first {
                        cleaned.push(component);
                    } else {
                        return Err(AgentError::PathEscape(PathBuf::from(p)));
                    }
                }
            }
            first = false;
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

    /// Get file modification time.
    async fn get_modified_time(path: &PathBuf) -> Result<SystemTime> {
        tokio::fs::metadata(path)
            .await
            .map_err(|e| AgentError::Io(std::io::Error::other(e.to_string())))?
            .modified()
            .map_err(|e| AgentError::Io(std::io::Error::other(e.to_string())))
    }

    pub async fn read(&self, path: &str, limit: Option<usize>) -> Result<String> {
        let fp = self.safe_path(path)?;

        // If file locking is enabled, acquire read lock
        if let (Some(manager), Some(agent_id)) = (&self.file_lock_manager, &self.agent_id) {
            let path_str = fp.to_string_lossy().to_string();
            let read_guard = manager.acquire_read(*agent_id, &path_str).await?;

            let text = tokio::fs::read_to_string(&fp).await.map_err(|e| {
                AgentError::Io(std::io::Error::other(format!(
                    "Failed to read {}: {}",
                    path, e
                )))
            })?;

            // Get modification time and cache the read
            let modified_at = Self::get_modified_time(&fp).await?;
            let content_hash = compute_content_hash(&text);

            // Record the read in the guard
            read_guard
                .record_read(manager, modified_at, Some(content_hash))
                .await;

            let mut lines: Vec<&str> = text.lines().collect();
            if let Some(lim) = limit {
                if lim < lines.len() {
                    lines = lines[..lim].to_vec();
                }
            }
            return Ok(self.truncate_output(lines.join("\n")));
        }

        // Without locking
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

        // If file locking is enabled, validate and acquire write lock
        if let (Some(manager), Some(agent_id)) = (&self.file_lock_manager, &self.agent_id) {
            let path_str = fp.to_string_lossy().to_string();

            // First validate that file wasn't modified since last read
            manager
                .validate_read_freshness(*agent_id, &path_str)
                .await?;

            // Acquire exclusive write lock
            let write_guard = manager.acquire_write(*agent_id, &path_str).await?;

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

            // Invalidate cache after write
            write_guard.invalidate_after_write(manager).await;

            return Ok(format!("Wrote {} bytes to {}", content.len(), path));
        }

        // Without locking
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

        // If file locking is enabled, validate and acquire write lock
        if let (Some(manager), Some(agent_id)) = (&self.file_lock_manager, &self.agent_id) {
            let path_str = fp.to_string_lossy().to_string();

            // First validate that file wasn't modified since last read
            manager
                .validate_read_freshness(*agent_id, &path_str)
                .await?;

            // Acquire exclusive write lock
            let write_guard = manager.acquire_write(*agent_id, &path_str).await?;

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

            // Invalidate cache after write
            write_guard.invalidate_after_write(manager).await;

            return Ok(format!("Edited {}", path));
        }

        // Without locking
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
