# Human-Agent Interaction Test Flow (HAI-TestFlow)

> Design document for simulating, recording, and debugging human-agent interactions with full JSON logging.

## Overview

This document describes a comprehensive test flow system for capturing, replaying, and debugging human-agent interactions. The system records all inputs, outputs, GUI events, and rendered TUI frames into structured JSON logs for bug analysis and regression testing.

---

## Goals

1. **Complete Session Recording**: Capture every aspect of an agent session
2. **Replayability**: Ability to replay recorded sessions deterministically
3. **GUI Event Tracking**: Monitor TUI state changes, key presses, and renders
4. **Bug Reproduction**: Convert bug reports into reproducible test scenarios
5. **Regression Testing**: Use recorded sessions as integration tests

---

## Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         HAI-TestFlow System                                 │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  ┌──────────────┐     ┌──────────────┐     ┌──────────────┐                 │
│  │   Recording  │────▶│   Session    │────▶│   Playback   │                 │
│  │    Layer     │     │     Log      │     │    Engine    │                 │
│  └──────────────┘     │   (JSON)     │     └──────────────┘                 │
│         │             └──────────────┘              │                       │
│         │                    │                      │                       │
│         ▼                    ▼                      ▼                       │
│  ┌──────────────┐     ┌──────────────┐     ┌──────────────┐                 │
│  │   Event      │     │   Analysis   │     │   Assertion  │                 │
│  │   Capture    │     │    Tools     │     │   Engine     │                 │
│  └──────────────┘     └──────────────┘     └──────────────┘                 │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Components

| Component | Purpose |
|-----------|---------|
| **Recording Layer** | Captures events during live sessions |
| **Session Log** | Structured JSON storage of all events |
| **Playback Engine** | Replays recorded sessions deterministically |
| **Event Capture** | Hooks into agent, tools, and TUI for event collection |
| **Analysis Tools** | Query and analyze recorded sessions |
| **Assertion Engine** | Verify expected behaviors in recordings |

---

## Session Log Format

### File Structure

```
logs/testflow/
├── sessions/
│   ├── session_2024-03-18_14-30-00_abcd1234.json
│   ├── session_2024-03-18_15-45-22_efgh5678.json
│   └── ...
├── scenarios/
│   ├── bug_fix_123.json          # Extracted scenario from bug report
│   ├── regression_test_auth.json  # Regression test case
│   └── ...
└── reports/
    ├── session_analysis_2024-03-18.json
    └── ...
```

### Session Schema

