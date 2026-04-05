# Tools Guide

Comprehensive documentation for Amadeus's built-in tools and how to create custom tools.

## Overview

Tools are the primary way agents interact with the external world. Each tool provides a specific capability, such as reading files, executing commands, or fetching web content.

### Tool Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                      Tool System                            │
│                                                             │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐          │
│  │  Tool       │  │  Schema     │  │  Execution  │          │
│  │  Trait      │  │  (JSON)     │  │  Pipeline   │          │
│  └─────────────┘  └─────────────┘  └─────────────┘          │
│                                                             │
│  ┌───────────────────────────────────────────────────┐      │
│  │           Policy & Safety Layer                   │      │
│  └───────────────────────────────────────────────────┘      │
└─────────────────────────────────────────────────────────────┘
```

## Built-in Tools

### bash

Execute shell commands with timeout, blocklist, and output truncation.

**Schema:**
```json
{
  "name": "bash",
  "description": "Execute a shell command",
  "input_schema": {
    "type": "object",
    "properties": {
      "command": {
        "type": "string",
        "description": "The shell command to execute"
      }
    },
    "required": ["command"]
  }
}
```

**Usage:**
```rust
// Via agent
let result = agent.run("List files in current directory").await?;

// Direct tool call
let bash = BashTool::new(
    60,                          // timeout seconds
    "/workspace".to_string(),    // working directory
    vec!["sudo".to_string()],    // blocked commands
    50000,                       // max output bytes
);
let output = bash.execute(json!({"command": "ls -la"})).await?;
```

**Features:**
- Async execution using `tokio::process::Command`
- Configurable timeout (returns `AgentError::Timeout`)
- Working directory support
- Combined stdout + stderr capture
- Command blocklist for security
- Output truncation to prevent context overflow

**Safety:**
- Blocks dangerous commands configured in `BLOCKED_COMMANDS`
- Default blocked patterns: `rm -rf /`, `sudo`, `chmod 777`
- Commands are validated before execution

---

### read_file

Read file contents with optional line limit.

**Schema:**
```json
{
  "name": "read_file",
  "description": "Read contents of a file",
  "input_schema": {
    "type": "object",
    "properties": {
      "path": {
        "type": "string",
        "description": "Path to the file (relative to workdir)"
      },
      "limit": {
        "type": "integer",
        "description": "Maximum number of lines to read"
      }
    },
    "required": ["path"]
  }
}
```

**Usage:**
```rust
// Read entire file
let result = agent.run("Read README.md").await?;

// Read first 100 lines
let output = read_file_tool.execute(json!({
    "path": "src/main.rs",
    "limit": 100
})).await?;
```

**Features:**
- Path traversal protection (stays within workdir)
- Optional line limiting
- Output truncation for large files
- UTF-8 text decoding

**Concurrency:**
When file locking is enabled:
- Acquires shared read locks
- Tracks file modification times
- Invalidates cache on external changes

---

### write_file

Create or overwrite files (creates parent directories automatically).

**Schema:**
```json
{
  "name": "write_file",
  "description": "Write content to a file",
  "input_schema": {
    "type": "object",
    "properties": {
      "path": {
        "type": "string",
        "description": "Path to the file"
      },
      "content": {
        "type": "string",
        "description": "Content to write"
      }
    },
    "required": ["path", "content"]
  }
}
```

**Usage:**
```rust
let output = write_file_tool.execute(json!({
    "path": "docs/new.md",
    "content": "# New Document\n\nContent here..."
})).await?;
```

**Features:**
- Creates parent directories automatically
- Overwrites existing files
- Path traversal protection
- UTF-8 encoding

**Safety:**
- Requires approval for sensitive paths by default
- Blocked from writing to `.env`, `.pem`, `.key` files

---

### edit_file

Make surgical changes using exact string matching.

**Schema:**
```json
{
  "name": "edit_file",
  "description": "Edit a file by replacing text",
  "input_schema": {
    "type": "object",
    "properties": {
      "path": {
        "type": "string",
        "description": "Path to the file"
      },
      "old_text": {
        "type": "string",
        "description": "Exact text to find"
      },
      "new_text": {
        "type": "string",
        "description": "Text to replace with"
      },
      "replace_all": {
        "type": "boolean",
        "description": "Replace all occurrences (default: false)"
      }
    },
    "required": ["path", "old_text", "new_text"]
  }
}
```

**Usage:**
```rust
let output = edit_file_tool.execute(json!({
    "path": "src/main.rs",
    "old_text": "fn main() {",
    "new_text": "fn main() {\n    println!(\"Hello, world!\");"
})).await?;
```

**Features:**
- Exact string matching (no regex)
- Optional replace-all mode
- Validates text exists before editing
- Returns error if text not found

**Concurrency:**
When file locking is enabled:
- Validates file wasn't modified since last read
- Acquires exclusive write locks
- Invalidates cache after write

---

### glob

Pattern-based file matching.

**Schema:**
```json
{
  "name": "glob",
  "description": "Find files matching a pattern",
  "input_schema": {
    "type": "object",
    "properties": {
      "pattern": {
        "type": "string",
        "description": "Glob pattern (e.g., '**/*.rs')"
      }
    },
    "required": ["pattern"]
  }
}
```

**Usage:**
```rust
// Find all Rust files
let output = glob_tool.execute(json!({
    "pattern": "**/*.rs"
})).await?;

