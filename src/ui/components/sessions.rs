// @amadeus-header
// summary: TUI component implementation for sessions.
// layer: ui
// status: active
// feature_flags:
// - tui
// provides:
// - module: crate::ui::components::sessions
// - type: crate::ui::components::sessions::SessionMetadata
// - type: crate::ui::components::sessions::SessionBrowser
// uses:
// - module: crate::agent::messages::Message
// - runtime: chrono date and time utilities
// - artifact: filesystem paths and files
// invariants:
// - Listed interfaces stay aligned with the implementation in this file.
// side_effects:
// - Reads or writes filesystem state.
// tests:
// - tests/tui_snapshot_test.rs
// @end-amadeus-header

//! # Session Browser Component
//!
//! Browse and load past conversation sessions.

use std::path::PathBuf;

use chrono::{DateTime, Local};

use crate::agent::messages::Message;

/// Metadata for a saved session.
#[derive(Debug, Clone)]
pub struct SessionMetadata {
    /// Session file path.
    pub path: PathBuf,
    /// Session timestamp.
    pub timestamp: DateTime<Local>,
    /// Number of messages in the session.
    pub message_count: usize,
    /// Model used.
    pub model: String,
}

/// Session browser component.
pub struct SessionBrowser {
    /// List of sessions.
    pub sessions: Vec<SessionMetadata>,
    /// Currently selected session.
    pub selected: usize,
}

impl SessionBrowser {
    /// Create a new session browser.
    pub fn new() -> Self {
        Self {
            sessions: Vec::new(),
            selected: 0,
        }
    }

    /// Load sessions from a log directory.
    pub fn load_sessions(&mut self, log_dir: &PathBuf) -> Result<(), std::io::Error> {
        let mut sessions = Vec::new();

        if !log_dir.exists() {
            return Ok(());
        }

        for entry_result in std::fs::read_dir(log_dir)? {
            let entry = entry_result?;
            let path = entry.path();
            if path
                .extension()
                .map(|e| e == "json" || e == "gz")
                .unwrap_or(false)
            {
                if let Some(meta) = self.read_metadata(&path).ok().flatten() {
                    sessions.push(meta);
                }
            }
        }

        // Sort by timestamp, newest first
        sessions.sort_by(|a, b| b.timestamp.cmp(&a.timestamp).reverse());

        self.sessions = sessions;
        Ok(())
    }

    /// Read session metadata from a file.
    fn read_metadata(&self, path: &PathBuf) -> Result<Option<SessionMetadata>, std::io::Error> {
        let content = std::fs::read_to_string(path)?;
        let json: serde_json::Value = match serde_json::from_str(&content) {
            Ok(v) => v,
            Err(_) => return Ok(None),
        };

        let timestamp = json
            .get("timestamp")
            .and_then(|t| t.as_str())
            .and_then(|t| DateTime::parse_from_rfc3339(t).ok())
            .map(|dt| dt.with_timezone(&Local))
            .unwrap_or_else(Local::now);

        let model = json
            .get("model")
            .and_then(|m| m.as_str())
            .unwrap_or("unknown")
            .to_string();

        let history = json
            .get("history")
            .and_then(|h| h.as_array())
            .map(|a| a.len())
            .unwrap_or(0);

        Ok(Some(SessionMetadata {
            path: path.clone(),
            timestamp,
            message_count: history,
            model,
        }))
    }

    /// Load messages from a session.
    pub fn load_session_messages(&self, index: usize) -> Result<Vec<Message>, std::io::Error> {
        if index >= self.sessions.len() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Session not found",
            ));
        }

        let session = &self.sessions[index];
        let content = std::fs::read_to_string(&session.path)?;

        let json: serde_json::Value = match serde_json::from_str(&content) {
            Ok(v) => v,
            Err(e) => return Err(std::io::Error::other(e.to_string())),
        };

        let history: Vec<Message> = json
            .get("history")
            .and_then(|h| serde_json::from_value(h.clone()).ok())
            .unwrap_or_default();

        Ok(history)
    }

    /// Select the next session.
    pub fn select_next(&mut self) {
        if !self.sessions.is_empty() {
            self.selected = (self.selected + 1) % self.sessions.len();
        }
    }

    /// Select the previous session.
    pub fn select_previous(&mut self) {
        if !self.sessions.is_empty() {
            self.selected = if self.selected == 0 {
                self.sessions.len() - 1
            } else {
                self.selected - 1
            };
        }
    }

    /// Get the currently selected session.
    pub fn selected_session(&self) -> Option<&SessionMetadata> {
        self.sessions.get(self.selected)
    }

    /// Get session count.
    pub fn len(&self) -> usize {
        self.sessions.len()
    }

    /// Check if there are no sessions.
    pub fn is_empty(&self) -> bool {
        self.sessions.is_empty()
    }
}

impl Default for SessionBrowser {
    fn default() -> Self {
        Self::new()
    }
}