```json
{
  "version": "1.0.0",
  "metadata": {
    "session_id": "sess_abc123",
    "created_at": "2024-03-18T14:30:00Z",
    "ended_at": "2024-03-18T14:35:42Z",
    "duration_ms": 342000,
    "platform": "darwin",
    "rust_version": "1.76.0",
    "amadeus_version": "0.1.0",
    "feature_flags": ["full"],
    "config_snapshot": {
      "provider": "anthropic",
      "model": "claude-sonnet-4-5-20250929"
    }
  },

  "timeline": [
    {
      "seq": 0,
      "timestamp_ms": 0,
      "event_type": "session_start",
      "data": {}
    },
    {
      "seq": 1,
      "timestamp_ms": 15,
      "event_type": "user_input",
      "data": {
        "input_id": "inp_001",
        "content": "Create a hello world program",
        "source": "keyboard"
      }
    },
    {
      "seq": 2,
      "timestamp_ms": 120,
      "event_type": "llm_request",
      "data": {
        "request_id": "req_001",
        "turn": 1,
        "message_count": 2,
        "tools_available": ["bash", "read_file", "write_file"]
      }
    },
    {
      "seq": 3,
      "timestamp_ms": 450,
      "event_type": "agent_event",
      "data": {
        "event": "TextDelta",
        "delta": "I'll create"
      }
    },
    {
      "seq": 4,
      "timestamp_ms": 520,
      "event_type": "agent_event",
      "data": {
        "event": "ToolStart",
        "tool_id": "tool_001",
        "tool_name": "bash",
        "parent_id": null
      }
    },
    {
      "seq": 5,
      "timestamp_ms": 535,
      "event_type": "tool_input_stream",
      "data": {
        "tool_id": "tool_001",
        "delta": "{\"command\": \"echo"
      }
    },
    {
      "seq": 6,
      "timestamp_ms": 540,
      "event_type": "tool_input_stream",
      "data": {
        "tool_id": "tool_001",
        "delta": " 'hello world' > hello.txt\"}"
      }
    },
    {
      "seq": 7,
      "timestamp_ms": 550,
      "event_type": "approval_request",
      "data": {
        "approval_id": "appr_001",
        "tool_id": "tool_001",
        "tool_name": "bash",
        "input": {"command": "echo 'hello world' > hello.txt"},
        "reason": "Shell command execution"
      }
    },
    {
      "seq": 8,
      "timestamp_ms": 3200,
      "event_type": "approval_response",
      "data": {
        "approval_id": "appr_001",
        "decision": "approve"
      }
    },
    {
      "seq": 9,
      "timestamp_ms": 3250,
      "event_type": "tool_execution_start",
      "data": {
        "tool_id": "tool_001",
        "tool_name": "bash"
      }
    },
    {
      "seq": 10,
      "timestamp_ms": 3300,
      "event_type": "tool_output_stream",
      "data": {
        "tool_id": "tool_001",
        "delta": ""
      }
    },
    {
      "seq": 11,
      "timestamp_ms": 3310,
      "event_type": "agent_event",
      "data": {
        "event": "ToolComplete",
        "tool_id": "tool_001",
        "tool_name": "bash",
        "output": "",
        "is_error": false
      }
    },
    {
      "seq": 12,
      "timestamp_ms": 3400,
      "event_type": "gui_state_change",
      "data": {
        "component": "message_panel",
        "state": {
          "scroll_offset": 0,
          "message_count": 3,
          "last_message_type": "tool_result"
        }
      }
    },
    {
      "seq": 13,
      "timestamp_ms": 3450,
      "event_type": "gui_render",
      "data": {
        "frame_id": 42,
        "components_updated": ["message_panel", "status_bar", "tool_panel"],
        "render_duration_us": 850
      }
    },
    {
      "seq": 14,
      "timestamp_ms": 3500,
      "event_type": "keyboard_input",
      "data": {
        "key": "Enter",
        "modifiers": [],
        "context": "input_field"
      }
    },
    {
      "seq": 15,
      "timestamp_ms": 342000,
      "event_type": "session_end",
      "data": {
        "reason": "user_exit",
        "final_state": "completed"
      }
    }
  ],

  "summaries": {
    "total_turns": 3,
    "total_tools_executed": 5,
    "tools_by_name": {
      "bash": 2,
      "write_file": 2,
      "read_file": 1
    },
    "approvals_requested": 3,
    "approvals_approved": 3,
    "approvals_denied": 0,
    "total_tokens": {
      "input": 1500,
      "output": 800
    },
    "errors": [],
    "gui_stats": {
      "total_frames": 450,
      "avg_render_time_us": 720,
      "max_render_time_us": 2100
    }
  },

  "snapshots": {
    "final_history": [
      {"role": "user", "content": "Create a hello world program"},
      {"role": "assistant", "content": [{"type": "text", "text": "I'll create..."}]},
      {"role": "user", "content": [{"type": "tool_result", "content": "..."}]}
    ],
    "final_result": {
      "text": "I've created the hello world program.",
      "tool_calls": [...]
    }
  }
}
```

---

## Event Types

### Core Events (Agent Layer)

| Event Type | Description |
|------------|-------------|
| `session_start` | Session initialized |
| `session_end` | Session terminated |
| `user_input` | User submitted a prompt |
| `llm_request` | Agent making LLM API call |
| `llm_response` | LLM response received |
| `agent_event` | Any `AgentEvent` from the stream |
| `approval_request` | Tool requires approval |
| `approval_response` | User approval decision |
| `error` | Error occurred |

### Tool Events

| Event Type | Description |
|------------|-------------|
| `tool_input_stream` | Streaming tool input delta |
| `tool_execution_start` | Tool begins execution |
| `tool_output_stream` | Streaming tool output delta |
| `tool_complete` | Tool finished execution |

### GUI Events (TUI Layer)

| Event Type | Description |
|------------|-------------|
| `keyboard_input` | Key press in TUI |
| `mouse_input` | Mouse click/scroll |
| `gui_state_change` | Component state updated |
| `gui_render` | Frame rendered |
| `focus_change` | Input focus moved |
| `scroll` | User scrolled a panel |
| `resize` | Terminal resized |

---

## Implementation Components

### 1. SessionRecorder

**Purpose**: Captures and serializes all events during a live session.

**Location**: `src/test_utils/recorder.rs`

```rust
pub struct SessionRecorder {
    session_id: String,
    start_time: Instant,
    events: Vec<RecordedEvent>,
    config: RecorderConfig,
}

pub struct RecorderConfig {
    pub capture_gui_events: bool,
    pub capture_tool_io: bool,
    pub capture_llm_requests: bool,
    pub max_output_size: usize,  // Truncate large outputs
}

impl SessionRecorder {
    pub fn record(&mut self, event: RecordedEvent);
    pub fn save(&self, path: &Path) -> Result<()>;
    pub fn to_json(&self) -> Result<String>;
}
```

