use std::time::Instant;

use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use super::phrase_cycler::PhraseCycler;
use super::spinner::GeminiSpinner;
use crate::ui::get_colors;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StreamingState {
    Idle,
    Responding,
    WaitingForConfirmation,
}

pub struct LoadingIndicator {
    spinner: GeminiSpinner,
    phrase_cycler: PhraseCycler,
    streaming_state: StreamingState,
    start_time: Option<Instant>,
    show_spinner: bool,
}

impl LoadingIndicator {
    pub fn new() -> Self {
        Self {
            spinner: GeminiSpinner::new(),
            phrase_cycler: PhraseCycler::default(),
            streaming_state: StreamingState::Idle,
            start_time: None,
            show_spinner: true,
        }
    }

    pub fn set_streaming_state(&mut self, state: StreamingState) {
        let was_idle = self.streaming_state == StreamingState::Idle;
        let now_idle = state == StreamingState::Idle;

        if state == StreamingState::Responding && was_idle {
            self.start_time = Some(Instant::now());
            self.spinner.start();
        } else if state == StreamingState::Idle && !now_idle {
            self.start_time = None;
            self.spinner.stop();
            self.phrase_cycler.reset();
        }

        if state == StreamingState::WaitingForConfirmation {
            self.phrase_cycler.set_waiting_phrase(true);
        }

        self.streaming_state = state;
    }

    pub fn streaming_state(&self) -> StreamingState {
        self.streaming_state
    }

    pub fn is_active(&self) -> bool {
        self.streaming_state != StreamingState::Idle
    }

    pub fn tick(&mut self) {
        self.spinner.tick();

        let is_responding = self.streaming_state == StreamingState::Responding;
        self.phrase_cycler.tick(is_responding);
    }

    pub fn set_show_spinner(&mut self, show: bool) {
        self.show_spinner = show;
    }

    fn format_elapsed(&self) -> String {
        let elapsed = match self.start_time {
            Some(t) => t.elapsed().as_secs(),
            None => return String::new(),
        };

        if elapsed < 60 {
            format!("{}s", elapsed)
        } else {
            let mins = elapsed / 60;
            let secs = elapsed % 60;
            format!("{}m{}s", mins, secs)
        }
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        if area.width < 3 {
            return;
        }

        let colors = get_colors();
        let mut spans = Vec::new();

        if self.streaming_state == StreamingState::Idle {
            return;
        }

        if self.show_spinner {
            let spinner_text = self.spinner.get_frame();
            let spinner_color = self.spinner.get_current_color();

            spans.push(Span::styled(
                format!("{} ", spinner_text),
                Style::default().fg(spinner_color),
            ));
        }

        if let Some(phrase) = self.phrase_cycler.get_phrase() {
            let display_phrase = if phrase.len() > area.width.saturating_sub(20) as usize {
                format!("{}...", &phrase[..area.width.saturating_sub(23) as usize])
            } else {
                phrase.to_string()
            };

            spans.push(Span::styled(
                display_phrase,
                Style::default()
                    .fg(colors.text.primary)
                    .add_modifier(Modifier::ITALIC),
            ));
        }

        if self.streaming_state != StreamingState::WaitingForConfirmation {
            let elapsed = self.format_elapsed();
            if !elapsed.is_empty() {
                spans.push(Span::raw(" "));
                spans.push(Span::styled(
                    format!("(esc to cancel, {})", elapsed),
                    Style::default().fg(colors.text.secondary),
                ));
            }
        }

        let line = Line::from(spans);
        let paragraph = Paragraph::new(line);

        frame.render_widget(paragraph, area);
    }

    pub fn render_inline(&self, max_width: usize) -> Line<'static> {
        let colors = get_colors();
        let mut spans = Vec::new();

        if self.streaming_state == StreamingState::Idle {
            return Line::default();
        }

        if self.show_spinner {
            let spinner_text = self.spinner.get_frame();
            let spinner_color = self.spinner.get_current_color();

            spans.push(Span::styled(
                format!("{} ", spinner_text),
                Style::default().fg(spinner_color),
            ));
        }

        if let Some(phrase) = self.phrase_cycler.get_phrase() {
            let truncated = if phrase.len() > max_width.saturating_sub(20) {
                format!(
                    "{}...",
                    &phrase[..max_width.saturating_sub(23).min(phrase.len())]
                )
            } else {
                phrase.to_string()
            };

            spans.push(Span::styled(
                truncated,
                Style::default()
                    .fg(colors.text.primary)
                    .add_modifier(Modifier::ITALIC),
            ));
        }

        Line::from(spans)
    }

    pub fn get_elapsed_secs(&self) -> u64 {
        self.start_time.map(|t| t.elapsed().as_secs()).unwrap_or(0)
    }
}

impl Default for LoadingIndicator {
    fn default() -> Self {
        Self::new()
    }
}
