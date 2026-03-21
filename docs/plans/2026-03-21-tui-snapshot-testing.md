# TUI Snapshot Testing Plan

> **Status**: IMPLEMENTED (Phase 1 Complete)

## Goal
Capture and verify ALL content the user sees in the terminal, enabling complete visual regression testing with mocked backends.

## Implemented Components

```
tests/tui/
├── mod.rs                    # Module exports
├── capture.rs                # TuiCapture, TuiFrameSnapshot, CellSnapshot
├── comparison.rs             # Frame diff, SnapshotComparison
├── harness.rs                # TuiTestHarness, InputSequence, run_scenario
├── scenarios.rs              # Scenario builders (simple_text, streaming_text, etc.)
└── snapshots/                # Golden snapshots directory
```

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                        Test Harness                             │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────────┐ │
│  │ MockLLM     │  │ Input       │  │ Frame                   │ │
│  │ (scripted)  │──│ Sequence    │──│ Capturer               │ │
│  └─────────────┘  └─────────────┘  └──────────┬──────────────┘ │
│                                               │                 │
│                                               ▼                 │
│  ┌─────────────────────────────────────────────────────────────┐ │
│  │                    Snapshot Store                           │ │
│  │  snapshots/                                                 │ │
│  │  ├── session_start.jsonl     (cell-by-cell, with styles)  │ │
│  │  ├── user_input_typed.jsonl                               │ │
│  │  ├── streaming_response.jsonl  (frames at each token)      │ │
│  │  ├── tool_call_active.jsonl                                │ │
│  │  └── final_state.jsonl                                     │ │
│  └─────────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────┘
```

## Core Components

### 1. TuiCapture - Frame Capturer

Captures complete terminal state at any point:

```rust
pub struct TuiCapture {
    session_id: String,
    frame_counter: u64,
}

impl TuiCapture {
    /// Capture entire viewport as structured data
    pub fn capture_frame(&self, terminal: &Terminal) -> TuiFrameSnapshot {
        let size = terminal.size().unwrap();
        let buffer = terminal.current_buffer();

        TuiFrameSnapshot {
            session_id: self.session_id.clone(),
            frame_id: self.frame_counter,
            timestamp_ms: now(),
            width: size.width,
            height: size.height,
            cursor: self.extract_cursor(buffer),
            cells: self.extract_all_cells(buffer),
            footer: self.extract_footer_state(),
            header: self.extract_header_state(),
            messages: self.extract_messages(),
        }
    }
}
```

### 2. MockScenario - Deterministic Input

Scripted mock LLM for reproducible outputs:

```rust
pub struct MockScenario {
    name: String,
    events: Vec<StreamEvent>,
    delays_ms: Vec<u64>,  // Optional delays for timing tests
}

impl MockScenario {
    /// Create a scenario that simulates a complete user interaction
    pub fn simple_text_response(text: &str) -> Self { ... }

    pub fn with_tool_call(command: &str, output: &str) -> Self { ... }

    pub fn with_approval_request(command: &str) -> Self { ... }

    pub fn with_error_then_recovery() -> Self { ... }
}
```

### 3. InputSequence - Keyboard Events

Scripted user inputs:

```rust
pub struct InputSequence {
    events: Vec<InputEvent>,
}

impl InputSequence {
    pub fn type_text(text: &str) -> Self { ... }

    pub fn press(key: Key) -> Self { ... }

    pub fn ctrl(key: char) -> Self { ... }

    pub fn then(&self, other: InputSequence) -> Self { ... }
}
```

### 4. SnapshotComparison - Diff Engine

```rust
pub struct SnapshotComparison {
    expected: TuiFrameSnapshot,
    actual: TuiFrameSnapshot,
}

impl SnapshotComparison {
    /// Returns human-readable diff
    pub fn diff(&self) -> FrameDiff {
        FrameDiff {
            added_cells: self.find_added(),
            removed_cells: self.find_removed(),
            style_changes: self.find_style_changes(),
            cursor_changes: self.find_cursor_changes(),
        }
    }

