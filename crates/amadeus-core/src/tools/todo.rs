// @amadeus-header
// summary: Tool implementation and support code for todo.
// layer: tools
// status: active
// feature_flags: none
// provides:
// - module: crate::tools::todo
// - type: crate::tools::todo::TodoStatus
// - type: crate::tools::todo::TodoItem
// - type: crate::tools::todo::TodoManager
// - type: crate::tools::todo::TodoTool
// - tool: todo
// uses:
// - module: crate::error
// - module: crate::tools::schema::todo_tool
// - module: crate::tools::tool_trait::Tool
// - protocol: serde serialization
// - format: JSON values
// invariants:
// - Declared tool interfaces stay aligned with runtime behavior and schema.
// side_effects: none
// tests:
// - tests/todo_test.rs
// @end-amadeus-header

//! # Todo Tool
//!
//! Stateful todo tracking for multi-step agent tasks.

use std::fmt;
use std::sync::Arc;
use std::sync::RwLock;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{AgentError, Result};
use crate::tools::schema::todo_tool;
use crate::tools::tool_trait::Tool;

const MAX_TODOS: usize = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TodoStatus {
    #[default]
    Pending,
    InProgress,
    Completed,
}

impl TodoStatus {
    fn marker(self) -> &'static str {
        match self {
            Self::Pending => "[ ]",
            Self::InProgress => "[>]",
            Self::Completed => "[x]",
        }
    }
}

impl fmt::Display for TodoStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let status = match self {
            Self::Pending => "pending",
            Self::InProgress => "in_progress",
            Self::Completed => "completed",
        };

        f.write_str(status)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TodoItem {
    pub id: String,
    pub text: String,
    pub status: TodoStatus,
}

#[derive(Debug, Clone, Deserialize)]
struct TodoItemInput {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    text: String,
    #[serde(default)]
    status: Option<TodoStatus>,
}

#[derive(Debug, Clone, Deserialize)]
struct TodoUpdateInput {
    items: Vec<TodoItemInput>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TodoManager {
    items: Vec<TodoItem>,
}

impl TodoManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn items(&self) -> &[TodoItem] {
        &self.items
    }

    pub fn cloned_items(&self) -> Vec<TodoItem> {
        self.items.clone()
    }

    pub fn replace_items(&mut self, items: Vec<TodoItem>) {
        self.items = items;
    }

    pub fn update(&mut self, items: Vec<TodoItem>) -> std::result::Result<String, String> {
        if items.len() > MAX_TODOS {
            return Err(format!("Max {} todos allowed", MAX_TODOS));
        }

        let mut validated = Vec::with_capacity(items.len());
        let mut in_progress_count = 0;

        for (index, item) in items.into_iter().enumerate() {
            let fallback_id = (index + 1).to_string();
            let item_id = item.id.trim();
            let item_id = if item_id.is_empty() {
                fallback_id.as_str()
            } else {
                item_id
            };

            let text = item.text.trim();
            if text.is_empty() {
                return Err(format!("Item {}: text required", item_id));
            }

            if item.status == TodoStatus::InProgress {
                in_progress_count += 1;
            }

            validated.push(TodoItem {
                id: item_id.to_string(),
                text: text.to_string(),
                status: item.status,
            });
        }

        if in_progress_count > 1 {
            return Err("Only one task can be in_progress at a time".to_string());
        }

        self.items = validated;
        Ok(self.render())
    }

    pub fn render(&self) -> String {
        if self.items.is_empty() {
            return "No todos.".to_string();
        }

        let mut lines: Vec<String> = self
            .items
            .iter()
            .map(|item| format!("{} #{}: {}", item.status.marker(), item.id, item.text))
            .collect();

        let completed = self
            .items
            .iter()
            .filter(|item| item.status == TodoStatus::Completed)
            .count();

        lines.push(String::new());
        lines.push(format!("({}/{} completed)", completed, self.items.len()));
        lines.join("\n")
    }
}

pub struct TodoTool {
    manager: Arc<RwLock<TodoManager>>,
}

