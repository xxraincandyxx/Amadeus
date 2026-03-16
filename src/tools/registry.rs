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
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: Arc::new(HashMap::new()),
        }
    }

    pub fn with_defaults(config: &Config) -> Self {
        Self::with_defaults_and_todo(config, Arc::new(RwLock::new(TodoManager::new())))
    }

    pub fn with_defaults_and_todo(config: &Config, todo_manager: Arc<RwLock<TodoManager>>) -> Self {
        let file_tools = FileTools::from_config(config);

        Self::new()
            .register(Box::new(BashTool::from_config(config)))
            .register(Box::new(ReadFileTool::new(file_tools.clone())))
            .register(Box::new(WriteFileTool::new(file_tools.clone())))
            .register(Box::new(EditFileTool::new(file_tools)))
            .register(Box::new(GlobTool::from_config(config)))
            .register(Box::new(GrepTool::from_config(config)))
            .register(Box::new(TodoTool::new(todo_manager)))
            .register(Box::new(WebFetchTool::from_config(config)))
    }

    pub fn with_sub_agnet_child_defaults(config: &Config) -> Self {
        Self::with_sub_agnet_child_defaults_recursive(config, None)
    }

    pub fn with_sub_agnet_child_defaults_recursive(
        config: &Config,
        subagent_tool: Option<Arc<dyn Tool>>,
    ) -> Self {
        let file_tools = FileTools::from_config(config);
        let registry = Self::new()
            .register(Box::new(BashTool::from_config(config)))
            .register(Box::new(ReadFileTool::new(file_tools.clone())))
            .register(Box::new(WriteFileTool::new(file_tools.clone())))
            .register(Box::new(EditFileTool::new(file_tools)));

        if let Some(tool) = subagent_tool {
            registry.register_arc(tool)
        } else {
            registry
        }
    }

    pub fn register(self, tool: Box<dyn Tool>) -> Self {
        let mut tools: ToolMap = (*self.tools).clone();
        tools.insert(tool.name(), Arc::from(tool));
        Self {
            tools: Arc::new(tools),
        }
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
        }
    }

    pub fn register_arc(self, tool: Arc<dyn Tool>) -> Self {
        let mut tools: ToolMap = (*self.tools).clone();
        tools.insert(tool.name(), tool);
        Self {
            tools: Arc::new(tools),
        }
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
