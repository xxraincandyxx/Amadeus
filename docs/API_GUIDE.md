# REST API Guide

Complete documentation for the Amadeus HTTP API server.

## Overview

Amadeus provides a REST API server built with Axum that allows you to interact with AI agents programmatically. The server supports:

- Stateless chat requests
- Direct tool execution
- Server-Sent Events (SSE) streaming
- Multi-agent task orchestration
- Session management
- Configuration management
- Approval workflow

Current routing behavior is worth calling out explicitly:

- `POST /chat` and `POST /tasks` dispatch through the shared `AgentOrchestrator`.
- `GET /stream` creates a fresh `Agent` and streams events directly.
- `POST /execute` runs `BashTool` directly and does not enter the normal agent loop.
- Some `/agents/*` routes exist today but are still partially provisional.

## Quick Start

### Starting the Server

```bash
# Run with default port (3000)
cargo run --features full -- --server

# Run with custom port
cargo run --features full -- --server 8080
```

### Basic Health Check

```bash
curl http://localhost:3000/health
```

Response:
```json
{
  "status": "ok",
  "version": "0.1.0"
}
```

## Core Endpoints

### Health Check

**GET `/health`**

Check if the server is running.

```bash
curl http://localhost:3000/health
```

**Response:**
```json
{
  "status": "ok",
  "version": "0.1.0"
}
```

---

### Chat

**POST `/chat`**

Send a message to the agent and receive a response.

**Request:**
```bash
curl -X POST http://localhost:3000/chat \
  -H "Content-Type: application/json" \
  -d '{
    "message": "List all Rust files in the current directory"
  }'
```

**Request Body:**
```json
{
  "message": "string (required)"
}
```

Note: the shared request type still contains `timeout_secs` and `stream`, but the current `/chat` handler only uses `message`. Use `/stream` for SSE responses.

**Response:**
```json
{
  "content": "I found 5 Rust files:\n1. src/main.rs\n2. src/lib.rs\n...",
  "tool_calls": [
    {
      "name": "glob",
      "input": { "pattern": "**/*.rs" },
      "output": "src/main.rs\nsrc/lib.rs\n..."
    }
  ],
  "stop_reason": "end_turn"
}
```

**Stop Reasons:**
- `end_turn` - Agent finished normally
- `tool_use` - Agent waiting for tool execution
- `max_tokens` - Token limit reached

---

### Execute

**POST `/execute`**

Execute a bash command directly without LLM involvement.

**Request:**
```bash
curl -X POST http://localhost:3000/execute \
  -H "Content-Type: application/json" \
  -d '{
    "command": "ls -la src/",
    "timeout_secs": 30
  }'
```

**Request Body:**
```json
{
  "command": "string (required)",
  "timeout_secs": 30         // optional, default: 30
}
```

**Response:**
```json
{
  "output": "total 24\ndrwxr-xr-x  5 user staff  160 Mar 20 12:00 .\n...",
  "exit_code": 0,
  "timed_out": false
}
```

**Exit Codes:**
- `0` - Success
- `1-255` - Error (command-specific)
- `-1` - Command failed to start or was killed

---

### Streaming

**GET `/stream`**

Receive real-time Server-Sent Events (SSE) for agent execution.

**Request:**
```bash
curl -N http://localhost:3000/stream \
  -H "Content-Type: application/json" \
  -d '{
    "message": "Create a hello world program",
    "timeout_secs": 60
  }'
```

**Note:** Use `-N` (no-buffer) to see events in real-time.

**Events:**

The server sends different event types via SSE:

**Token Usage Event:**
```
event: token_usage
data: {"input_tokens": 150, "output_tokens": 200, "total_tokens": 350, "context_percent": 25}
```

**Tool Progress Event:**
```
event: tool_progress
data: {"id": "tool_1", "message": "Reading file...", "percent": 50}
```

**Approval Request Event:**
```
event: approval_request
data: {"id": "approval_1", "tool": "bash", "action": "Execute command", "input": {"command": "rm -rf test/"}}
```