### 2. Event Hooks

**Purpose**: Integration points for capturing events from different layers.

**Agent Hook** (`src/agent/hooks/recorder_hook.rs`):
```rust
pub struct RecorderHook {
    recorder: Arc<Mutex<SessionRecorder>>,
}

impl Hook for RecorderHook {
    async fn on_event(&self, event: &AgentEvent);
    async fn on_tool_start(&self, tool: &str, input: &Value);
    async fn on_tool_complete(&self, tool: &str, output: &str);
}
```

**TUI Hook** (`src/ui/recorder.rs`):
```rust
pub struct TuiRecorder {
    recorder: Arc<Mutex<SessionRecorder>>,
}

impl TuiRecorder {
    pub fn on_key_event(&self, key: KeyEvent);
    pub fn on_render(&self, frame_id: u64, duration: Duration);
    pub fn on_state_change(&self, component: &str, state: Value);
}
```

### 3. PlaybackEngine

**Purpose**: Replay recorded sessions deterministically.

**Location**: `src/test_utils/playback.rs`

```rust
pub struct PlaybackEngine {
    session: SessionLog,
    mock_client: PlaybackMockClient,
    current_position: usize,
}

pub struct PlaybackConfig {
    pub speed: PlaybackSpeed,
    pub pause_on_approval: bool,
    pub auto_approve: Option<ApprovalDecision>,
    pub stop_on_error: bool,
}

pub enum PlaybackSpeed {
    RealTime,           // Use original timing
    Fast,               // Minimal delays
    Instant,            // No delays, sync execution
    Custom(f32),        // Speed multiplier
}

impl PlaybackEngine {
    pub async fn play(&mut self) -> Result<PlaybackResult>;
    pub async fn step(&mut self) -> Option<RecordedEvent>;
    pub fn seek_to(&mut self, seq: usize);
    pub fn pause(&mut self);
    pub fn resume(&mut self);
}
```

### 4. SessionAnalyzer

**Purpose**: Query and analyze recorded sessions.

**Location**: `src/test_utils/analyzer.rs`

```rust
pub struct SessionAnalyzer {
    session: SessionLog,
}

impl SessionAnalyzer {
    // Queries
    pub fn find_events(&self, filter: EventFilter) -> Vec<&RecordedEvent>;
    pub fn tool_calls(&self) -> Vec<ToolCallSummary>;
    pub fn errors(&self) -> Vec<ErrorSummary>;
    pub fn timing_analysis(&self) -> TimingAnalysis;

    // Assertions
    pub fn assert_event_order(&self, expected: &[EventType]);
    pub fn assert_tool_called(&self, tool: &str);
    pub fn assert_no_errors(&self);
    pub fn assert_approval_sequence(&self, expected: &[(&str, ApprovalDecision)]);

    // Debugging
    pub fn diff(&self, other: &SessionLog) -> SessionDiff;
    pub fn find_anomalies(&self) -> Vec<Anomaly>;
}
```

### 5. ScenarioExtractor

**Purpose**: Convert recorded sessions into reusable test scenarios.

**Location**: `src/test_utils/extractor.rs`

```rust
pub struct ScenarioExtractor;

impl ScenarioExtractor {
    pub fn from_session(session: &SessionLog) -> ScenarioDefinition;
    pub fn extract_llm_responses(session: &SessionLog) -> Vec<MockResponse>;
    pub fn to_test_case(session: &SessionLog) -> TestCase;
}
```

---

## Integration Points

### 1. Agent Integration

```rust
// In Agent::run_stream()
let recorder = Arc::new(Mutex::new(SessionRecorder::new(config)));

// Wrap the event stream
let recorded_stream = RecordableStream::new(
    original_stream,
    recorder.clone()
);

// Tool execution hook
impl ToolExecutor {
    async fn execute(&self, tool: &str, input: Value) -> Result<String> {
        self.recorder.lock().await.record(RecordedEvent::ToolStart { ... });
        let result = self.inner_execute(tool, input.clone()).await;
        self.recorder.lock().await.record(RecordedEvent::ToolComplete { ... });
        result
    }
}
```

### 2. TUI Integration

```rust
// In App::run()
let tui_recorder = TuiRecorder::new(recorder.clone());

// Key events
fn handle_key(&mut self, key: KeyEvent) {
    self.tui_recorder.on_key_event(key);
    // ... existing logic
}

// Render tracking
fn render(&mut self, frame: &mut Frame) {
    let start = Instant::now();
    // ... render logic
    self.tui_recorder.on_render(self.frame_count, start.elapsed());
}
```

