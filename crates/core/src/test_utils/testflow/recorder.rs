// @amadeus-header
// summary: Testflow support code for recorder.
// layer: infra
// status: active
// feature_flags:
// - api
// - concurrency
// - context
// - orchestra
// - test-utils
// - tui
// provides:
// - module: crate::test_utils::testflow::recorder
// - type: crate::test_utils::testflow::recorder::SessionRecorder
// - fn: crate::test_utils::testflow::recorder::load_session
// uses:
// - module: crate::agent::config::Config
// - module: crate::error::Result
// - runtime: tokio async runtime
// - runtime: chrono date and time utilities
// - artifact: filesystem paths and files
// invariants:
// - Listed interfaces stay aligned with the implementation in this file.
// side_effects:
// - Reads or writes filesystem state.
// - Writes output to stdout or stderr.
// tests:
// - cmd: cargo test --features full
// @end-amadeus-header

//! # Session Recorder
//!
//! Records all session events to a structured log for debugging and replay.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;
use std::{fs::OpenOptions, io::Write};

use chrono::Utc;
use tokio::sync::Mutex;
use uuid::Uuid;

use super::types::*;
use crate::agent::config::Config;
use crate::error::Result;

pub struct SessionRecorder {
    session: Arc<Mutex<SessionLog>>,
    start_time: Instant,
    seq_counter: AtomicU64,
    config: RecorderConfig,
    output_dir: PathBuf,
}

impl Clone for SessionRecorder {
    fn clone(&self) -> Self {
        Self {
            session: Arc::clone(&self.session),
            start_time: self.start_time,
            seq_counter: AtomicU64::new(self.seq_counter.load(Ordering::SeqCst)),
            config: self.config.clone(),
            output_dir: self.output_dir.clone(),
        }
    }
}

impl SessionRecorder {
    pub fn new(output_dir: impl Into<PathBuf>) -> Self {
        Self::with_config(output_dir, RecorderConfig::default())
    }

    pub fn with_config(output_dir: impl Into<PathBuf>, config: RecorderConfig) -> Self {
        let session_id = format!("sess_{}", Uuid::new_v4().simple());
        let now = Utc::now();

        let metadata = SessionMetadata {
            session_id: session_id.clone(),
            created_at: Some(now),
            platform: std::env::consts::OS.to_string(),
            rust_version: get_rust_version(),
            amadeus_version: env!("CARGO_PKG_VERSION").to_string(),
            feature_flags: get_enabled_features(),
            ..SessionMetadata::default()
        };

        let mut session = SessionLog::new();
        session.metadata = metadata;

        Self {
            session: Arc::new(Mutex::new(session)),
            start_time: Instant::now(),
            seq_counter: AtomicU64::new(0),
            config,
            output_dir: output_dir.into(),
        }
    }

    pub fn session_id(&self) -> String {
        self.session
            .try_lock()
            .map(|s| s.metadata.session_id.clone())
            .unwrap_or_default()
    }

    pub async fn set_config_snapshot(&self, config: &Config) {
        let mut session = self.session.lock().await;
        session.metadata.config_snapshot = ConfigSnapshot {
            provider: match config.provider {
                crate::agent::config::Provider::Anthropic => "anthropic".to_string(),
                crate::agent::config::Provider::OpenAI => "openai".to_string(),
            },
            model: config.model.clone(),
            workdir: config.workdir.to_string_lossy().to_string(),
            permission_mode: config.permission_mode.as_str().to_string(),
            config_roots: config
                .config_roots()
                .into_iter()
                .map(|root| root.to_string_lossy().to_string())
                .collect(),
            global_hook_path: Config::global_hooks_path()
                .map(|path| path.to_string_lossy().to_string()),
            workspace_hook_path: config.workspace_hooks_path().to_string_lossy().to_string(),
            agents_dir: config.agents_dir().to_string_lossy().to_string(),
            skills_dir: config.skills_dir().to_string_lossy().to_string(),
        };
    }

    pub async fn record(&self, event: RecordedEvent) {
        let seq = self.seq_counter.fetch_add(1, Ordering::SeqCst) as usize;
        let timestamp_ms = self.start_time.elapsed().as_millis() as u64;

        let timeline_event = TimelineEvent {
            seq,
            timestamp_ms,
            event_type: event,
        };

        let mut session = self.session.lock().await;
        session.push_event(timeline_event);
    }

    pub async fn record_session_start(&self) {
        self.record(RecordedEvent::SessionStart).await;
    }

    pub async fn record_session_end(&self, reason: SessionEndReason, final_state: SessionState) {
        self.record(RecordedEvent::SessionEnd {
            reason,
            final_state,
        })
        .await;
    }

