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
    active_tool_label: Option<String>,
    active_tool_progress: Option<u8>,
    active_tool_count: usize,
    active_progress_message: Option<String>,
}

const SLIDING_BLOCK_FRAMES: &[&str] = &[
    "[=     ]", "[ =    ]", "[  =   ]", "[   =  ]", "[    = ]", "[     =]", "[    = ]", "[   =  ]",
    "[  =   ]", "[ =    ]",
];
const PULSE_FRAMES: &[&str] = &["·  ", "•• ", "•••", " ••", "  •"];

impl LoadingIndicator {
    pub fn new() -> Self {
        Self {
            spinner: GeminiSpinner::new(),
            phrase_cycler: PhraseCycler::default(),
            streaming_state: StreamingState::Idle,
            start_time: None,
            show_spinner: true,
            active_tool_label: None,
            active_tool_progress: None,
            active_tool_count: 0,
            active_progress_message: None,
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
            self.clear_activity_context();
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

    pub fn set_tool_activity_phrase(&mut self, tool_name: &str) {
        self.phrase_cycler.set_tool_activity_phrase(tool_name);
    }

    pub fn set_activity_context(
        &mut self,
        label: Option<String>,
        progress_message: Option<String>,
        progress: Option<u8>,
        count: usize,
    ) {
        self.active_tool_label = label;
        self.active_progress_message = progress_message;
        self.active_tool_progress = progress;
        self.active_tool_count = count;
    }

    pub fn clear_activity_context(&mut self) {
        self.active_tool_label = None;
        self.active_progress_message = None;
        self.active_tool_progress = None;
        self.active_tool_count = 0;
    }

    fn sliding_block_frame(&self) -> &'static str {
        let idx = self.spinner.frame_index() % SLIDING_BLOCK_FRAMES.len();
        SLIDING_BLOCK_FRAMES[idx]
    }

    fn pulse_frame(&self) -> &'static str {
        let idx = self.spinner.frame_index() % PULSE_FRAMES.len();
        PULSE_FRAMES[idx]
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

        spans.extend(self.primary_spans(area.width as usize));

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

        spans.extend(self.primary_spans(max_width));

        Line::from(spans)
    }

    pub fn render_detail_line(&self, max_width: usize) -> Option<Line<'static>> {
        if self.active_tool_count == 0 {
            return None;
        }

        let colors = get_colors();
        let label = self
            .active_tool_label
            .clone()
            .unwrap_or_else(|| "tool".to_string());
        let mut summary = format!("↳ {label}");

        if let Some(message) = &self.active_progress_message {
            summary.push_str(" • ");
            summary.push_str(message);
        }
        if let Some(progress) = self.active_tool_progress {
            summary.push_str(&format!(" • {progress}%"));
        }
        if self.active_tool_count > 1 {
            summary.push_str(&format!(" • {} active", self.active_tool_count));
        }

        let summary_len = summary.chars().count();
        let truncated = if summary_len > max_width {
            let keep = max_width.saturating_sub(1);
            let trimmed: String = summary.chars().take(keep).collect();
            format!("{trimmed}…")
        } else {
            summary
        };

        Some(Line::from(vec![
            Span::styled(self.pulse_frame(), Style::default().fg(colors.text.accent)),
            Span::raw(" "),
            Span::styled(
                truncated,
                Style::default()
                    .fg(colors.text.secondary)
                    .add_modifier(Modifier::ITALIC),
            ),
        ]))
    }

    fn primary_spans(&self, max_width: usize) -> Vec<Span<'static>> {
        let colors = get_colors();
        let mut spans = Vec::new();

        if let Some(phrase) = self.phrase_cycler.get_phrase() {
            let reserved = if self.active_tool_count > 0 { 34 } else { 20 };
            let truncated = if phrase.len() > max_width.saturating_sub(reserved) {
                format!(
                    "{}...",
                    &phrase[..max_width.saturating_sub(reserved + 3).min(phrase.len())]
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
            spans.push(Span::raw(" "));
            spans.push(Span::styled(
                self.sliding_block_frame(),
                Style::default().fg(colors.ui.comment),
            ));
        }

        if self.active_tool_count > 0 {
            let label = self
                .active_tool_label
                .clone()
                .unwrap_or_else(|| "tool".to_string());
            spans.push(Span::raw(" "));
            spans.push(Span::styled(
                format!("{} {label}", self.pulse_frame()),
                Style::default().fg(colors.text.accent),
            ));

            if let Some(progress) = self.active_tool_progress {
                spans.push(Span::raw(" "));
                spans.push(Span::styled(
                    format!("{progress}%"),
                    Style::default()
                        .fg(colors.status.success)
                        .add_modifier(Modifier::BOLD),
                ));
            }

            if self.active_tool_count > 1 {
                spans.push(Span::raw(" "));
                spans.push(Span::styled(
                    format!("{} tools", self.active_tool_count),
                    Style::default().fg(colors.text.secondary),
                ));
            }
        }

        spans
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
