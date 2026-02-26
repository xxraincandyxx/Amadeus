# Amadeus + NeuroCore Integration Guide

## Overview

This document defines the integration between Amadeus SDK and NeuroCore Platform.

## Relationship

```
┌─────────────────────────────────────────────────────────────────┐
│                        NeuroCore                                 │
│                     (Agent Platform)                             │
│                                                                 │
│  Sessions │ Memory │ Adapters │ HTTP API │ Plugins │ UI        │
│                                                                 │
│  "When to run, what context, where to send results"            │
│                                                                 │
└───────────────────────────┬─────────────────────────────────────┘
                            │
                            │ uses
                            │
┌───────────────────────────▼─────────────────────────────────────┐
│                        Amadeus                                   │
│                      (Agent SDK)                                 │
│                                                                 │
│  Agent Loop │ Tools │ LLM Clients │ Streaming                   │
│                                                                 │
│  "How to run, how to call LLM, how to execute tools"           │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

## Responsibility Matrix

| Concern | Amadeus SDK | NeuroCore Platform |
|---------|-------------|-------------------|
| LLM API calls | ✅ | ❌ |
| Tool execution | ✅ | ❌ |
| Agent loop | ✅ | ❌ |
| Streaming | ✅ | ❌ |
| Session storage | ❌ | ✅ |
| Memory management | ❌ | ✅ |
| Platform adapters | ❌ | ✅ |
| HTTP API | ❌ | ✅ |
| Plugin system | ❌ | ✅ |
| UI | ❌ | ✅ |
| Auth | ❌ | ✅ |

## Integration Points

### 1. Python Bindings (PyO3)

Amadeus exposes Python bindings via PyO3:

```rust
// amadeus/bindings/python/src/lib.rs

use pyo3::prelude::*;

#[pymodule]
fn amadeus(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_class::<PyAgent>()?;
    m.add_class::<PyToolRegistry>()?;
    m.add_class::<PyBashTool>()?;
    m.add_class::<PyReadFileTool>()?;
    m.add_class::<PyWriteFileTool>()?;
    m.add_class::<PyEditFileTool>()?;
    Ok(())
}
```

### 2. Data Types

Shared types between SDK and Platform:

```python
# Message format (both sides)

class Message:
    role: Literal["user", "assistant", "system"]
    content: list[ContentBlock]

class ContentBlock:
    type: Literal["text", "tool_use", "tool_result"]
    # text: str
    # tool_use_id: str
    # name: str
    # input: dict
    # content: str

class RunResult:
    text: str
    tool_calls: list[ToolCall]
    usage: Usage
```

### 3. Streaming Protocol

SSE format for streaming:

```
event: text_delta
data: {"text": "Hello"}

event: tool_call_start
data: {"id": "call_123", "name": "bash"}

event: tool_call_delta
data: {"arguments": "{\"command\": \"ls\""}

event: tool_call_done
data: {"id": "call_123", "output": "file1.txt\nfile2.txt"}

event: done
data: {}
```

## Development Workflow

### Amadeus Development

```bash
cd ~/projects/amadeus

# Run tests
cargo test

# Build SDK
cargo build --release

# Build Python bindings
cd bindings/python
maturin develop --release

# Run TUI test harness
cargo run --example tui
```

### NeuroCore Development

```bash
cd ~/NeuraBot

# Install Amadeus SDK
pip install ../amadeus/bindings/python

# Run backend
cd backend
uvicorn main:app --reload

# Run frontend
cd frontend
npm run dev
```

## File Structure After Refactoring

### Amadeus

```
amadeus/
├── Cargo.toml
├── src/
│   ├── lib.rs              # SDK exports
│   ├── error.rs            # Error types
│   ├── agent/
│   │   ├── agent.rs        # Agent struct
│   │   ├── config.rs       # Config
│   │   ├── messages.rs     # Message types
│   │   └── events.rs       # Event types
│   ├── client/
│   │   ├── mod.rs          # LLMClient trait
│   │   ├── anthropic.rs    # Anthropic client
│   │   └── openai.rs       # OpenAI client
│   └── tools/
│       ├── mod.rs          # Tool trait
│       ├── bash.rs         # Bash tool
│       └── file.rs         # File tools
├── bindings/
│   └── python/             # PyO3 bindings
└── examples/
    └── tui/                # TUI test harness
```

### NeuroCore

```
NeuraBot/
├── backend/
│   ├── main.py
│   ├── core/
│   │   ├── session.py      # Session manager
│   │   ├── memory.py       # Memory engine
│   │   └── amadeus_client.py  # SDK wrapper
│   ├── adapters/
│   │   ├── discord.py
│   │   ├── telegram.py
│   │   └── qq.py
│   ├── api/
│   │   └── routes.py
│   └── plugins/
├── frontend/               # React UI
└── docs/
    └── PLATFORM_ARCHITECTURE.md
```

## Migration Checklist

### Amadeus Refactoring

- [ ] Create `refactor/sdk-scope` branch
- [ ] Remove `src/api/http.rs` (move to examples)
- [ ] Remove `src/core/workspace.rs`
- [ ] Remove `src/concurrency/`
- [ ] Refactor TUI as test harness
- [ ] Add Python bindings (PyO3)
- [ ] Update documentation
- [ ] Update tests

### NeuroCore Refactoring

- [ ] Create `refactor/use-amadeus-sdk` branch
- [ ] Remove `backend/agent/` (use Amadeus)
- [ ] Remove `backend/tools/` (use Amadeus)
- [ ] Add `amadeus` dependency
- [ ] Implement `AmadeusClient` wrapper
- [ ] Update adapters to use SDK
- [ ] Update API endpoints
- [ ] Update tests

## Testing Strategy

### Unit Tests

- **Amadeus**: Test SDK core (agent loop, tools, clients)
- **NeuroCore**: Test platform services (session, memory, adapters)

### Integration Tests

```python
# NeuroCore integration test

import pytest
from amadeus import Agent
from core.session import SessionManager
from core.memory import MemoryEngine

@pytest.mark.asyncio
async def test_session_with_amadeus():
    # Create session
    sessions = SessionManager(":memory:")
    session = await sessions.create("user_123", "test")
    
    # Create agent
    agent = Agent(provider="openai", model="gpt-4")
    
    # Run
    result = await agent.run("Hello", session.history)
    
    # Save
    await sessions.add_message(session.id, {
        "role": "assistant",
        "content": result.text,
    })
    
    # Verify
    loaded = await sessions.get(session.id)
    assert len(loaded.history) == 2
```

## Release Process

1. **Amadeus Release**
   - Update version in `Cargo.toml`
   - Build Python bindings
   - Publish to crates.io
   - Publish to PyPI

2. **NeuroCore Release**
   - Update Amadeus dependency
   - Run integration tests
   - Build Docker image
   - Deploy

---

*Document created: 2026-02-20*
*Last updated: 2026-02-20*
