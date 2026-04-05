//! # Tool Registry
//!
//! Central registry for managing and discovering tools.
//!
//! ## Features
//!
//! - Dynamic tool registration
//! - O(1) tool lookup by name
//! - Lazy schema caching
//! - Thread-safe (inner tools are Send + Sync)

use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::RwLock;

use crate::agent::config::Config;
use crate::concurrency::FileLockManager;
use crate::core::id::AgentId;
use crate::error::Result;
use crate::tools::bash::BashTool;
use crate::tools::file::{EditFileTool, FileTools, ReadFileTool, WriteFileTool};
use crate::tools::glob::GlobTool;
use crate::tools::grep::GrepTool;
use crate::tools::todo::{TodoManager, TodoTool};
use crate::tools::tool_trait::Tool;
use crate::tools::web::WebFetchTool;

type ToolMap = HashMap<&'static str, Arc<dyn Tool>>;

#[derive(Clone)]
pub struct ToolRegistry {
    tools: Arc<ToolMap>,
    /// Optional file lock manager for concurrent file access control.
    file_lock_manager: Option<Arc<FileLockManager>>,
    /// Agent ID for this agent (used for file lock tracking).
    agent_id: Option<AgentId>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: Arc::new(HashMap::new()),
            file_lock_manager: None,
            agent_id: None,
        }
    }

    /// Create a new ToolRegistry with file locking enabled.
    pub fn new_with_file_locks(file_lock_manager: Arc<FileLockManager>, agent_id: AgentId) -> Self {
        Self {
            tools: Arc::new(HashMap::new()),
            file_lock_manager: Some(file_lock_manager),
            agent_id: Some(agent_id),
        }
    }

    pub fn with_defaults(config: &Config) -> Self {
        Self::with_defaults_and_todo(config, Arc::new(RwLock::new(TodoManager::new())))
    }

    pub fn with_defaults_and_todo(config: &Config, todo_manager: Arc<RwLock<TodoManager>>) -> Self {
        Self::with_defaults_and_todo_with_locks(config, todo_manager, None, None)
    }

    /// Create default tools with optional file locking.
    pub fn with_defaults_and_todo_with_locks(
        config: &Config,
        todo_manager: Arc<RwLock<TodoManager>>,
        file_lock_manager: Option<Arc<FileLockManager>>,
        agent_id: Option<AgentId>,
    ) -> Self {
        let file_tools =
            FileTools::from_config_with_locks(config, file_lock_manager.clone(), agent_id);

        Self {
            tools: Arc::new(HashMap::new()),
            file_lock_manager,
            agent_id,
        }
        .register(Box::new(BashTool::from_config(config)))
        .register(Box::new(ReadFileTool::new(file_tools.clone())))
        .register(Box::new(WriteFileTool::new(file_tools.clone())))
        .register(Box::new(EditFileTool::new(file_tools)))
        .register(Box::new(GlobTool::from_config(config)))
        .register(Box::new(GrepTool::from_config(config)))
        .register(Box::new(TodoTool::new(todo_manager)))
        .register(Box::new(WebFetchTool::from_config(config)))
    }

    /// Set the file lock manager for this registry.
    pub fn with_file_locks(mut self, manager: Arc<FileLockManager>, agent_id: AgentId) -> Self {
        self.file_lock_manager = Some(manager);
        self.agent_id = Some(agent_id);
        self
    }

    /// Get the file lock manager if available.
    pub fn file_lock_manager(&self) -> Option<&Arc<FileLockManager>> {
        self.file_lock_manager.as_ref()
    }

    /// Get the agent ID if available.
    pub fn agent_id(&self) -> Option<&AgentId> {
        self.agent_id.as_ref()
    }

    pub fn with_sub_agent_child_defaults(config: &Config) -> Self {
        Self::with_sub_agent_child_defaults_recursive(config, None)
    }

    pub fn with_sub_agent_child_defaults_recursive(
        config: &Config,
        subagent_tool: Option<Arc<dyn Tool>>,
    ) -> Self {
        let file_tools = FileTools::from_config(config);
        let todo_manager = Arc::new(RwLock::new(TodoManager::new()));
        let registry = Self::new()
            .register(Box::new(BashTool::from_config(config)))
            .register(Box::new(ReadFileTool::new(file_tools.clone())))
            .register(Box::new(WriteFileTool::new(file_tools.clone())))
            .register(Box::new(EditFileTool::new(file_tools)))
            .register(Box::new(GlobTool::from_config(config)))
            .register(Box::new(GrepTool::from_config(config)))
            .register(Box::new(TodoTool::new(todo_manager)))
            .register(Box::new(WebFetchTool::from_config(config)));

        if let Some(tool) = subagent_tool {
            registry.register_arc(tool)
        } else {
            registry
        }
    }

    pub fn register(mut self, tool: Box<dyn Tool>) -> Self {
        let mut tools: ToolMap = (*self.tools).clone();
        tools.insert(tool.name(), Arc::from(tool));
        self.tools = Arc::new(tools);
        self
    }

    pub fn get(&self, name: &str) -> Option<Arc<dyn Tool>> {
        self.tools.get(name).cloned()
    }

    pub fn schemas(&self) -> Vec<&'static Value> {
        self.tools.values().map(|t| t.schema()).collect()
    }

    pub fn names(&self) -> Vec<&'static str> {
        self.tools.keys().copied().collect()
    }

    pub fn len(&self) -> usize {
        self.tools.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }

    pub async fn execute(&self, name: &str, input: Value) -> Result<String> {
        match self.get(name) {
            Some(tool) => tool.execute(input).await,
            None => Err(crate::error::AgentError::ToolNotFound(name.to_string())),
        }
    }

    pub fn filter_by_name(self, allowed: &[String]) -> Self {
        let allowed_set: std::collections::HashSet<&str> =
            allowed.iter().map(|s| s.as_str()).collect();

        let filtered: ToolMap = self
            .tools
            .iter()
            .filter(|(name, _)| allowed_set.contains(*name))
            .map(|(name, tool)| (*name, tool.clone()))
            .collect();

        Self {
            tools: Arc::new(filtered),
            file_lock_manager: self.file_lock_manager.clone(),
            agent_id: self.agent_id,
        }
    }

    pub fn register_arc(mut self, tool: Arc<dyn Tool>) -> Self {
        let mut tools: ToolMap = (*self.tools).clone();
        tools.insert(tool.name(), tool);
        self.tools = Arc::new(tools);
        self
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for ToolRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ToolRegistry")
            .field("tools", &self.tools.keys().collect::<Vec<_>>())
            .finish()
    }
}
