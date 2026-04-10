# Context Compaction

Automatic conversation history summarization to manage context window limits.

## Overview

When conversations grow long, they can exceed the model's context window. The compaction system:

1. Monitors token usage in conversation history
2. Triggers summarization when approaching limits
3. Preserves recent messages and important context
4. Uses LLM to generate meaningful summaries

## Configuration

### Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `AUTO_COMPACT` | `true` | Enable automatic compaction |
| `COMPACT_THRESHOLD_PERCENT` | `75` | Trigger at this % of context window |
| `COMPACT_PRESERVE_RECENT` | `6` | Number of recent messages to keep |
| `CONTEXT_WINDOW_SIZE` | `200000` | Model's context window size (tokens) |

### CompactionConfig

```rust
pub struct CompactionConfig {
    /// Threshold % to trigger compaction (default: 75)
    pub threshold_percent: u8,
    
    /// Target % after compaction (default: 30)
    pub target_percent: u8,
    
    /// Recent messages to preserve (default: 6)
    pub preserve_recent: usize,
    
    /// Use LLM for summarization (default: true)
    pub use_llm_summary: bool,
    
    /// Max summary characters (default: 2000)
    pub max_summary_chars: usize,
    
    /// Minimum messages before compaction (default: 10)
    pub min_messages: usize,
    
    /// Max tool result chars before truncation (default: 5000)
    pub max_tool_result_chars: usize,
}
```

## Code Flow

### 1. Agent Loop Integration

Location: `src/agent/loop_agent.rs`

```
┌─────────────────────────────────────────────────────────────┐
│                    Agent Loop (run_stream)                  │
│                                                             │
│  ┌──────────────────────────────────────────────────────┐   │
│  │  Initialize Compactor                                │   │
│  │  if config.auto_compact {                            │   │
│  │      compactor = ContextCompactor::new(...)          │   │
│  │  }                                                   │   │
│  └──────────────────────────────────────────────────────┘   │
│                           │                                 │
│                           ▼                                 │
│  ┌──────────────────────────────────────────────────────┐   │
│  │  Turn Loop                                           │   │
│  │  while should_continue {                             │   │
│  │      ┌────────────────────────────────────────────┐  │   │
│  │      │ Check if compaction needed                 │  │   │
│  │      │ compactor.needs_compaction(&history, ...)  │  │   │
│  │      └────────────────────────────────────────────┘  │   │
│  │                      │                               │   │
│  │           ┌──────────┴──────────┐                    │   │
│  │           ▼                     ▼                    │   │
│  │      [Needed]              [Not Needed]              │   │
│  │           │                     │                    │   │
│  │           ▼                     │                    │   │
│  │      compact()                  │                    │   │
│  │           │                     │                    │   │
│  │           └──────────┬──────────┘                    │   │
│  │                      ▼                               │   │
│  │      Execute LLM turn                                │   │
│  │      Process tool calls                              │   │
│  │  }                                                   │   │
│  └──────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
```

### 2. Compaction Check (needs_compaction)

Location: `src/agent/compaction.rs:144`

```
needs_compaction(history, context_window_size)
│
├── Check: history.len() < min_messages?
│   └── YES → return false (too few messages)
│
├── Estimate tokens in history
│   └── estimate_tokens(history)
│       └── Sum char counts / 4 (rough approximation)
│
├── Calculate threshold
│   └── threshold = context_window_size * threshold_percent / 100
│
└── Return: estimated_tokens > threshold
```

### 3. Compaction Execution (compact)

Location: `src/agent/compaction.rs:319`

```
compact(history, client, context_window_size)
│
├── 1. Calculate split point
│   └── split_point = history.len() - preserve_recent
│
├── 2. Split history
│   ├── to_summarize = history.drain(0..split_point)
│   └── remaining = last N messages (preserved)
│
├── 3. Generate summary
│   ├── if use_llm_summary:
│   │   ├── summarize_messages() via LLM
│   │   └── is_valid_summary() check
│   │       └── Fallback: extract_key_points()
│   └── else:
│       └── extract_key_points()
│
├── 4. Insert summary message
│   └── history.insert(0, summary_message)
│
├── 5. Truncate large tool results
│   └── truncate_tool_results(history)
│
└── 6. Return CompactionResult
    ├── original_count
    ├── compacted_count
    ├── tokens_saved
    └── summary
```