// Find all markdown files in docs
let output = glob_tool.execute(json!({
    "pattern": "docs/**/*.md"
})).await?;
```

**Features:**
- Supports `**` for recursive matching
- Returns sorted file paths
- Auto-approved (safe operation)

---

### grep

Search file contents using regex.

**Schema:**
```json
{
  "name": "grep",
  "description": "Search for pattern in files",
  "input_schema": {
    "type": "object",
    "properties": {
      "pattern": {
        "type": "string",
        "description": "Regex pattern to search"
      },
      "path": {
        "type": "string",
        "description": "Directory or file to search (default: workdir)"
      },
      "glob": {
        "type": "string",
        "description": "Glob pattern to filter files"
      },
      "case_sensitive": {
        "type": "boolean",
        "description": "Case sensitive search (default: false)"
      },
      "head_limit": {
        "type": "integer",
        "description": "Limit results (default: 100)"
      }
    },
    "required": ["pattern"]
  }
}
```

**Usage:**
```rust
// Search for function definitions
let output = grep_tool.execute(json!({
    "pattern": "fn \\w+",
    "glob": "*.rs"
})).await?;

// Case-sensitive search in specific directory
let output = grep_tool.execute(json!({
    "pattern": "TODO",
    "path": "src",
    "case_sensitive": true
})).await?;
```

**Features:**
- Full regex syntax support
- File filtering with glob patterns
- Case sensitivity option
- Result limiting
- Context-aware output modes

---

### web_fetch

Fetch and convert web content.

**Schema:**
```json
{
  "name": "web_fetch",
  "description": "Fetch content from a URL",
  "input_schema": {
    "type": "object",
    "properties": {
      "url": {
        "type": "string",
        "description": "HTTP/HTTPS URL to fetch"
      },
      "format": {
        "type": "string",
        "description": "Desired format (e.g., 'text', 'markdown')"
      },
      "max_bytes": {
        "type": "integer",
        "description": "Maximum bytes to read (default: 50000)"
      },
      "timeout_secs": {
        "type": "integer",
        "description": "Request timeout (default: 20)"
      }
    },
    "required": ["url"]
  }
}
```

**Usage:**
```rust
// Fetch documentation
let output = web_fetch_tool.execute(json!({
    "url": "https://docs.rs/reqwest/latest/reqwest/",
    "format": "markdown"
})).await?;

// Fetch with custom limits
let output = web_fetch_tool.execute(json!({
    "url": "https://example.com",
    "max_bytes": 10000,
    "timeout_secs": 30
})).await?;
```

**Features:**
- HTTP/HTTPS protocol support
- Text-based content only
- Format conversion (markdown, text, etc.)
- Configurable size and timeout limits
- Only supports text-based content

**Safety:**
- Requires approval by default
- Blocks non-HTTP/HTTPS protocols
- Size and timeout limits prevent abuse

---

## Creating Custom Tools

### Basic Tool Implementation

```rust
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use amadeus::tools::Tool;
use amadeus::error::{Result, AgentError};

// 1. Define input structure
#[derive(Debug, Clone, Deserialize)]
pub struct MyToolInput {
    pub param1: String,
    pub param2: Option<i32>,
}

// 2. Create tool struct
pub struct MyTool {
    api_key: String,
    timeout_secs: u64,
}

impl MyTool {
    pub fn new(api_key: String, timeout_secs: u64) -> Self {
        Self { api_key, timeout_secs }
    }
    
