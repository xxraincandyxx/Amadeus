# Claude Agent (Rust v0)

Bash is all you need. A minimal AI coding agent implementation in Rust.

## Philosophy

Like the Python reference implementation, this Rust version demonstrates that **one tool is enough**. Bash provides:
- File reading: `cat`, `head`, `grep`
- File writing: `echo '...' > file`, `sed`
- Execution: `python`, `npm`, `make`, any command
- **Subagents**: `cargo run -- "task description"`

## Features

- **Multi-provider support** - Anthropic and OpenAI compatible
- **Single bash tool** - All file operations and command execution
- **Recursive subagents** - Spawn isolated agents for complex tasks
- **Process isolation** - Each subagent has fresh context
- **Tokio async** - Efficient concurrent operations
- **Streaming responses** - Optional streaming for faster output
- **Dracula-themed UI** - Purple/pink color scheme
- **Strongly-typed** - Full error handling with `Result<T>`

## Quick Start

```bash
# Install dependencies
cargo install --locked

# Configure API key
cp .env.example .env
# Edit .env with your API key (ANTHROPIC_API_KEY or OPENAI_API_KEY)

# Interactive mode (default: Anthropic)
cargo run

# Use OpenAI instead
PROVIDER=openai cargo run

# Enable streaming
USE_STREAMING=true cargo run

# Subagent mode (for testing)
cargo run -- "echo hello world and tell me the output"
```

## Configuration

Environment variables (`.env` file):

| Variable | Required | Default | Description |
|----------|-----------|---------|-------------|
| `PROVIDER` | No | anthropic | AI provider: `anthropic` or `openai` |
| `ANTHROPIC_API_KEY` | Yes* | - | Your Anthropic API key (required for Anthropic) |
| `ANTHROPIC_BASE_URL` | No | https://api.anthropic.com | Anthropic API endpoint |
| `OPENAI_API_KEY` | Yes* | - | Your OpenAI API key (required for OpenAI) |
| `OPENAI_BASE_URL` | No | https://api.openai.com | OpenAI API endpoint |
| `MODEL_ID` | No | claude-sonnet-4-5-20250929 (Anthropic) / gpt-4 (OpenAI) | Model to use |
| `USE_STREAMING` | No | false | Enable streaming responses |
| `TIMEOUT_SECONDS` | No | 30 | Command timeout in seconds |

*At least one provider API key is required based on the selected provider.

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
│   ├── mod.rs           # LLMClient trait
│   ├── anthropic.rs     # Anthropic API client
│   └── openai.rs        # OpenAI API client
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
├── bash_test.rs         # Bash tool unit tests
└── openai_test.rs       # OpenAI client unit tests
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