impl TodoTool {
    pub fn new(manager: Arc<RwLock<TodoManager>>) -> Self {
        Self { manager }
    }

    pub fn manager(&self) -> Arc<RwLock<TodoManager>> {
        Arc::clone(&self.manager)
    }

    fn parse_items(parsed: TodoUpdateInput) -> Vec<TodoItem> {
        parsed
            .items
            .into_iter()
            .enumerate()
            .map(|(index, item)| TodoItem {
                id: item.id.unwrap_or_else(|| (index + 1).to_string()),
                text: item.text,
                status: item.status.unwrap_or_default(),
            })
            .collect()
    }
}

#[async_trait]
impl Tool for TodoTool {
    fn name(&self) -> &'static str {
        "todo"
    }

    fn schema(&self) -> &'static Value {
        todo_tool()
    }

    async fn execute(&self, input: Value) -> Result<String> {
        let parsed: TodoUpdateInput =
            serde_json::from_value(input).map_err(|e| AgentError::ToolInput {
                tool: "todo".to_string(),
                reason: e.to_string(),
            })?;

        let items = Self::parse_items(parsed);
        let mut manager = self
            .manager
            .write()
            .map_err(|_| AgentError::InvalidResponse("Todo state lock poisoned".to_string()))?;
        manager
            .update(items)
            .map_err(|reason| AgentError::ToolInput {
                tool: "todo".to_string(),
                reason,
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn render_empty_todos() {
        let manager = TodoManager::new();
        assert_eq!(manager.render(), "No todos.");
    }

    #[test]
    fn update_and_render_todos() {
        let mut manager = TodoManager::new();
        let output = manager
            .update(vec![
                TodoItem {
                    id: "1".to_string(),
                    text: "Plan feature".to_string(),
                    status: TodoStatus::Completed,
                },
                TodoItem {
                    id: "2".to_string(),
                    text: "Implement feature".to_string(),
                    status: TodoStatus::InProgress,
                },
            ])
            .unwrap();

        assert_eq!(
            output,
            "[x] #1: Plan feature\n[>] #2: Implement feature\n\n(1/2 completed)"
        );
    }

    #[test]
    fn rejects_multiple_in_progress_items() {
        let mut manager = TodoManager::new();
        let error = manager
            .update(vec![
                TodoItem {
                    id: "1".to_string(),
                    text: "First".to_string(),
                    status: TodoStatus::InProgress,
                },
                TodoItem {
                    id: "2".to_string(),
                    text: "Second".to_string(),
                    status: TodoStatus::InProgress,
                },
            ])
            .unwrap_err();

        assert_eq!(error, "Only one task can be in_progress at a time");
    }

    #[test]
    fn rejects_empty_text() {
        let mut manager = TodoManager::new();
        let error = manager
            .update(vec![TodoItem {
                id: "1".to_string(),
                text: "   ".to_string(),
                status: TodoStatus::Pending,
            }])
            .unwrap_err();

        assert_eq!(error, "Item 1: text required");
    }

    #[tokio::test]
    async fn tool_defaults_missing_id_and_status() {
        let tool = TodoTool::new(Arc::new(RwLock::new(TodoManager::new())));
        let output = tool
            .execute(json!({
                "items": [
                    {"text": "Plan todo feature"}
                ]
            }))
            .await
            .unwrap();

        assert_eq!(output, "[ ] #1: Plan todo feature\n\n(0/1 completed)");
    }

    #[tokio::test]
    async fn tool_rejects_more_than_twenty_items() {
        let tool = TodoTool::new(Arc::new(RwLock::new(TodoManager::new())));
        let items: Vec<Value> = (1..=21)
            .map(|index| {
                json!({
                    "id": index.to_string(),
                    "text": format!("Task {}", index),
                    "status": "pending"
                })
            })
            .collect();

        let error = tool.execute(json!({ "items": items })).await.unwrap_err();
        assert!(matches!(error, AgentError::ToolInput { ref tool, .. } if tool == "todo"));
        assert!(error.to_string().contains("Max 20 todos allowed"));
    }
}