    pub async fn record_user_input(&self, input_id: &str, content: &str, source: InputSource) {
        let redacted = self.redact_sensitive(content);
        self.record(RecordedEvent::UserInput {
            input_id: input_id.to_string(),
            content: redacted,
            source,
        })
        .await;
    }

    pub async fn record_llm_request(
        &self,
        request_id: &str,
        turn: usize,
        message_count: usize,
        tools_available: &[String],
    ) {
        if !self.config.capture_llm_requests {
            return;
        }
        self.record(RecordedEvent::LlmRequest {
            request_id: request_id.to_string(),
            turn,
            message_count,
            tools_available: tools_available.to_vec(),
        })
        .await;
    }

    pub async fn record_llm_response(
        &self,
        request_id: &str,
        stop_reason: &str,
        duration: std::time::Duration,
    ) {
        if !self.config.capture_llm_requests {
            return;
        }
        self.record(RecordedEvent::LlmResponse {
            request_id: request_id.to_string(),
            stop_reason: stop_reason.to_string(),
            duration_ms: duration.as_millis() as u64,
        })
        .await;
    }

    pub async fn record_agent_event(&self, event: crate::agent::events::AgentEvent) {
        let data = AgentEventData::from(event);
        self.record(RecordedEvent::AgentEvent { event: data }).await;
    }

    pub async fn record_approval_request(
        &self,
        approval_id: &str,
        tool_id: &str,
        tool_name: &str,
        input: &serde_json::Value,
        reason: &str,
    ) {
        let redacted_input = self.redact_json(input);
        self.record(RecordedEvent::ApprovalRequest {
            approval_id: approval_id.to_string(),
            tool_id: tool_id.to_string(),
            tool_name: tool_name.to_string(),
            input: redacted_input,
            reason: reason.to_string(),
        })
        .await;
    }

    pub async fn record_approval_response(&self, approval_id: &str, decision: ApprovalDecision) {
        self.record(RecordedEvent::ApprovalResponse {
            approval_id: approval_id.to_string(),
            decision,
        })
        .await;
    }

    pub async fn record_tool_start(&self, tool_id: &str, tool_name: &str) {
        if !self.config.capture_tool_io {
            return;
        }
        self.record(RecordedEvent::ToolExecutionStart {
            tool_id: tool_id.to_string(),
            tool_name: tool_name.to_string(),
        })
        .await;
    }

    pub async fn record_tool_input_stream(&self, tool_id: &str, delta: &str) {
        if !self.config.capture_tool_io {
            return;
        }
        let redacted = self.redact_sensitive(delta);
        self.record(RecordedEvent::ToolInputStream {
            tool_id: tool_id.to_string(),
            delta: redacted,
        })
        .await;
    }

    pub async fn record_tool_output_stream(&self, tool_id: &str, delta: &str) {
        if !self.config.capture_tool_io {
            return;
        }
        let truncated = self.truncate_output(delta);
        self.record(RecordedEvent::ToolOutputStream {
            tool_id: tool_id.to_string(),
            delta: truncated,
        })
        .await;
    }

    pub async fn record_tool_complete(
        &self,
        tool_id: &str,
        tool_name: &str,
        output: &str,
        is_error: bool,
        duration: std::time::Duration,
    ) {
        let truncated = self.truncate_output(output);
        self.record(RecordedEvent::ToolComplete {
            tool_id: tool_id.to_string(),
            tool_name: tool_name.to_string(),
            output: truncated,
            is_error,
            duration_ms: duration.as_millis() as u64,
        })
        .await;
    }

    pub async fn record_keyboard_input(&self, key: &str, modifiers: &[&str], context: &str) {
        if !self.config.capture_gui_events {
            return;
        }
        self.record(RecordedEvent::KeyboardInput {
            key: key.to_string(),
            modifiers: modifiers.iter().map(|s| s.to_string()).collect(),
            context: context.to_string(),
        })
        .await;
    }

    pub async fn record_gui_render(
        &self,
        frame_id: u64,
        components_updated: &[&str],
        render_duration: std::time::Duration,
    ) {
        if !self.config.capture_gui_events {
            return;
        }
        self.record(RecordedEvent::GuiRender {
            frame_id,
            components_updated: components_updated.iter().map(|s| s.to_string()).collect(),
            render_duration_us: render_duration.as_micros() as u64,
        })
        .await;
    }

    pub async fn record_gui_state_change(&self, component: &str, state: serde_json::Value) {
        if !self.config.capture_gui_events {
            return;
        }
        self.record(RecordedEvent::GuiStateChange {
            component: component.to_string(),
            state,
        })
        .await;
    }