**Text Delta Event:**
```
event: text_delta
data: {"delta": "Hello"}
```

**Done Event:**
```
event: done
data: {"result": "Complete!", "stop_reason": "end_turn"}
```

---

### Tasks (Multi-Agent)

**POST `/tasks`**

Dispatch a task to the multi-agent supervisor.

**Request:**
```bash
curl -X POST http://localhost:3000/tasks \
  -H "Content-Type: application/json" \
  -d '{
    "id": "task-001",
    "prompt": "Analyze the codebase and create a summary",
    "capabilities": ["code_analysis", "documentation"]
  }'
```

**Request Body:**
```json
{
  "id": "string (required)",
  "prompt": "string (required)",
  "capabilities": ["code_analysis"]  // optional
}
```

**Response:**
```json
{
  "task_id": "task-001",
  "worker_id": "worker-abc123",
  "success": true,
  "output": "Codebase analysis complete...\n",
  "error": null,
  "duration_ms": 15420
}
```

---

## Session Management

### List Sessions

**GET `/sessions`**

List all saved conversation sessions.

```bash
curl http://localhost:3000/sessions
```

**Response:**
```json
{
  "sessions": [
    {
      "id": "2024-03-20T12-00-00.json",
      "timestamp": "2024-03-20T12:00:00Z",
      "model": "claude-sonnet-4-5-20250929",
      "total_tokens": 1250,
      "tool_calls": 8,
      "duration_ms": 45000,
      "message_count": 12,
      "todo_count": 3
    }
  ]
}
```

---

### Get Session Details

**GET `/sessions/:id`**

Get full details of a specific session.

```bash
curl http://localhost:3000/sessions/2024-03-20T12-00-00.json
```

**Response:**
```json
{
  "id": "2024-03-20T12-00-00.json",
  "timestamp": "2024-03-20T12:00:00Z",
  "model": "claude-sonnet-4-5-20250929",
  "system_prompt": "You are a helpful assistant...",
  "history": [
    {
      "role": "user",
      "content": "List files in src/"
    },
    {
      "role": "assistant",
      "content": "I found the following files..."
    }
  ],
  "todos": [
    {
      "id": "1",
      "text": "Review code",
      "status": "completed"
    }
  ],
  "stats": {
    "total_tokens": 1250,
    "tool_calls": 8,
    "duration_ms": 45000
  }
}
```

---

### Restore Session

**POST `/sessions/:id/restore`**

Restore a session into the current conversation history.

```bash
curl -X POST http://localhost:3000/sessions/2024-03-20T12-00-00.json/restore \
  -H "Content-Type: application/json" \
  -d '{
    "clear_history": true
  }'
```

**Request Body:**
```json
{
  "clear_history": true  // optional, default: false
}
```

**Response:**
```json
{
  "success": true,
  "message_count": 12
}
```

---

## Configuration

### Get Config

**GET `/config`**

Get current agent configuration.

```bash
curl http://localhost:3000/config
```

**Response:**
```json
{
  "working_dir": "/path/to/project",
  "model": "claude-sonnet-4-5-20250929",
  "max_tokens": 4096,
  "context_window_size": 200000,
  "tool_timeout_secs": 120,
  "require_approval": true,
  "shell_profile": "bash",
  "session_log_dir": "./logs"
}
```

---

### Update Config

**PATCH `/config`**

Update agent configuration settings.

```bash
curl -X PATCH http://localhost:3000/config \
  -H "Content-Type: application/json" \
  -d '{
    "model": "claude-3-5-sonnet-latest",
    "max_tokens": 8192,
    "tool_timeout_secs": 180
  }'
```

**Request Body (all optional):**
```json
{
  "model": "string",
  "max_tokens": 4096,
  "context_window_size": 200000,
  "tool_timeout_secs": 120,
  "require_approval": true
}
```

**Response:**
```json
{
  "success": true,
  "config": {
    "working_dir": "/path/to/project",
    "model": "claude-3-5-sonnet-latest",
    "max_tokens": 8192,
    "context_window_size": 200000,
    "tool_timeout_secs": 180,
    "require_approval": true,
    "shell_profile": "bash",
    "session_log_dir": "./logs"
  }
}
```