    // Optional: Helper methods
    async fn call_api(&self, param: &str) -> Result<String> {
        // Your API logic here
        Ok(format!("Result for: {}", param))
    }
}

// 3. Implement Tool trait
#[async_trait]
impl Tool for MyTool {
    fn name(&self) -> &'static str {
        "my_tool"
    }

    fn schema(&self) -> &'static Value {
        // Return static JSON schema
        &serde_json::json!({
            "name": "my_tool",
            "description": "My custom tool description",
            "input_schema": {
                "type": "object",
                "properties": {
                    "param1": {
                        "type": "string",
                        "description": "First parameter"
                    },
                    "param2": {
                        "type": "integer",
                        "description": "Optional second parameter"
                    }
                },
                "required": ["param1"]
            }
        })
    }

    async fn execute(&self, input: Value) -> Result<String> {
        // Parse input
        let parsed: MyToolInput = serde_json::from_value(input)
            .map_err(|e| AgentError::ToolInput {
                tool: "my_tool".to_string(),
                reason: e.to_string(),
            })?;

        // Execute logic
        let result = self.call_api(&parsed.param1).await?;
        
        // Return result as string
        Ok(result)
    }
}
```

### Registering Custom Tools

```rust
use amadeus::{Agent, AnthropicClient};
use std::sync::Arc;

// Create your tool
let my_tool = MyTool::new(
    "your-api-key".to_string(),
    30
);

// Build agent with custom tool
let agent = Agent::builder(client, config)
    .with_default_tools()           // Include built-in tools
    .register_tool(Box::new(my_tool)) // Add your tool
    .build();

// Your tool is now available to the LLM
let result = agent.run("Use my_tool with param1='hello'").await?;
```

### Tool with File Locking Support

```rust
use amadeus::concurrency::FileLockManager;
use amadeus::core::id::AgentId;
use std::sync::Arc;

pub struct MyFileTool {
    workdir: PathBuf,
    file_lock_manager: Option<Arc<FileLockManager>>,
    agent_id: Option<AgentId>,
}

impl MyFileTool {
    pub fn new_with_locks(
        workdir: PathBuf,
        file_lock_manager: Arc<FileLockManager>,
        agent_id: AgentId,
    ) -> Self {
        Self {
            workdir,
            file_lock_manager: Some(file_lock_manager),
            agent_id: Some(agent_id),
        }
    }

    async fn safe_operation(&self, path: &str) -> Result<String> {
        // Validate path
        let fp = self.safe_path(path)?;

        // Acquire lock if available
        if let (Some(manager), Some(agent_id)) = 
           (&self.file_lock_manager, &self.agent_id) {
            let path_str = fp.to_string_lossy().to_string();
            let _guard = manager
                .acquire_read(agent_id.clone(), &path_str)
                .await?;

            // Perform operation while holding lock
            // ...
        }

        // Operation without lock
        // ...
        
        Ok("Success".to_string())
    }
}
```

### Tool with Configuration

```rust
use amadeus::agent::config::Config;