    pub async fn record_tui_frame(&self, mut snapshot: TuiFrameSnapshot) -> Result<()> {
        if !self.config.capture_gui_events {
            return Ok(());
        }

        if snapshot.session_id.is_empty() {
            let session = self.session.lock().await;
            snapshot.session_id = session.metadata.session_id.clone();
        }

        std::fs::create_dir_all(&self.output_dir)?;
        let path = self.output_dir.join("tui_capture.log");
        let line = serde_json::to_string(&snapshot)?;

        let mut file = OpenOptions::new().create(true).append(true).open(path)?;
        writeln!(file, "{line}")?;

        Ok(())
    }

    pub async fn record_error(&self, message: &str, context: Option<&str>) {
        self.record(RecordedEvent::Error {
            message: message.to_string(),
            context: context.map(|s| s.to_string()),
        })
        .await;
    }

    pub async fn save(&self) -> Result<PathBuf> {
        let mut session = self.session.lock().await;
        session.finalize();

        std::fs::create_dir_all(&self.output_dir)?;

        let filename = format!(
            "session_{}_{}.json",
            session
                .metadata
                .created_at
                .map(|t: chrono::DateTime<chrono::Utc>| t.format("%Y-%m-%d_%H-%M-%S").to_string())
                .unwrap_or_else(|| "unknown".to_string()),
            &session.metadata.session_id[..12]
        );
        let path = self.output_dir.join(filename);

        let json = serde_json::to_string_pretty(&*session)?;
        std::fs::write(&path, json)?;

        Ok(path)
    }

    pub async fn to_json(&self) -> Result<String> {
        let session = self.session.lock().await;
        Ok(serde_json::to_string_pretty(&*session)?)
    }

    pub async fn get_session(&self) -> SessionLog {
        self.session.lock().await.clone()
    }

    fn redact_sensitive(&self, text: &str) -> String {
        let mut result = text.to_string();
        for pattern in &self.config.redact_patterns {
            result = result.replace(pattern, "[REDACTED]");
        }
        result
    }

    fn redact_json(&self, value: &serde_json::Value) -> serde_json::Value {
        let json_str = serde_json::to_string(value).unwrap_or_default();
        let redacted = self.redact_sensitive(&json_str);
        serde_json::from_str(&redacted).unwrap_or(value.clone())
    }

    fn truncate_output(&self, output: &str) -> String {
        if output.len() > self.config.max_output_size {
            let truncated_len = self.config.max_output_size;
            format!(
                "{}\n\n... [TRUNCATED: {} bytes total]",
                &output[..truncated_len],
                output.len()
            )
        } else {
            output.to_string()
        }
    }
}

#[allow(clippy::vec_init_then_push)]
fn get_enabled_features() -> Vec<String> {
    let mut features = Vec::new();
    #[cfg(feature = "tui")]
    features.push("tui".to_string());
    #[cfg(feature = "api")]
    features.push("api".to_string());
    #[cfg(feature = "orchestra")]
    features.push("orchestra".to_string());
    #[cfg(feature = "concurrency")]
    features.push("concurrency".to_string());
    #[cfg(feature = "context")]
    features.push("context".to_string());
    #[cfg(feature = "test-utils")]
    features.push("test-utils".to_string());
    features
}

fn get_rust_version() -> String {
    // Use a compile-time approach with CARGO_PKG_RUST_VERSION or fallback to unknown
    option_env!("CARGO_PKG_RUST_VERSION")
        .unwrap_or("unknown")
        .to_string()
}