---

## Conversation History

### Get History

**GET `/history`**

Get the current conversation history.

```bash
curl http://localhost:3000/history
```

**Response:**
```json
{
  "messages": [
    {
      "role": "user",
      "content": "List files in src/"
    },
    {
      "role": "assistant",
      "content": "I found the following files..."
    },
    {
      "role": "user",
      "content": "Read main.rs"
    }
  ],
  "total": 3
}
```

---

## Skills

### List Skills

**GET `/skills`**

List available prompt templates/skills.

```bash
curl http://localhost:3000/skills
```

**Response:**
```json
{
  "skills": [
    {
      "name": "code_review",
      "description": "Perform code review on provided files"
    },
    {
      "name": "documentation",
      "description": "Generate documentation for code"
    },
    {
      "name": "testing",
      "description": "Write tests for code"
    }
  ]
}
```

---

## Approval Workflow

### List Pending Approvals

**GET `/approvals`**

List all pending approval requests.

```bash
curl http://localhost:3000/approvals
```

**Response:**
```json
{
  "approvals": [
    {
      "id": "approval_1",
      "tool": "bash",
      "action": "Execute command",
      "input": {
        "command": "rm -rf test/"
      }
    }
  ]
}
```

---

### Submit Approval Decision

**POST `/approvals/:id`**

Submit a decision for a pending approval request.

```bash
curl -X POST http://localhost:3000/approvals/approval_1 \
  -H "Content-Type: application/json" \
  -d '{
    "decision": "approve",
    "reason": null
  }'
```

**Request Body:**
```json
{
  "decision": "approve|deny|modify",  // required
  "modified_command": "ls -la",       // optional, for "modify"
  "reason": "Security concern"        // optional
}
```

**Response:**
```json
{
  "success": true,
  "decision": "approve"
}
```

---

## Multi-Agent Endpoints

### List Agents

**GET `/agents`**

List all active agents.

```bash
curl http://localhost:3000/agents
```

**Response:**
```json
{
  "agents": [
    {
      "id": "agent-abc123",
      "name": "Code Reviewer",
      "profile": "review",
      "status": "idle",
      "task_count": 5
    },
    {
      "id": "agent-def456",
      "name": "Documentation Writer",
      "profile": "docs",
      "status": "working",
      "task_count": 2
    }
  ],
  "active_agent_id": "agent-abc123"
}
```

---

### Create Agent

**POST `/agents`**

Create a new agent.

```bash
curl -X POST http://localhost:3000/agents \
  -H "Content-Type: application/json" \
  -d '{
    "name": "My Custom Agent",
    "profile": "default"
  }'
```

**Request Body:**
```json
{
  "name": "string",           // optional
  "profile": "default"        // optional, default: "default"
                              // Options: "default", "debug", "docs", "review", "custom"
}
```

**Response:**
```json
{
  "agent": {
    "id": "agent-xyz789",
    "name": "My Custom Agent",
    "profile": "default",
    "status": "idle",
    "task_count": 0
  }
}
```

---

### Get Agent

**GET `/agents/:id`**

Get information about a specific agent.

Note: the current handler validates the id format but still returns placeholder agent details rather than a fully wired lookup.

```bash
curl http://localhost:3000/agents/agent-abc123
```

**Response:**
```json
{
  "id": "agent-abc123",
  "name": "Code Reviewer",
  "profile": "review",
  "status": "idle",
  "task_count": 5
}
```

---

### Switch Agent

**POST `/agents/:id/switch`**

Switch the active agent.

```bash
curl -X POST http://localhost:3000/agents/agent-def456/switch \
  -H "Content-Type: application/json" \
  -d '{
    "agent_id": "agent-def456"
  }'
```

**Request Body:**
```json
{
  "agent_id": "agent-def456"
}
```

**Response:**
```json
{
  "success": true,
  "active_agent_id": "agent-def456"
}
```

---

### Chat with Agent