    /// Assert with detailed failure message
    pub fn assert_match(&self) -> Result<(), FrameDiff> { ... }
}
```

## Test Scenarios

### Scenario 1: Initial State
```
Input: App starts with empty history
Expected: Clean slate, footer shows default values
```

### Scenario 2: User Types Input
```
Input: User types "Hello world" + Enter
Expected: Input field shows text, cursor at end
```

### Scenario 3: Streaming Response
```
Input: Mock sends "Hello" → "Hello world" token by token
Expected: Text appears progressively, streaming indicator
```

### Scenario 4: Tool Call Active
```
Input: Agent calls bash tool
Expected: Tool panel shows "Running...", command visible
```

### Scenario 5: Tool Success
```
Input: Tool completes
Expected: Checkmark, duration, output preview
```

### Scenario 6: Approval Modal
```
Input: Dangerous command requires approval
Expected: Modal overlay with approve/deny options
```

### Scenario 7: Error State
```
Input: Tool fails
Expected: Error message, red styling, stack trace
```

### Scenario 8: Context Compaction
```
Input: Context reaches 90%
Expected: Warning indicator, compaction triggered
```

### Scenario 9: Multi-Agent Indicator
```
Input: MESH mode active
Expected: "MESH" indicator in footer, agent name shown
```

### Scenario 10: Session Breadcrumb
```
Input: Sub-agent spawned
Expected: "root ▸ sub1 ▸ sub2" in footer
```

## Test Structure

```
tests/tui/
├── mod.rs
├── harness.rs          # Test harness setup
├── capture.rs         # Frame capture utilities
├── scenarios/
│   ├── mod.rs
│   ├── empty_session.rs
│   ├── simple_chat.rs
│   ├── tool_calls.rs
│   ├── approvals.rs
│   ├── errors.rs
│   └── multi_agent.rs
└── snapshots/
    ├── empty_session_initial.jsonl
    ├── simple_chat_user_input.jsonl
    ├── simple_chat_streaming_001.jsonl
    ├── simple_chat_streaming_002.jsonl
    └── ...
```

## Example Test

```rust
#[tokio::test]
async fn test_simple_chat_streaming() {
    // Arrange
    let harness = TuiTestHarness::new("simple_chat");
    let mock = MockScenario::streaming_text("Hello world");
    let input = InputSequence::type_text("Say hello");

    // Act
    harness.run(mock, input).await;

    // Assert - capture at each frame and compare
    let frames = harness.captured_frames();

    // Frame 0: Just user input
    assert_snapshot!(frames[0], "simple_chat_input");

    // Frame 1-12: Each token streamed
    for (i, frame) in frames[1..].iter().enumerate() {
        assert_snapshot!(frame, format!("simple_chat_token_{}", i));
    }

    // Final frame: Complete response
    assert_snapshot!(frames.last().unwrap(), "simple_chat_complete");
}
```

## Snapshot Format (JSONL)

```jsonl
{"version":"1.0","frame_id":0,"width":120,"height":40,"timestamp_ms":0,
 "cursor":{"x":11,"y":38,"visible":true},
 "footer":{"cwd":"/project","git_branch":"main","model":"claude-3-sonnet","context_pct":0,"agent_name":null,"is_mesh":false},
 "header":{"session_label":"session0","streaming":false},
 "cells":[{"x":0,"y":0,"c":" ","fg":"default","bg":"default","bold":false}],
 "regions":{
   "input":{"y":38,"text":"Hello world","cursor_x":11},
   "messages":[{"role":"user","content":"Hello world","y_start":0}],
   "tool_panel":{"active":false}
 }}
```

## Update Workflow

```bash
# Update snapshots when UI intentionally changes
cargo test tui -- --update-snapshots

# This regenerates all .snap files
# Review git diff, commit intentional changes
```

## CI Integration

```yaml
# .github/workflows/test.yml
- name: TUI Snapshot Tests
  run: |
    cargo test tui --features full
  env:
    UPDATE_SNAPSHOTS: ${{ github.event_name == 'workflow_dispatch' }}
```

## Key Implementation Tasks

1. [ ] Create `tests/tui/mod.rs` with test harness
2. [ ] Implement `TuiCapture::capture_frame()` using ratatui's backend
3. [ ] Create `MockScenario` builder for scripted LLM responses
4. [ ] Implement `InputSequence` for scripted keyboard events
5. [ ] Build `SnapshotComparison` with diff generation
6. [ ] Write 10+ scenario tests covering all UI states
7. [ ] Create snapshot storage and update mechanism
8. [ ] Add `#[tokio::test]` variants for async scenarios

## Benefits

1. **Zero API costs** - All mocks, no real LLM calls
2. **Deterministic** - Same input → same output every time
3. **Complete coverage** - Every pixel the user sees is tested
4. **Regression prevention** - Changes break tests, not users
5. **Documentation** - Snapshots serve as living docs of UI behavior
