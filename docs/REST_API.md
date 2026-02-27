# Amadeus REST API Guide

This document provides a comprehensive reference for the Amadeus SDK REST APIs. The server is built on Axum and leverages the multi-agent Supervisor for task orchestration.

## Overview

The Amadeus server exposes a high-performance interface for interacting with agents and tools. It supports stateless chat, direct command execution, real-time event streaming (SSE), and complex multi-agent task dispatching.

### Base URL
By default, the server runs on: `http://localhost:3000`

---

## 1. System Endpoints

### GET `/health`
Check the health and version of the server.

**Response:**
```json
{
  "status": "ok",
  "version": "0.1.0"
}
```

---

## 2. Agent Endpoints

### POST `/chat`
Send a stateless message to the agent. This is a convenience wrapper that dispatches a generic task to the Supervisor.

**Request Body:**
| Field | Type | Description |
|-------|------|-------------|
| `message` | String | The user's prompt |
| `timeout_secs` | Integer | (Optional) Tool execution timeout |

**Example Request:**
```bash
curl -X POST http://localhost:3000/chat 
  -H "Content-Type: application/json" 
  -d '{"message": "Summarize the project structure"}'
```

**Response:**
```json
{
  "content": "The project structure is...",
  "tool_calls": [
    {
      "name": "bash",
      "input": {"command": "ls -R"},
      "output": "..."
    }
  ],
  "stop_reason": "end_turn"
}
```

### GET `/stream`
Stream agent execution events in real-time using Server-Sent Events (SSE).

**Query Parameters:**
- `message`: The user's prompt (URL encoded)

**Example Request:**
```bash
curl -N "http://localhost:3000/stream?message=Explain+the+SDK"
```

**Events:**
- `text`: Partial text response deltas.
- `tool_start`: Triggered when a tool begins execution.
- `tool_done`: Triggered when a tool finishes execution.
- `done`: Final event containing the stop reason.
- `error`: Sent if an error occurs.

---

## 3. Tool Endpoints

### POST `/execute`
Execute a bash command directly via the SDK's high-performance Bash tool. Security rules and timeouts are enforced.

**Request Body:**
| Field | Type | Description |
|-------|------|-------------|
| `command` | String | The shell command to run |
| `timeout_secs` | Integer | (Optional) Timeout in seconds |

**Example Request:**
```bash
curl -X POST http://localhost:3000/execute 
  -H "Content-Type: application/json" 
  -d '{"command": "ls -la src/"}'
```

---

## 4. Orchestration Endpoints

### POST `/tasks`
Dispatch a complex task to the multi-agent Supervisor. This supports capability matching and backpressure.

**Request Body:**
| Field | Type | Description |
|-------|------|-------------|
| `id` | String | Unique identifier for the task |
| `prompt` | String | Detailed instructions for the agent |
| `capabilities` | Array<String> | (Optional) List of required worker capabilities |

**Example Request:**
```bash
curl -X POST http://localhost:3000/tasks 
  -H "Content-Type: application/json" 
  -d '{
    "id": "refactor-1",
    "prompt": "Refactor the error handling in lib.rs",
    "capabilities": ["rust", "bash"]
  }'
```

**Response:**
```json
{
  "task_id": "refactor-1",
  "worker_id": "worker-0",
  "success": true,
  "output": "Refactoring complete...",
  "error": null,
  "duration_ms": 12450
}
```

---

## Error Handling

The API uses standard HTTP status codes and returns a structured JSON error response for 4xx and 5xx errors.

**Error Response Body:**
```json
{
  "error": "Timeout",
  "message": "Operation timed out after 30s",
  "tool": "bash"
}
```

---
*Last updated: 2026-02-27*