**POST `/agents/:id/chat`**

Send a message to a specific agent.

Note: this route is currently a placeholder response and is not yet wired to the full agent runtime.

```bash
curl -X POST http://localhost:3000/agents/agent-abc123/chat \
  -H "Content-Type: application/json" \
  -d '{
    "message": "Review this code for security issues"
  }'
```

**Request Body:**
```json
{
  "message": "string (required)"
}
```

**Response:**
```json
{
  "content": "I found the following security issues...\n",
  "tool_calls": [],
  "stop_reason": "end_turn"
}
```

---

### Stream Agent Events

**GET `/agents/:id/stream`**

Stream events from a specific agent.

Note: this route exists in the router, but the handler is still provisional and should be treated as incomplete.

```bash
curl -N http://localhost:3000/agents/agent-abc123/stream \
  -H "Content-Type: application/json" \
  -d '{
    "message": "Analyze this file"
  }'
```

---

### Kill Agent

**DELETE `/agents/:id`**

Terminate an agent.

```bash
curl -X DELETE http://localhost:3000/agents/agent-abc123
```

**Response:**
```json
{
  "success": true
}
```

---

## Error Handling

All endpoints return appropriate HTTP status codes and error responses.

### Error Response Format

```json
{
  "error": "Timeout",
  "message": "Operation timed out after 30s",
  "tool": "bash",
  "retry_after": null
}
```

### Common Error Types

| Error | HTTP Status | Description |
|-------|-------------|-------------|
| `ToolInputError` | 400 | Invalid tool input |
| `Timeout` | 408 | Operation timed out |
| `CommandBlocked` | 403 | Command blocked by policy |
| `PathEscape` | 403 | Path traversal attempt |
| `TextNotFound` | 404 | Text not found in file |
| `ToolNotFound` | 404 | Tool not registered |
| `AgentError` | 500 | Generic agent error |

### HTTP Status Codes

| Code | Meaning |
|------|---------|
| 200 | Success |
| 400 | Bad Request |
| 403 | Forbidden |
| 404 | Not Found |
| 408 | Timeout |
| 500 | Internal Server Error |

---

## Client Examples

### Python Client

```python
import requests
import json

BASE_URL = "http://localhost:3000"

# Health check
response = requests.get(f"{BASE_URL}/health")
print(response.json())

# Chat
response = requests.post(
    f"{BASE_URL}/chat",
    json={"message": "List files in src/"}
)
print(response.json())

# Execute command
response = requests.post(
    f"{BASE_URL}/execute",
    json={"command": "ls -la"}
)
print(response.json())

# Stream events
response = requests.post(
    f"{BASE_URL}/stream",
    json={"message": "Create a hello world program"},
    stream=True
)

for line in response.iter_lines():
    if line:
        print(line.decode())
```

### JavaScript/TypeScript Client

```javascript
const BASE_URL = "http://localhost:3000";

// Health check
const health = await fetch(`${BASE_URL}/health`);
console.log(await health.json());

// Chat
const chat = await fetch(`${BASE_URL}/chat`, {
  method: "POST",
  headers: { "Content-Type": "application/json" },
  body: JSON.stringify({ message: "List files in src/" })
});
console.log(await chat.json());

// Stream events
const response = await fetch(`${BASE_URL}/stream`, {
  method: "POST",
  headers: { "Content-Type": "application/json" },
  body: JSON.stringify({ message: "Create hello world" })
});

const reader = response.body.getReader();
while (true) {
  const { done, value } = await reader.read();
  if (done) break;
  console.log(new TextDecoder().decode(value));
}
```

### cURL Examples

```bash
# Health check
curl http://localhost:3000/health

# Chat
curl -X POST http://localhost:3000/chat \
  -H "Content-Type: application/json" \
  -d '{"message": "Hello"}'

# Execute command
curl -X POST http://localhost:3000/execute \
  -H "Content-Type: application/json" \
  -d '{"command": "ls -la"}'

# Stream events
curl -N http://localhost:3000/stream \
  -H "Content-Type: application/json" \
  -d '{"message": "Write a Python script"}'

# List sessions
curl http://localhost:3000/sessions

# Get config
curl http://localhost:3000/config

# Update config
curl -X PATCH http://localhost:3000/config \
  -H "Content-Type: application/json" \
  -d '{"max_tokens": 8192}'
```