pub struct ConfigurableTool {
    config: MyToolConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MyToolConfig {
    pub api_endpoint: String,
    pub timeout_secs: u64,
    pub max_retries: u32,
}

impl ConfigurableTool {
    pub fn from_config(config: &Config) -> Self {
        // Extract tool-specific config from main config
        // or load from separate config file
        Self {
            config: MyToolConfig {
                api_endpoint: "https://api.example.com".to_string(),
                timeout_secs: config.timeout_seconds,
                max_retries: 3,
            }
        }
    }
}
```

## Tool Schema Reference

### JSON Schema Format

Tools use JSON Schema for input validation:

```json
{
  "name": "tool_name",
  "description": "Clear description for the LLM",
  "input_schema": {
    "type": "object",
    "properties": {
      "field_name": {
        "type": "string|integer|boolean|object|array",
        "description": "Field description",
        "enum": ["option1", "option2"],  // Optional
        "default": "default_value"        // Optional
      }
    },
    "required": ["field_name"]
  }
}
```

### Common Schema Patterns

**String with enum:**
```json
{
  "format": {
    "type": "string",
    "enum": ["text", "markdown", "json"]
  }
}
```

**Optional integer:**
```json
{
  "limit": {
    "type": "integer",
    "description": "Maximum results (optional)"
  }
}
```

**Nested object:**
```json
{
  "options": {
    "type": "object",
    "properties": {
      "recursive": { "type": "boolean" },
      "follow_symlinks": { "type": "boolean" }
    }
  }
}
```

## Tool Execution Flow

```
┌─────────────┐
│   LLM       │
│  Requests   │
└──────┬──────┘
       │
       ▼
┌─────────────┐
│  Tool       │
│  Registry   │
└──────┬──────┘
       │
       ▼
┌─────────────┐
│  Policy     │
│  Check      │
└──────┬──────┘
       │
       ▼
┌─────────────┐
│  Input      │
│  Validation │
└──────┬──────┘
       │
       ▼
┌─────────────┐
│  Execution  │
│  (async)    │
└──────┬──────┘
       │
       ▼
┌─────────────┐
│  Output     │
│  Formatting │
└──────┬──────┘
       │
       ▼
┌─────────────┐
│  Result     │
│  Returned   │
└─────────────┘
```

## Best Practices

### Tool Design

1. **Clear Naming**: Use descriptive tool names that LLMs can understand
2. **Detailed Schemas**: Provide thorough descriptions for all fields
3. **Error Handling**: Return meaningful error messages
4. **Idempotency**: Design tools to be safely retryable
5. **Timeouts**: Always implement timeouts for external calls

### Security

1. **Path Validation**: Always validate file paths stay within workdir
2. **Input Sanitization**: Validate and sanitize all inputs
3. **Rate Limiting**: Implement rate limits for API calls
4. **Secret Protection**: Never expose API keys in tool output
5. **Command Blocklists**: Block dangerous commands in bash-like tools

### Performance

1. **Async Operations**: Use async/await for I/O operations
2. **Streaming**: Support streaming for large responses
3. **Caching**: Cache results when appropriate
4. **Batching**: Batch operations when possible
5. **Resource Limits**: Implement size and timeout limits

## Troubleshooting

### Common Issues

**Tool not found:**
```
Error: ToolNotFound { tool: "my_tool" }
```
- Ensure tool is registered with the agent
- Check tool name matches exactly

**Invalid input:**
```
Error: ToolInput { tool: "my_tool", reason: "..." }
```
- Validate JSON schema matches input
- Check required fields are present

**Command blocked:**
```
Error: CommandBlocked { command: "..." }
```
- Check BLOCKED_COMMANDS configuration
- Review policy settings

**Path escape attempt:**
```
Error: PathEscape { path: "..." }
```
- Ensure paths don't use `..` to escape workdir
- Use relative paths within workdir

## Advanced Topics

### Tool Composition

Combine multiple tools for complex operations:

```rust
pub struct ComposedTool {
    read_tool: ReadFileTool,
    edit_tool: EditFileTool,
}

impl ComposedTool {
    pub async fn refactor(&self, path: &str, old: &str, new: &str) -> Result<String> {
        // Read file
        let content = self.read_tool.execute(json!({"path": path})).await?;
        
        // Validate change makes sense
        if !content.contains(old) {
            return Err(AgentError::TextNotFound { ... });
        }
        
        // Apply edit
        self.edit_tool.execute(json!({
            "path": path,
            "old_text": old,
            "new_text": new
        })).await?;
        
        Ok("Refactored successfully".to_string())
    }
}
```

### Tool Hooks

Execute code before/after tool calls:

```rust
use amadeus::hooks::ToolHook;

pub struct LoggingHook;

#[async_trait]
impl ToolHook for LoggingHook {
    async fn before_tool(&self, name: &str, input: &Value) {
        tracing::info!("Tool {} called with: {:?}", name, input);
    }

    async fn after_tool(&self, name: &str, result: &Result<String>) {
        match result {
            Ok(output) => tracing::info!("Tool {} succeeded", name),
            Err(e) => tracing::error!("Tool {} failed: {}", name, e),
        }
    }
}
```

### MCP Integration

Tools can be exposed via Model Context Protocol:

```rust
use amadeus::mcp::{McpServer, ToolDefinition};

let mcp_server = McpServer::new();

mcp_server.register_tool(ToolDefinition {
    name: "my_mcp_tool".to_string(),
    description: "My MCP tool".to_string(),
    input_schema: my_tool.schema().clone(),
    handler: Arc::new(|input| async {
        my_tool.execute(input).await
    }),
});
```

## See Also

- [ARCHITECTURE.md](ARCHITECTURE.md) - Overall system architecture
- [POLICY.md](POLICY.md) - Safety and approval system
- [API_GUIDE.md](API_GUIDE.md) - REST API documentation
- [CONTRIBUTING.md](../CONTRIBUTING.md) - How to contribute tools
