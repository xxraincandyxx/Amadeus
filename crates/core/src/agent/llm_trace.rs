// @amadeus-header
// summary: Durable JSONL trace of full LLM request/response payloads per agent turn.
// layer: agent
// status: active
// feature_flags: none
// provides:
// - module: crate::agent::llm_trace
// - type: crate::agent::llm_trace::LlmTraceSink
// - type: crate::agent::llm_trace::LlmTraceRequest
// - type: crate::agent::llm_trace::LlmTraceResponse
// - type: crate::agent::llm_trace::LlmTraceToolCall
// uses:
// - type: crate::agent::messages::Message
// - type: crate::error::{AgentError, Result}
// - protocol: serde serialization
// - format: JSONL trace file
// invariants:
// - Each provider call produces exactly one request record followed by one response record sharing the turn number.
// - The trace file is append-only JSONL; records are flushed before log_* returns so a crash mid-turn still leaves prior turns on disk.
// - The sink is cheaply shareable across sub-agents via Arc and safe to call from async contexts (the inner lock is never held across an await).
// side_effects:
// - Creates the trace directory and appends to llm_trace_<session_id>.jsonl under it.
// - Writes full unredacted prompt, history, tool inputs, and model output; intended for operator diagnosis only, not for public logs.
// tests:
// - cmd: cargo test -p core llm_trace --features full
// @end-amadeus-header

//! Standardized tracing of every LLM input and output.
//!
//! The agent loop calls [`LlmTraceSink::log_request`] just before handing a
//! request to the provider and [`LlmTraceSink::log_response`] once the streamed
//! response has been fully assembled. Together these records answer "what did
//! the model see and what did it say?" for any turn, which is the diagnostic
//! surface used when debugging agent behavior (e.g. benchmark runs).

use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use chrono::Utc;
use serde::Serialize;
use serde_json::Value;

use crate::agent::messages::Message;
use crate::error::{AgentError, Result};

/// Append-only JSONL sink capturing full provider request/response payloads.
pub struct LlmTraceSink {
    writer: Mutex<BufWriter<File>>,
    session_id: String,
    path: PathBuf,
}

/// Inputs to a single provider call. Borrowed to avoid cloning the whole
/// history; the sink serializes it once.
pub struct LlmTraceRequest<'a> {
    pub turn: usize,
    pub model: &'a str,
    pub system: &'a str,
    pub messages: &'a [Message],
    pub tools: &'a [Value],
    pub max_tokens: u32,
}

/// Assembled outputs of a single provider call, captured after the stream ends.
#[derive(Default)]
pub struct LlmTraceResponse {
    pub turn: usize,
    pub model: String,
    pub stop_reason: String,
    pub text: String,
    pub thinking: String,
    pub tool_calls: Vec<LlmTraceToolCall>,
    pub duration_ms: u64,
    pub input_tokens: Option<u32>,
    pub output_tokens: Option<u32>,
}

/// A tool call the model made during the turn.
#[derive(Serialize)]
pub struct LlmTraceToolCall {
    pub id: String,
    pub name: String,
    pub input: Value,
}

impl LlmTraceSink {
    /// Open (creating if needed) `<dir>/llm_trace_<session_id>.jsonl` for append.
    pub fn open(dir: impl AsRef<Path>, session_id: impl Into<String>) -> Result<Self> {
        let session_id = session_id.into();
        let dir = dir.as_ref();
        std::fs::create_dir_all(dir).map_err(AgentError::Io)?;
        let path = dir.join(format!("llm_trace_{}.jsonl", sanitize(&session_id)));

        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(AgentError::Io)?;

        Ok(Self {
            writer: Mutex::new(BufWriter::new(file)),
            session_id,
            path,
        })
    }

    /// Path of the underlying JSONL file.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Record the request side of a turn.
    pub fn log_request(&self, request: LlmTraceRequest<'_>) -> Result<()> {
        let record = serde_json::json!({
            "timestamp": Utc::now().to_rfc3339(),
            "session_id": self.session_id,
            "turn": request.turn,
            "kind": "request",
            "model": request.model,
            "system": request.system,
            "messages": request.messages,
            "tools": request.tools,
            "max_tokens": request.max_tokens,
        });
        self.write_line(&record)
    }