### 3. Test Integration

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_recorded_session_replay() {
        let session = SessionLog::load("tests/fixtures/session_hello_world.json").unwrap();
        let engine = PlaybackEngine::new(session, PlaybackConfig::instant());
        let result = engine.play().await.unwrap();

        assert!(result.success);
        assert_eq!(result.tool_calls.len(), 2);
    }

    #[test]
    fn test_bug_reproduction() {
        // Load session from bug report
        let session = SessionLog::load("bugs/issue_123_session.json").unwrap();

        // Extract scenario
        let scenario = ScenarioExtractor::from_session(&session);

        // Run scenario
        let result = ScenarioRunner::new(scenario)
            .execute(mock_client)
            .await;

        // Verify bug is fixed
        assert!(!result.has_errors());
    }
}
```

---

## Usage Examples

### Recording a Live Session

```bash
# Enable recording via environment
export AMADEUS_RECORD_SESSION=true
export AMADEUS_RECORD_DIR=./logs/testflow/sessions

# Run agent (TUI or CLI)
cargo run --features full
```

### Analyzing a Session

```bash
# Analyze session
cargo run --features full -- analyze-session ./logs/testflow/sessions/session_xxx.json

# Output: timing analysis, tool usage, errors, anomalies
```

### Replaying a Session

```bash
# Replay with original timing
cargo run --features full -- replay-session ./logs/testflow/sessions/session_xxx.json

# Replay instantly for testing
cargo run --features full -- replay-session ./logs/testflow/sessions/session_xxx.json --speed instant
```

### Converting to Test

```bash
# Extract scenario from session
cargo run --features full -- extract-scenario ./logs/testflow/sessions/session_xxx.json \
  --output ./tests/scenarios/regression_issue_123.json
```

---

## File Locations

```
src/
├── test_utils/
│   ├── mod.rs
│   ├── recorder.rs          # SessionRecorder
│   ├── playback.rs          # PlaybackEngine
│   ├── analyzer.rs          # SessionAnalyzer
│   ├── extractor.rs         # ScenarioExtractor
│   ├── types.rs             # RecordedEvent, SessionLog, etc.
│   └── assertions.rs        # Test assertion helpers

tests/
├── testflow/
│   ├── recorder_test.rs     # Recorder tests
│   ├── playback_test.rs     # Playback tests
│   ├── analyzer_test.rs     # Analyzer tests
│   └── fixtures/
│       └── sample_session.json

logs/testflow/
├── sessions/                # Recorded sessions
├── scenarios/               # Extracted scenarios
└── reports/                 # Analysis reports
```

---

## Implementation Phases

### Phase 1: Core Recording (Priority: High)
- [ ] `SessionRecorder` with JSON serialization
- [ ] `RecordedEvent` types
- [ ] Agent event hook integration
- [ ] Basic CLI flag for recording

### Phase 2: TUI Integration (Priority: Medium)
- [ ] TUI event capture hooks
- [ ] Keyboard input recording
- [ ] Render timing capture
- [x] State snapshot capture
- [x] Frame-by-frame TUI buffer capture (`tui_capture.log`)

### Phase 3: Playback Engine (Priority: High)
- [ ] `PlaybackEngine` implementation
- [ ] `PlaybackMockClient` for deterministic replay
- [ ] Speed control options
- [ ] Approval auto-response

### Phase 4: Analysis Tools (Priority: Medium)
- [ ] `SessionAnalyzer` queries
- [ ] Event filtering
- [ ] Diff comparison
- [ ] Anomaly detection

### Phase 5: Test Integration (Priority: High)
- [ ] Scenario extraction
- [ ] Regression test generation
- [ ] CI integration
- [ ] Bug reproduction workflow

---

## Questions for Review

1. **Event Granularity**: Should we record every `TextDelta` or aggregate into complete messages?

2. **Sensitive Data**: How should we handle API keys and secrets in recordings? Redaction strategy?

3. **File Size**: For long sessions, should we:
   - Stream to disk incrementally?
   - Compress recordings automatically?
   - Implement rotation/archival?

4. **GUI Event Detail**: How detailed should TUI state capture be?
   - Full widget tree snapshots?
   - Just visible content?
   - Only key interaction points?

5. **Playback Fidelity**: For exact reproduction, do we need:
   - Deterministic UUIDs?
   - Fixed timestamps?
   - Mock file system state?

6. **CI Integration**: Should recordings be:
   - Committed to repo?
   - Stored externally (S3)?
   - Generated on-demand?

---

## Next Steps

After design approval:

1. Implement Phase 1 (Core Recording)
2. Add basic CLI integration
3. Create sample recordings
4. Implement Phase 3 (Playback) for basic replay
5. Iterate based on testing feedback
