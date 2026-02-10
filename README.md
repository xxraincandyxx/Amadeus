# Amadeus - AI Coding Agent

**Bash is all you need.** A minimal AI coding agent implementation in Rust.

## Philosophy

This project demonstrates that **one tool is sufficient** for a fully functional AI coding agent. With bash, the model can:
- Read files: `cat`, `grep`, `head`, `tail`, `rg`, `ls`
- Write files: `echo '...' > file`, `sed`, `cat << 'EOF' > file`
- Execute any command: `python`, `npm`, `make`, `cargo`, etc.
- Spawn subagents: `cargo run -- "task description"` for isolated context

## Features

- **Multi-provider support** - Anthropic and OpenAI APIs with unified interface
- **Single bash tool** - Covers all file operations and command execution
- **Recursive subagents** - Spawn isolated agents for complex tasks
- **Async architecture** - Based on Tokio for non-blocking I/O
- **Streaming responses** - Optional real-time streaming for better UX
- **Type-safe** - Strong error handling with `Result<T>` and `thiserror`
- **Concurrent execution** - Parallel tool execution for independent tasks
- **Dracula-themed UI** - Purple/pink terminal colors

## Quick Start

```bash
# 1. Install dependencies
cargo install --locked

# 2. Configure API key
cp .env.example .env
# Edit .env with your ANTHROPIC_API_KEY or OPENAI_API_KEY

# 3. Interactive mode
cargo run

# 4. Use OpenAI instead
PROVIDER=openai cargo run

# 5. Enable streaming
USE_STREAMING=true cargo run

# 6. Subagent mode (single task)
cargo run -- "echo hello world and tell me the output"
```

## Configuration

Environment variables in `.env` file:

| Variable | Required | Default | Description |
|----------|-----------|---------|-------------|
| `PROVIDER` | No | `anthropic` | AI provider: `anthropic` or `openai` |
| `ANTHROPIC_API_KEY` | Yes* | - | Anthropic API key |
| `ANTHROPIC_BASE_URL` | No | https://api.anthropic.com | Anthropic API endpoint |
| `OPENAI_API_KEY` | Yes* | - | OpenAI API key |
| `OPENAI_BASE_URL` | No | https://api.openai.com | OpenAI API endpoint |
| `MODEL_ID` | No | Provider default | Model to use |
| `USE_STREAMING` | No | false | Enable streaming responses |
| `TIMEOUT_SECONDS` | No | 300 | Command timeout in seconds |

*At least one provider API key required.

## Testing

```bash
# Run all tests
cargo test

# Run specific test file
cargo test --test bash_test

# Run with output
cargo test -- --nocapture

# Lint check
cargo check
```

## Documentation

- **[ARCHITECTURE.md](ARCHITECTURE.md)** - Detailed technical documentation, design patterns
- **[DEVELOPMENT.md](DEVELOPMENT.md)** - Development guide, working theory
- **[AGENTS.md](AGENTS.md)** - Guide for AI agents working in this codebase

## Project Structure

```
src/
├── main.rs              # CLI entry point
├── lib.rs               # Library exports
├── error.rs             # Custom error types (thiserror)
├── agent/
│   ├── mod.rs
│   ├── config.rs        # Environment-based configuration
│   ├── messages.rs      # Message types with serde
│   └── loop_agent.rs   # Core agent loop (streaming + non-streaming)
├── client/
│   ├── mod.rs           # LLMClient trait (generic provider abstraction)
│   ├── anthropic.rs     # Anthropic API implementation
│   └── openai.rs        # OpenAI API implementation
├── tools/
│   ├── mod.rs
│   ├── bash.rs          # Async bash executor with timeout
│   └── schema.rs        # Tool schemas (JSON)
└── ui/
    ├── colors.rs        # Dracula theme palette
    └── repl.rs          # Interactive REPL

tests/                    # Integration tests
├── bash_test.rs
├── agent_test.rs
├── config_test.rs
└── messages_test.rs
```

## Comparison with Python Reference

| Feature | Python v0 | Rust v0 |
|---------|-----------|----------|
| Lines of code | ~50 | ~200 |
| Async | No | Yes (Tokio) |
| Type safety | Dynamic | Strong |
| Error handling | String returns | `Result<T>` |
| Concurrency | No | Yes |
| Tool count | 1 (bash) | 1 (bash) |

## License

MIT
