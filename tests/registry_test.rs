use claude_agent::error::AgentError;
use claude_agent::tools::bash::BashTool;
use claude_agent::tools::registry::ToolRegistry;
use claude_agent::tools::tool_trait::Tool;
use serde_json::json;
use std::sync::{Arc, LazyLock};

static MOCK_SCHEMA: LazyLock<serde_json::Value> = LazyLock::new(|| {
    json!({
        "name": "mock",
        "description": "A mock tool"
    })
});

struct MockTool {
    name: &'static str,
}

impl MockTool {
    fn new(name: &'static str) -> Self {
        Self { name }
    }
}

#[async_trait::async_trait]
impl Tool for MockTool {
    fn name(&self) -> &'static str {
        self.name
    }

    fn schema(&self) -> &'static serde_json::Value {
        &MOCK_SCHEMA
    }

    async fn execute(&self, _input: serde_json::Value) -> claude_agent::error::Result<String> {
        Ok(format!("executed {}", self.name))
    }
}

#[test]
fn test_registry_new() {
    let registry = ToolRegistry::new();
    assert!(registry.is_empty());
    assert_eq!(registry.len(), 0);
}

#[test]
fn test_registry_default() {
    let registry = ToolRegistry::default();
    assert!(registry.is_empty());
}

#[test]
fn test_registry_register() {
    let registry = ToolRegistry::new()
        .register(Box::new(MockTool::new("tool1")));

    assert!(!registry.is_empty());
    assert_eq!(registry.len(), 1);
    assert!(registry.get("tool1").is_some());
}

#[test]
fn test_registry_register_multiple() {
    let registry = ToolRegistry::new()
        .register(Box::new(MockTool::new("tool1")))
        .register(Box::new(MockTool::new("tool2")))
        .register(Box::new(MockTool::new("tool3")));

    assert_eq!(registry.len(), 3);
    assert!(registry.get("tool1").is_some());
    assert!(registry.get("tool2").is_some());
    assert!(registry.get("tool3").is_some());
}

#[test]
fn test_registry_get_nonexistent() {
    let registry = ToolRegistry::new();
    assert!(registry.get("nonexistent").is_none());
}

#[test]
fn test_registry_names() {
    let registry = ToolRegistry::new()
        .register(Box::new(MockTool::new("alpha")))
        .register(Box::new(MockTool::new("beta")));

    let mut names = registry.names();
    names.sort();
    assert_eq!(names, vec!["alpha", "beta"]);
}

#[test]
fn test_registry_schemas() {
    let registry = ToolRegistry::new()
        .register(Box::new(MockTool::new("tool1")));

    let schemas = registry.schemas();
    assert_eq!(schemas.len(), 1);
}

#[tokio::test]
async fn test_registry_execute() {
    let registry = ToolRegistry::new()
        .register(Box::new(MockTool::new("test_tool")));

    let result = registry.execute("test_tool", json!({})).await.unwrap();
    assert_eq!(result, "executed test_tool");
}

#[tokio::test]
async fn test_registry_execute_nonexistent() {
    let registry = ToolRegistry::new();

    let result = registry.execute("nonexistent", json!({})).await;
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), AgentError::ToolNotFound(_)));
}

#[test]
fn test_registry_filter_by_name() {
    let registry = ToolRegistry::new()
        .register(Box::new(MockTool::new("bash")))
        .register(Box::new(MockTool::new("read_file")))
        .register(Box::new(MockTool::new("write_file")))
        .register(Box::new(MockTool::new("edit_file")));

    let filtered = registry.filter_by_name(&["bash".to_string(), "read_file".to_string()]);

    assert_eq!(filtered.len(), 2);
    assert!(filtered.get("bash").is_some());
    assert!(filtered.get("read_file").is_some());
    assert!(filtered.get("write_file").is_none());
    assert!(filtered.get("edit_file").is_none());
}

#[test]
fn test_registry_filter_empty_allowed() {
    let registry = ToolRegistry::new()
        .register(Box::new(MockTool::new("bash")))
        .register(Box::new(MockTool::new("read_file")));

    let filtered = registry.filter_by_name(&[]);
    assert!(filtered.is_empty());
}

#[test]
fn test_registry_filter_nonexistent_tools() {
    let registry = ToolRegistry::new()
        .register(Box::new(MockTool::new("bash")));

    let filtered = registry.filter_by_name(&["nonexistent".to_string()]);
    assert!(filtered.is_empty());
}

#[test]
fn test_registry_clone() {
    let registry = ToolRegistry::new()
        .register(Box::new(MockTool::new("tool1")));

    let cloned = registry.clone();
    assert_eq!(cloned.len(), 1);
    assert!(cloned.get("tool1").is_some());
}

#[test]
fn test_registry_register_arc() {
    let tool = Arc::new(MockTool::new("arc_tool"));
    let registry = ToolRegistry::new()
        .register_arc(tool);

    assert_eq!(registry.len(), 1);
    assert!(registry.get("arc_tool").is_some());
}

#[test]
fn test_registry_debug() {
    let registry = ToolRegistry::new()
        .register(Box::new(MockTool::new("tool1")));

    let debug_str = format!("{:?}", registry);
    assert!(debug_str.contains("ToolRegistry"));
    assert!(debug_str.contains("tool1"));
}

#[tokio::test]
async fn test_registry_with_real_bash_tool() {
    let registry = ToolRegistry::new()
        .register(Box::new(BashTool::new(30, "/tmp".to_string(), vec![], 50_000)));

    assert_eq!(registry.len(), 1);
    assert!(registry.get("bash").is_some());

    let result = registry.execute("bash", json!({"command": "echo hello"})).await.unwrap();
    assert!(result.contains("hello"));
}

#[test]
fn test_registry_replace_tool() {
    let registry = ToolRegistry::new()
        .register(Box::new(MockTool::new("tool1")))
        .register(Box::new(MockTool::new("tool1")));

    assert_eq!(registry.len(), 1);
}
