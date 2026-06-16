// @amadeus-header
// summary: Headless TUI driver that renders the real App against a TestBackend.
// layer: ui
// status: test-only
// feature_flags:
// - test-utils
// provides:
// - module: crate::ui::headless
// - type: crate::ui::headless::HeadlessApp
// uses:
// - module: crate::ui::app
// - type: amadeus_core::test_utils::testflow::types::TuiFrameSnapshot
// invariants:
// - Captured frames reflect the real App render, not stub data.
// side_effects: none
// tests:
// - cmd: cargo test -p tui --features test-utils headless
// @end-amadeus-header

//! Reusable headless driver for TUI tests. Integration tests supply the
//! `LLMClient` (e.g. `ScenarioMockClient`); this struct stays client-agnostic.

use std::sync::Arc;

use ratatui::{backend::TestBackend, Terminal};

use amadeus_core::agent::config::Config;
use amadeus_core::agent::loop_agent::Agent;
use amadeus_core::client::LLMClient;
use amadeus_core::test_utils::frame_text::render_frame_text;
use amadeus_core::test_utils::testflow::types::{TuiCellSnapshot, TuiFrameSnapshot};

use super::app::App;

pub struct HeadlessApp<C: LLMClient + Clone + 'static> {
    app: App<C>,
    terminal: Terminal<TestBackend>,
    width: u16,
    height: u16,
    frame_counter: u64,
}

impl<C: LLMClient + Clone + 'static> HeadlessApp<C> {
    pub fn new(client: C, workdir: &str, model: &str, width: u16, height: u16) -> Self {
        let agent = Agent::builder(client, Arc::new(Config::default())).build();
        let app = App::new(agent, workdir.into(), model.to_string());
        let terminal = Terminal::new(TestBackend::new(width, height)).expect("test terminal");
        Self { app, terminal, width, height, frame_counter: 0 }
    }

    /// Type a string into the input box (no submission).
    pub fn type_text(&mut self, text: &str) {
        let session = self.app.test_session_mut();
        for c in text.chars() {
            session.test_type_char(c);
        }
    }

    /// Submit current input and await the turn to settle.
    pub async fn submit(&mut self) {
        {
            let session = self.app.test_session_mut();
            let _ = session.test_submit().await;
        }
        self.settle().await;
    }

    /// Pump until the streaming turn finishes (or we give up after many yields).
    async fn settle(&mut self) {
        for _ in 0..10_000 {
            if !self.app.test_session_mut().test_is_streaming() {
                return;
            }
            tokio::task::yield_now().await;
        }
    }

    /// Render the current state and return a populated snapshot + its text form.
    pub fn capture(&mut self) -> (TuiFrameSnapshot, String) {
        let width = self.width;
        let height = self.height;
        // Split the mutable borrow of `self` so the closure can borrow `self.app`
        // while `self.terminal.draw` borrows `self.terminal` (disjoint fields).
        let app = &mut self.app;
        self.terminal
            .draw(|frame| {
                app.test_session_mut().test_render(frame);
            })
            .expect("draw");

        let buffer = self.terminal.backend().buffer();
        let mut cells = Vec::with_capacity((width as usize) * (height as usize));
        for y in 0..height {
            for x in 0..width {
                let cell = &buffer[(x, y)];
                let style = cell.style();
                cells.push(TuiCellSnapshot {
                    x,
                    y,
                    symbol: cell.symbol().to_string(),
                    fg: color_to_string(style.fg),
                    bg: color_to_string(style.bg),
                    underline_color: color_to_string(style.underline_color),
                    add_modifier: modifier_to_string(style.add_modifier),
                    sub_modifier: modifier_to_string(style.sub_modifier),
                });
            }
        }
        let frame_id = self.frame_counter;
        self.frame_counter += 1;
        let snapshot = TuiFrameSnapshot {
            session_id: "headless".to_string(),
            frame_id,
            timestamp_ms: 0,
            width,
            height,
            cursor: None,
            cells,
        };
        let text = render_frame_text(&snapshot);
        (snapshot, text)
    }
}

/// Mirror of `App::color_to_string`: ratatui style fields are `Option<Color>`.
fn color_to_string(color: Option<ratatui::style::Color>) -> String {
    match color {
        Some(c) => format!("{c:?}"),
        None => "default".to_string(),
    }
}

/// Mirror of `App::modifier_to_string` from app.rs:3747.
fn modifier_to_string(modifier: ratatui::style::Modifier) -> String {
    if modifier.is_empty() {
        return "NONE".to_string();
    }
    format!("{modifier:?}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use amadeus_core::benchmark::case::MockScript;
    use amadeus_core::benchmark::mock::BenchmarkMockClient;

    #[tokio::test]
    async fn typing_then_capture_shows_typed_text() {
        // Use a realistic terminal size: the live viewport + footer need room, so
        // very small heights (e.g. 6) collapse the input box out of the layout.
        let client = BenchmarkMockClient::new(MockScript { steps: Vec::new() });
        let mut app = HeadlessApp::new(client, ".", "m", 80, 24);
        app.type_text("hello");
        let (_snap, text) = app.capture();
        assert!(text.contains("hello"), "capture should show typed text:\n{text}");
    }

    #[test]
    fn capture_returns_full_grid_dimensions() {
        let client = BenchmarkMockClient::new(MockScript { steps: Vec::new() });
        let mut app = HeadlessApp::new(client, ".", "m", 12, 3);
        let (snap, _text) = app.capture();
        assert_eq!(snap.width, 12);
        assert_eq!(snap.height, 3);
        assert_eq!(snap.cells.len(), 12 * 3);
    }
}
