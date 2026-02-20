# AGENTS.md - AI Agent Development Guide

## Project Overview

Amadeus is an **Agent SDK** - a Rust library providing core building blocks for building AI agents. It is NOT a platform or application.

## Core Principles

1. **SDK, not Platform** - We provide agent capabilities, not session/memory/platform features
2. **Minimal dependencies** - Only what's needed for LLM interaction and tool execution
3. **Type-safe** - Strong typing with Result-based error handling
4. **Streaming-first** - Real-time response streaming as a first-class feature

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                     Amadeus SDK                              │
│                                                             │
│  Agent Loop │ Tool System │ LLM Clients │ Streaming         │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

**See also:**
- [SDK_SCOPE.md](docs/SDK_SCOPE.md) - Detailed scope definition
- [INTEGRATION_GUIDE.md](docs/INTEGRATION_GUIDE.md) - Integration with NeuroCore

## Development Workflow

### Building

```bash
cargo build
cargo test
cargo clippy
```

### Running TUI Test Harness

```bash
# The TUI is for testing SDK performance
cargo run --example tui

# Or with custom config
ANTHROPIC_API_KEY=xxx cargo run --example tui
```

### Building Python Bindings

```bash
cd bindings/python
maturin develop --release
```

## Code Organization

### Keep (SDK Core)

```
src/
├── lib.rs              # Public SDK exports
├── error.rs            # Error types
├── agent/
│   ├── agent.rs        # Agent<C> struct
│   ├── config.rs       # AgentConfig
│   ├── messages.rs     # Message types
│   └── events.rs       # AgentEvent
├── client/
│   ├── mod.rs          # LLMClient trait
│   ├── anthropic.rs    # Anthropic implementation
│   └── openai.rs       # OpenAI implementation
└── tools/
    ├── mod.rs          # Tool trait
    ├── bash.rs         # BashTool
    └── file.rs         # FileTools
```

### Remove/Move (Platform Layer)

```
src/
├── api/                # Move to examples/ - for testing only
├── core/workspace.rs   # Remove - Platform concern
└── concurrency/        # Remove - Platform concern
```

## When Adding New Features

### ✅ Belongs in SDK

- New LLM client (e.g., Gemini, Claude direct)
- New tool type (e.g., web search, database)
- Agent loop improvements
- Streaming optimizations
- Error handling improvements

### ❌ Does NOT Belong in SDK

- Session persistence
- Memory management
- HTTP server (except for testing)
- Platform adapters (Discord, Slack, etc.)
- User authentication
- Database connections

## Testing

### Unit Tests

```bash
cargo test
cargo test --test bash_test
```

### Integration Tests

Mock LLM responses:

```rust
use wiremock::{MockServer, Mock, ResponseTemplate};

#[tokio::test]
async fn test_agent_run() {
    let mock_server = MockServer::start();
    // Setup mock response
    // Test agent
}
```

### TUI Testing

Use the TUI to manually test:
- Streaming performance
- Tool execution
- Error handling
- Long-running conversations

## Commit Guidelines

```
feat(agent): add support for tool result streaming
fix(client): handle timeout errors correctly
docs(readme): update installation instructions
refactor(tools): simplify bash tool implementation
test(agent): add tests for multi-turn conversations
```

## Key Files to Understand

| File | Purpose |
|------|---------|
| `src/lib.rs` | SDK entry point, re-exports |
| `src/agent/agent.rs` | Core Agent struct |
| `src/client/mod.rs` | LLMClient trait definition |
| `src/tools/mod.rs` | Tool trait definition |
| `src/error.rs` | All error types |

## Platform Integration

Amadeus SDK is used by NeuroCore Platform. When making changes:

1. Check if it breaks the Python bindings
2. Update `INTEGRATION_GUIDE.md` if API changes
3. Test with NeuroCore integration tests
4. Version bump appropriately

---

*Last updated: 2026-02-20*