    /// Record the assembled response side of a turn.
    pub fn log_response(&self, response: LlmTraceResponse) -> Result<()> {
        let record = serde_json::json!({
            "timestamp": Utc::now().to_rfc3339(),
            "session_id": self.session_id,
            "turn": response.turn,
            "kind": "response",
            "model": response.model,
            "stop_reason": response.stop_reason,
            "text": response.text,
            "thinking": response.thinking,
            "tool_calls": response.tool_calls,
            "duration_ms": response.duration_ms,
            "input_tokens": response.input_tokens,
            "output_tokens": response.output_tokens,
        });
        self.write_line(&record)
    }

    fn write_line(&self, value: &Value) -> Result<()> {
        let mut guard = self.writer.lock().map_err(|_| {
            AgentError::InvalidResponse("llm trace writer lock poisoned".to_string())
        })?;
        serde_json::to_writer(&mut *guard, value)
            .map_err(|e| AgentError::InvalidResponse(format!("llm trace serialize: {e}")))?;
        guard.write_all(b"\n").map_err(AgentError::Io)?;
        guard.flush().map_err(AgentError::Io)?;
        Ok(())
    }
}

fn sanitize(value: &str) -> String {
    value
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::messages::Message;

    fn read_lines(path: &Path) -> Vec<Value> {
        let bytes = std::fs::read_to_string(path).expect("read trace file");
        bytes
            .lines()
            .filter(|l| !l.is_empty())
            .map(|l| serde_json::from_str(l).expect("jsonl line"))
            .collect()
    }

    #[test]
    fn writes_one_request_and_one_response_line() {
        let dir = tempfile::tempdir().expect("tempdir");
        let sink = LlmTraceSink::open(dir.path(), "sess-1").expect("open");
        let messages = vec![Message::user("hello")];
        let tools = vec![serde_json::json!({"name": "bash"})];

        sink.log_request(LlmTraceRequest {
            turn: 1,
            model: "test-model",
            system: "be helpful",
            messages: &messages,
            tools: &tools,
            max_tokens: 1024,
        })
        .expect("log_request");

        sink.log_response(LlmTraceResponse {
            turn: 1,
            model: "test-model".into(),
            stop_reason: "end_turn".into(),
            text: "hi there".into(),
            tool_calls: vec![LlmTraceToolCall {
                id: "t1".into(),
                name: "bash".into(),
                input: serde_json::json!({"command": "ls"}),
            }],
            duration_ms: 42,
            input_tokens: Some(10),
            output_tokens: Some(5),
            ..Default::default()
        })
        .expect("log_response");

        let lines = read_lines(sink.path());
        assert_eq!(lines.len(), 2);
        assert_eq!(
            lines[0].get("kind").and_then(|v| v.as_str()),
            Some("request")
        );
        assert_eq!(
            lines[1].get("kind").and_then(|v| v.as_str()),
            Some("response")
        );
        assert_eq!(lines[0].get("turn").and_then(|v| v.as_u64()), Some(1));
        // The full message history is captured on the request side.
        let msgs = lines[0].get("messages").expect("messages present");
        assert_eq!(msgs[0].get("role").and_then(|v| v.as_str()), Some("user"));
        // The model's tool call is captured on the response side.
        let tool_calls = lines[1].get("tool_calls").expect("tool_calls present");
        assert_eq!(
            tool_calls[0].get("name").and_then(|v| v.as_str()),
            Some("bash")
        );
        assert_eq!(
            lines[1].get("text").and_then(|v| v.as_str()),
            Some("hi there")
        );
    }

    #[test]
    fn sanitizes_session_id_in_filename() {
        let dir = tempfile::tempdir().expect("tempdir");
        let sink = LlmTraceSink::open(dir.path(), "session/1:2 3").expect("open");
        let filename = sink
            .path()
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        assert_eq!(filename, "llm_trace_session_1_2_3.jsonl");
    }

    #[test]
    fn appends_across_calls() {
        let dir = tempfile::tempdir().expect("tempdir");
        let sink = LlmTraceSink::open(dir.path(), "sess").expect("open");
        for turn in 1..=3 {
            sink.log_request(LlmTraceRequest {
                turn,
                model: "m",
                system: "",
                messages: &[],
                tools: &[],
                max_tokens: 1,
            })
            .expect("log_request");
        }
        let lines = read_lines(sink.path());
        assert_eq!(lines.len(), 3);
    }
}
