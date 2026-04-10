// @amadeus-header
// summary: TUI test support for harness.
// layer: test
// status: test-only
// feature_flags:
// - full
// provides:
// - module: tests::tui::harness
// - type: tests::tui::harness::TuiTestHarness
// - type: tests::tui::harness::InputEvent
// - type: tests::tui::harness::InputSequence
// - fn: tests::tui::harness::run_scenario
// uses:
// - module: amadeus::agent::config::Config
// - module: amadeus::client::LLMClient
// invariants:
// - Assertions stay aligned with current user-visible behavior.
// side_effects: none
// tests:
// - cmd: cargo test harness --features full
// @end-amadeus-header

//! TUI Test Harness
//!
//! Sets up a complete TUI testing environment with mocked LLM.

use std::sync::Arc;

use amadeus::agent::config::Config;
use amadeus::client::LLMClient;

use super::capture::{TuiCapture, TuiFrameSnapshot};

/// Test harness for TUI testing
pub struct TuiTestHarness {
    session_id: String,
    captures: Vec<TuiFrameSnapshot>,
    config: Arc<Config>,
}

impl TuiTestHarness {
    pub fn new(session_id: impl Into<String>) -> Self {
        Self {
            session_id: session_id.into(),
            captures: Vec::new(),
            config: Arc::new(Config::default()),
        }
    }

    pub fn with_config(mut self, config: Config) -> Self {
        self.config = Arc::new(config);
        self
    }

    /// Capture a frame with the current terminal state
    pub fn capture_frame(&mut self) -> TuiFrameSnapshot {
        let mut capture = TuiCapture::new(&self.session_id);
        let frame = capture.minimal();
        self.captures.push(frame.clone());
        frame
    }

    /// Get all captured frames
    pub fn captured_frames(&self) -> &[TuiFrameSnapshot] {
        &self.captures
    }

    /// Get the last captured frame
    pub fn last_frame(&self) -> Option<&TuiFrameSnapshot> {
        self.captures.last()
    }

    /// Clear all captures
    pub fn clear_captures(&mut self) {
        self.captures.clear();
    }
}

impl Default for TuiTestHarness {
    fn default() -> Self {
        Self::new("test_session")
    }
}

/// Input event for scripted user interactions
#[derive(Debug, Clone)]
pub enum InputEvent {
    Text(String),
    Enter,
    Escape,
    Ctrl(char),
    ArrowUp,
    ArrowDown,
    Tab,
}

/// Scripted user input sequence
#[derive(Debug, Clone, Default)]
pub struct InputSequence {
    events: Vec<InputEvent>,
}

impl InputSequence {
    pub fn new() -> Self {
        Self { events: Vec::new() }
    }

    pub fn type_text(mut self, text: &str) -> Self {
        self.events.push(InputEvent::Text(text.to_string()));
        self
    }

    pub fn enter(mut self) -> Self {
        self.events.push(InputEvent::Enter);
        self
    }

    pub fn escape(mut self) -> Self {
        self.events.push(InputEvent::Escape);
        self
    }

    pub fn ctrl(mut self, c: char) -> Self {
        self.events.push(InputEvent::Ctrl(c));
        self
    }

    pub fn arrow_up(mut self) -> Self {
        self.events.push(InputEvent::ArrowUp);
        self
    }

    pub fn arrow_down(mut self) -> Self {
        self.events.push(InputEvent::ArrowDown);
        self
    }

    pub fn tab(mut self) -> Self {
        self.events.push(InputEvent::Tab);
        self
    }

    pub fn then(mut self, other: InputSequence) -> Self {
        self.events.extend(other.events);
        self
    }

    pub fn len(&self) -> usize {
        self.events.len()
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }
}

impl From<&str> for InputSequence {
    fn from(text: &str) -> Self {
        Self::new().type_text(text)
    }
}

/// Run a complete test scenario
pub async fn run_scenario<C: LLMClient + Clone + 'static>(
    _client: C,
    input: InputSequence,
    width: u16,
    height: u16,
) -> Vec<TuiFrameSnapshot> {
    let mut captures = Vec::new();
    let mut capture = TuiCapture::new("scenario");

    // Capture initial state
    captures.push(capture.minimal());

    // Process input sequence
    for event in input.events {
        match event {
            InputEvent::Text(_) => {
                captures.push(capture.capture(width, height, &[]));
            }
            InputEvent::Enter => {
                captures.push(capture.capture(width, height, &[]));
            }
            InputEvent::Escape => {
                captures.push(capture.capture(width, height, &[]));
            }
            InputEvent::Ctrl(_) => {
                captures.push(capture.capture(width, height, &[]));
            }
            InputEvent::ArrowUp | InputEvent::ArrowDown => {
                captures.push(capture.capture(width, height, &[]));
            }
            InputEvent::Tab => {
                captures.push(capture.capture(width, height, &[]));
            }
        }
    }

    captures
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_harness_captures_frames() {
        let mut harness = TuiTestHarness::new("test");
        harness.capture_frame();
        harness.capture_frame();
        harness.capture_frame();

        assert_eq!(harness.captured_frames().len(), 3);
    }

    #[test]
    fn test_input_sequence_builder() {
        let seq = InputSequence::new()
            .type_text("Hello")
            .enter()
            .type_text("World")
            .enter();

        assert_eq!(seq.events.len(), 4);
    }

    #[test]
    fn test_input_sequence_chaining() {
        let seq = InputSequence::new()
            .type_text("Hello")
            .then(InputSequence::new().enter())
            .then(InputSequence::new().type_text("World"));

        assert_eq!(seq.events.len(), 3);
    }
}