---

## Advanced Usage

### Batch Operations

```bash
# Chain multiple requests
curl http://localhost:3000/execute -d '{"command": "find . -name "*.rs""}' \
  && curl http://localhost:3000/chat -d '{"message": "Summarize these files"}'
```

### WebSocket Alternative

For bi-directional communication, use the streaming endpoint with client-side event handling:

```javascript
const eventSource = new EventSource(`${BASE_URL}/stream`);

eventSource.addEventListener('token_usage', (e) => {
  const data = JSON.parse(e.data);
  console.log('Tokens:', data.total_tokens);
});

eventSource.addEventListener('tool_progress', (e) => {
  const data = JSON.parse(e.data);
  console.log('Progress:', data.message);
});

eventSource.addEventListener('text_delta', (e) => {
  console.log('Text:', e.data);
});
```

### Integration with Frontend

```javascript
// React example
function ChatInterface() {
  const [messages, setMessages] = useState([]);
  const [streaming, setStreaming] = useState(false);

  const sendMessage = async (message) => {
    setStreaming(true);
    
    const response = await fetch(`${BASE_URL}/stream`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ message })
    });

    const reader = response.body.getReader();
    const decoder = new TextDecoder();

    while (true) {
      const { done, value } = await reader.read();
      if (done) break;

      const chunk = decoder.decode(value);
      // Parse SSE events and update UI
      // ...
    }

    setStreaming(false);
  };

  return (
    <div>
      {/* Chat UI */}
    </div>
  );
}
```

---

## Security Considerations

### CORS

The server enables CORS by default for all origins, methods, and headers. For production:

```rust
// Modify src/api/http.rs
CorsLayer::new()
    .allow_origin("https://your-domain.com".parse::<HeaderValue>().unwrap())
    .allow_methods([Method::GET, Method::POST])
    .allow_headers([HeaderName::CONTENT_TYPE]);
```

### Authentication

Add authentication middleware for production:

```rust
// Custom middleware for API key validation
async fn auth_middleware<B>(
    request: Request<B>,
    next: Next<B>,
) -> Response {
    let auth_header = request.headers().get(AUTHORIZATION);
    
    if auth_header != Some("Bearer your-api-key") {
        return Response::builder()
            .status(401)
            .body("Unauthorized".into())
            .unwrap();
    }

    next.run(request).await
}
```

### Rate Limiting

Implement rate limiting with `tower-governor`:

```toml
# Cargo.toml
tower-governor = "0.3"
```

```rust
use tower_governor::{Governor, GovernorConfigBuilder};

let governor_config = Arc::new(
    GovernorConfigBuilder::default()
        .per_second(10)
        .burst_size(20)
        .finish()
        .unwrap()
);

let app = app.layer(Governor::new(&governor_config));
```

---

## Troubleshooting

### Server Won't Start

**Issue:** Port already in use

```bash
# Check what's using port 3000
lsof -i :3000

# Kill the process or use a different port
cargo run --features full -- --server 8080
```

### CORS Errors

**Issue:** Browser blocking requests

Ensure CORS is enabled in the server configuration or add the appropriate headers to your requests.

### Timeout Errors

**Issue:** Operations timing out

Increase timeout in request:
```json
{
  "timeout_secs": 300
}
```

Or update global config:
```bash
curl -X PATCH http://localhost:3000/config \
  -d '{"tool_timeout_secs": 300}'
```

---

## See Also

- [TOOLS.md](TOOLS.md) - Tool system documentation
- [TUI_GUIDE.md](TUI_GUIDE.md) - Terminal UI guide
- [MULTI_AGENT.md](MULTI_AGENT.md) - Multi-agent coordination
- [ARCHITECTURE.md](ARCHITECTURE.md) - System architecture
