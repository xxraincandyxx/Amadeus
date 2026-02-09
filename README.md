# Claude Agent (Rust v0)

Bash is all you need. A minimal AI coding agent implementation in Rust.

## Philosophy

Like the Python reference implementation, this Rust version demonstrates that **one tool is enough**. Bash provides:
- File reading: `cat`, `head`, `grep`
- File writing: `echo '...' > file`, `sed`
- Execution: `python`, `npm`, `make`, any command
- **Subagents**: `cargo run -- "task description"`

## Features

- **Single bash tool** - All file operations and command execution
- **Recursive subagents** - Spawn isolated agents for complex tasks
- **Process isolation** - Each subagent has fresh context
- **Tokio async** - Efficient concurrent operations
- **Dracula-themed UI** - Purple/pink color scheme
- **Strongly-typed** - Full error handling with `Result<T>`

## Quick Start

```bash
# Install dependencies
cargo install --locked

# Configure API key
cp .env.example .env
# Edit .env with your ANTHROPIC_API_KEY

# Interactive mode
cargo run

# Subagent mode (for testing)
cargo run -- "echo hello world and tell me the output"
```

## Configuration

Environment variables (`.env` file):

| Variable | Required | Default | Description |
|----------|-----------|---------|-------------|
| `ANTHROPIC_API_KEY` | Yes | - | Your Anthropic API key |
| `ANTHROPIC_BASE_URL` | No | https://api.anthropic.com | API endpoint (for proxies) |
| `MODEL_ID` | No | claude-sonnet-4-5-20250929 | Model to use |

## Running Tests

```bash
# Run all unit tests
cargo test

# Run bash tool tests specifically
cargo test --test bash_test

# Run with output
cargo test -- --nocapture
```

## Project Structure

```
src/
├── main.rs              # CLI entry point
├── lib.rs               # Library exports
├── error.rs             # Custom error types
├── agent/
│   ├── mod.rs
│   ├── config.rs        # Configuration loading
│   ├── messages.rs      # Message types
│   └── loop_agent.rs   # Core agent loop
├── client/
│   ├── mod.rs
│   └── anthropic.rs     # API client
├── tools/
│   ├── mod.rs
│   ├── bash.rs          # Bash executor
│   └── schema.rs        # Tool schemas
└── ui/
    ├── mod.rs
    ├── colors.rs        # Dracula theme
    └── repl.rs          # Interactive REPL

tests/
├── mod.rs
└── bash_test.rs         # Unit tests
```

## Comparison with Python Reference

| Feature | Python v0 | Rust v0 |
|---------|------------|----------|
| Lines of code | ~50 | ~200 |
| Async | No | Yes (Tokio) |
| Type safety | Dynamic | Strong |
| Error handling | String returns | Result<T> |
| Colors | ANSI | colored crate |
| Concurrent | No | Yes |
| Tool count | 1 (bash) | 1 (bash) |

## License

MIT
