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

use crate::error::Result;
use crate::tools::tool_trait::Tool;

pub struct ToolRegistry {
    tools: HashMap<&'static str, Box<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    pub fn register(mut self, tool: Box<dyn Tool>) -> Self {
        self.tools.insert(tool.name(), tool);
        self
    }

    pub fn get(&self, name: &str) -> Option<&dyn Tool> {
        self.tools.get(name).map(|t| t.as_ref())
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