### 4. LLM Summarization

Location: `src/agent/compaction.rs:418`

```
summarize_messages(messages, client)
│
├── Format messages for summary
│   └── format_messages_for_summary()
│       ├── Truncate individual texts (>500 chars)
│       ├── Truncate tool results (>300 chars)
│       └── Max 10000 chars total
│
├── Build prompt
│   └── "Summarize conversation focusing on:
│        - Key tasks and objectives
│        - Important decisions
│        - Files modified/created
│        - Errors and resolutions
│        - Current state"
│
├── Call LLM
│   └── client.create_message_stream(prompt)
│
└── Process stream
    └── Collect TextDelta events into summary
```

### 5. Summary Validation

Location: `src/agent/compaction.rs:199`

```
is_valid_summary(summary, original_messages)
│
├── Check minimum length (>= 50 chars)
│
├── Extract technical terms from original
│   └── extract_technical_terms()
│       ├── File extensions (.rs, .ts, .py, etc.)
│       ├── File paths (contains /)
│       └── Tool names
│
├── Check summary contains >= 1 technical term
│
└── Check for hallucination indicators
    └── Reject if contains: "once upon a time", 
        "chapter 1", "the end", etc.
```

### 6. Fallback: Extract Key Points

Location: `src/agent/compaction.rs:487`

Used when LLM summarization fails or produces invalid output.

```
extract_key_points(messages)
│
├── Scan messages for:
│   ├── File paths (contains / or .rs/.ts/.py/etc)
│   ├── Tool names (from ToolUse blocks)
│   └── Errors (from ToolResult containing "error")
│
└── Build summary string:
    ├── "Tools used: <tools>"
    ├── "Files involved: <files>"
    └── "Key events: <errors>"
```

## Result Structure

```rust
pub struct CompactionResult {
    /// Messages before compaction
    pub original_count: usize,
    
    /// Messages after compaction
    pub compacted_count: usize,
    
    /// Tokens before compaction
    pub original_tokens: usize,
    
    /// Tokens after compaction
    pub new_tokens: usize,
    
    /// Tokens saved
    pub tokens_saved: usize,
    
    /// Generated summary (if LLM used)
    pub summary: Option<String>,
    
    /// Number of messages summarized
    pub messages_summarized: usize,
}
```

## Events

The agent emits `AgentEvent::Compaction` after successful compaction:

```rust
AgentEvent::Compaction {
    original_count: usize,
    compacted_count: usize,
    tokens_saved: usize,
    messages_summarized: usize,
}
```

## TUI Integration

Location: `src/ui/app.rs`

The TUI supports manual compaction via `Ctrl+O`:

```
┌─────────────────────────────────────────────────────────┐
│                    TUI Compaction                       │
│                                                         │
│  User presses Ctrl+O                                    │
│         │                                               │
│         ▼                                               │
│  start_compaction()                                     │
│         │                                               │
│         ├── Clone history (Arc<RwLock>)                 │
│         ├── Spawn background tokio task                 │
│         │   └── compactor.compact()                     │
│         │                                               │
│         ▼                                               │
│  UI shows animation (CompactionAnimator)                │
│         │                                               │
│         ▼                                               │
│  poll_compaction_result()                               │
│         │                                               │
│         ├── Result received → Update history            │
│         │   └── Show "X tokens saved" message           │
│         │                                               │
│         └── Error → Show error message                  │
└─────────────────────────────────────────────────────────┘
```

## Example Usage

```rust
use amadeus::agent::compaction::{ContextCompactor, CompactionConfig};

let compactor = ContextCompactor::new(CompactionConfig {
    threshold_percent: 75,
    target_percent: 40,
    preserve_recent: 6,
    ..Default::default()
});

// Check if needed
if compactor.needs_compaction(&history, 200_000) {
    let result = compactor.compact(&mut history, &client, 200_000).await?;
    println!("Saved {} tokens", result.tokens_saved);
}
```

## Best Practices

1. **Preserve sufficient context**: Keep `preserve_recent >= 6` to maintain conversation flow
2. **Set appropriate threshold**: 75% gives buffer before hitting actual limits
3. **Enable LLM summaries**: Higher quality than extract-based fallback
4. **Monitor compaction events**: Track tokens saved to tune settings

---
*Last updated: 2026-03-07*