pub fn load_session(path: &Path) -> Result<SessionLog> {
    let json = std::fs::read_to_string(path)?;
    let session = serde_json::from_str(&json)?;
    Ok(session)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_recorder_basic_flow() {
        let dir = TempDir::new().unwrap();
        let recorder = SessionRecorder::new(dir.path());

        recorder.record_session_start().await;
        recorder
            .record_user_input("inp_001", "Hello world", InputSource::Keyboard)
            .await;
        recorder
            .record_session_end(SessionEndReason::Completed, SessionState::Completed)
            .await;

        let session = recorder.get_session().await;
        assert_eq!(session.timeline.len(), 3);
        assert!(session.metadata.session_id.starts_with("sess_"));
    }

    #[tokio::test]
    async fn test_redaction() {
        let dir = TempDir::new().unwrap();
        let recorder = SessionRecorder::new(dir.path());

        recorder
            .record_user_input(
                "inp_001",
                "My key is sk-ant-12345secret",
                InputSource::Keyboard,
            )
            .await;

        let session = recorder.get_session().await;
        let event = &session.timeline[0];
        if let RecordedEvent::UserInput { content, .. } = &event.event_type {
            assert!(content.contains("[REDACTED]"));
            assert!(!content.contains("sk-ant-12345secret"));
        } else {
            panic!("Expected UserInput event");
        }
    }

    #[tokio::test]
    async fn test_save_and_load() {
        let dir = TempDir::new().unwrap();
        let recorder = SessionRecorder::new(dir.path());

        recorder.record_session_start().await;
        recorder
            .record_user_input("inp_001", "Test", InputSource::Keyboard)
            .await;
        recorder
            .record_session_end(SessionEndReason::Completed, SessionState::Completed)
            .await;

        let saved_path = recorder.save().await.unwrap();
        assert!(saved_path.exists());

        let loaded = load_session(&saved_path).unwrap();
        assert_eq!(loaded.timeline.len(), 3);
    }

    #[tokio::test]
    async fn test_timing_and_sequence() {
        let dir = TempDir::new().unwrap();
        let recorder = SessionRecorder::new(dir.path());

        recorder.record_session_start().await;
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        recorder
            .record_user_input("inp_001", "Test", InputSource::Keyboard)
            .await;

        let session = recorder.get_session().await;
        assert_eq!(session.timeline[0].seq, 0);
        assert_eq!(session.timeline[1].seq, 1);
        assert!(session.timeline[1].timestamp_ms >= 10);
    }

    #[tokio::test]
    async fn test_record_tui_frame_writes_jsonl() {
        let dir = TempDir::new().unwrap();
        let recorder = SessionRecorder::new(dir.path());

        recorder
            .record_tui_frame(TuiFrameSnapshot {
                session_id: String::new(),
                frame_id: 1,
                timestamp_ms: 42,
                width: 2,
                height: 1,
                cursor: Some(TuiCursorSnapshot {
                    x: 1,
                    y: 0,
                    visible: true,
                }),
                cells: vec![TuiCellSnapshot {
                    x: 0,
                    y: 0,
                    symbol: "A".to_string(),
                    fg: "red".to_string(),
                    bg: "black".to_string(),
                    underline_color: "reset".to_string(),
                    add_modifier: "BOLD".to_string(),
                    sub_modifier: "NONE".to_string(),
                }],
            })
            .await
            .unwrap();

        let capture_path = dir.path().join("tui_capture.log");
        assert!(capture_path.exists());

        let content = std::fs::read_to_string(capture_path).unwrap();
        assert!(content.contains("\"frame_id\":1"));
        assert!(content.contains("\"symbol\":\"A\""));
        assert!(content.contains("\"fg\":\"red\""));
    }

    #[test]
    fn test_load_session() {
        let json = r#"{
            "version": "1.0.0",
            "metadata": {
                "session_id": "sess_test",
                "created_at": null,
                "ended_at": null,
                "duration_ms": 0,
                "platform": "test",
                "rust_version": "1.0",
                "amadeus_version": "0.1.0",
                "feature_flags": [],
                "config_snapshot": {
                    "provider": "test",
                    "model": "test",
                    "workdir": "/tmp"
                }
            },
            "timeline": [],
            "summaries": {
                "total_turns": 0,
                "total_tools_executed": 0,
                "tools_by_name": {},
                "approvals_requested": 0,
                "approvals_approved": 0,
                "approvals_denied": 0,
                "total_tokens": 0,
                "compaction_events": 0,
                "tokens_saved_by_compaction": 0,
                "errors": [],
                "gui_stats": {
                    "total_frames": 0,
                    "avg_render_time_us": 0,
                    "max_render_time_us": 0
                }
            },
            "snapshots": {
                "final_history": [],
                "final_result": null
            }
        }"#;

        let path = std::env::temp_dir().join("test_session.json");
        std::fs::write(&path, json).unwrap();
        let session = load_session(&path).unwrap();
        assert_eq!(session.metadata.session_id, "sess_test");
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn test_load_sample_fixture() {
        let fixture_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("testflow")
            .join("fixtures")
            .join("sample_session.json");

        if !fixture_path.exists() {
            eprintln!(
                "Skipping fixture test - file not found at {:?}",
                fixture_path
            );
            return;
        }

        let session = load_session(&fixture_path).unwrap();
        assert_eq!(session.version, "1.0.0");
        assert_eq!(
            session.metadata.session_id,
            "sess_sample00000000000000000000000000"
        );
        assert_eq!(session.timeline.len(), 9);
        assert_eq!(session.summaries.total_turns, 1);
        assert_eq!(session.summaries.total_tools_executed, 1);
    }
}
