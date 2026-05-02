//! Memory tool — LLM-callable persistent memory operations.

use std::sync::Arc;
use std::sync::RwLock;

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::Value;

use crate::context::memory::{MemoryEntry, MemoryRegistry};
use crate::error::{AgentError, Result};
use crate::tools::tool_trait::Tool;

#[derive(Debug, Clone, Deserialize)]
struct MemoryInput {
    operation: String,
    #[serde(default)]
    key: Option<String>,
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    query: Option<String>,
}

fn memory_schema() -> &'static Value {
    static SCHEMA: std::sync::OnceLock<Value> = std::sync::OnceLock::new();
    SCHEMA.get_or_init(|| {
        serde_json::json!({
            "name": "memory",
            "description": "Store, recall, search, list, or delete persistent memories. Use this to remember important information the user shares, decisions made, or context that should persist across conversations.",
            "parameters": {
                "type": "object",
                "properties": {
                    "operation": {
                        "type": "string",
                        "enum": ["store", "recall", "search", "list", "delete"],
                        "description": "The memory operation to perform."
                    },
                    "key": {
                        "type": "string",
                        "description": "A unique key for the memory (required for store, recall, delete)."
                    },
                    "content": {
                        "type": "string",
                        "description": "The content to store (required for store)."
                    },
                    "query": {
                        "type": "string",
                        "description": "Search keyword(s) to find matching memories (for search)."
                    }
                },
                "required": ["operation"]
            }
        })
    })
}

pub struct MemoryTool {
    registry: Arc<RwLock<MemoryRegistry>>,
}

impl MemoryTool {
    pub fn new(registry: Arc<RwLock<MemoryRegistry>>) -> Self {
        Self { registry }
    }

    fn do_store(&self, key: String, content: String) -> Result<String> {
        let entry = MemoryEntry::new(&key, &content, "llm");
        let registry = self
            .registry
            .read()
            .map_err(|e| AgentError::InvalidResponse(format!("Memory registry lock: {}", e)))?;
        // Store in each writable provider
        let mut stored = 0;
        for provider in registry.list_providers() {
            if provider.writable() {
                provider
                    .store(entry.clone())
                    .map_err(|e| AgentError::ToolInput {
                        tool: "memory".into(),
                        reason: e.to_string(),
                    })?;
                stored += 1;
            }
        }
        Ok(format!("Stored memory '{}' ({} provider(s) updated)", key, stored))
    }

    fn do_recall(&self, key: &str) -> Result<String> {
        let registry = self
            .registry
            .read()
            .map_err(|e| AgentError::InvalidResponse(format!("Memory registry lock: {}", e)))?;
        let entries = registry.load_all();
        let matches: Vec<_> = entries.iter().filter(|e| e.key == key).collect();
        if matches.is_empty() {
            Ok(format!("No memory found with key '{}'", key))
        } else {
            let result: Vec<String> = matches
                .iter()
                .map(|e| format!("[{}] {}", e.key, e.content))
                .collect();
            Ok(result.join("\n"))
        }
    }

    fn do_search(&self, query: &str) -> Result<String> {
        let registry = self
            .registry
            .read()
            .map_err(|e| AgentError::InvalidResponse(format!("Memory registry lock: {}", e)))?;
        let entries = registry.load_all();
        let query_lower = query.to_lowercase();
        let matches: Vec<_> = entries
            .iter()
            .filter(|e| {
                e.key.to_lowercase().contains(&query_lower)
                    || e.content.to_lowercase().contains(&query_lower)
            })
            .collect();
        if matches.is_empty() {
            Ok(format!("No memories found matching '{}'", query))
        } else {
            let result: Vec<String> = matches
                .iter()
                .map(|e| format!("[{}] {}", e.key, e.content))
                .collect();
            Ok(format!(
                "Found {} matching memories:\n{}",
                matches.len(),
                result.join("\n")
            ))
        }
    }

    fn do_list(&self) -> Result<String> {
        let registry = self
            .registry
            .read()
            .map_err(|e| AgentError::InvalidResponse(format!("Memory registry lock: {}", e)))?;
        let entries = registry.load_all();
        if entries.is_empty() {
            Ok("No memories stored.".to_string())
        } else {
            let keys: Vec<String> = entries.iter().map(|e| e.key.clone()).collect();
            Ok(format!(
                "{} memories stored: {}",
                keys.len(),
                keys.join(", ")
            ))
        }
    }

    fn do_delete(&self, key: &str) -> Result<String> {
        let registry = self
            .registry
            .read()
            .map_err(|e| AgentError::InvalidResponse(format!("Memory registry lock: {}", e)))?;
        let mut deleted = 0;
        for provider in registry.list_providers() {
            if provider.writable() {
                match provider.delete(key) {
                    Ok(()) => deleted += 1,
                    Err(e) => {
                        return Err(AgentError::ToolInput {
                            tool: "memory".into(),
                            reason: e.to_string(),
                        })
                    }
                }
            }
        }
        if deleted == 0 {
            Ok(format!(
                "No writable provider supports delete for '{}'.",
                key
            ))
        } else {
            Ok(format!("Deleted memory '{}'", key))
        }
    }
}

#[async_trait]
impl Tool for MemoryTool {
    fn name(&self) -> &'static str {
        "memory"
    }

    fn schema(&self) -> &'static Value {
        memory_schema()
    }

    async fn execute(&self, input: Value) -> Result<String> {
        let parsed: MemoryInput = serde_json::from_value(input).map_err(|e| {
            AgentError::ToolInput {
                tool: "memory".to_string(),
                reason: e.to_string(),
            }
        })?;

        match parsed.operation.as_str() {
            "store" => {
                let key = parsed.key.ok_or_else(|| AgentError::ToolInput {
                    tool: "memory".into(),
                    reason: "key is required for store".into(),
                })?;
                let content = parsed.content.ok_or_else(|| AgentError::ToolInput {
                    tool: "memory".into(),
                    reason: "content is required for store".into(),
                })?;
                self.do_store(key, content)
            }
            "recall" => {
                let key = parsed.key.ok_or_else(|| AgentError::ToolInput {
                    tool: "memory".into(),
                    reason: "key is required for recall".into(),
                })?;
                self.do_recall(&key)
            }
            "search" => {
                let query = parsed.query.ok_or_else(|| AgentError::ToolInput {
                    tool: "memory".into(),
                    reason: "query is required for search".into(),
                })?;
                self.do_search(&query)
            }
            "list" => self.do_list(),
            "delete" => {
                let key = parsed.key.ok_or_else(|| AgentError::ToolInput {
                    tool: "memory".into(),
                    reason: "key is required for delete".into(),
                })?;
                self.do_delete(&key)
            }
            other => Err(AgentError::ToolInput {
                tool: "memory".into(),
                reason: format!(
                    "Unknown operation '{}'. Use store, recall, search, list, or delete.",
                    other
                ),
            }
            .into()),
        }
    }
}
