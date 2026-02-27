use std::time::Instant;

use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::ui::colors::THEME;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AppState {
    Idle,
    Processing,
    Success,
    Error,
}

pub struct StatusBar {
    state: AppState,
    start_time: Option<Instant>,
    token_count: usize,
    model_name: String,
    spinner_frame: usize,
}

impl StatusBar {
    pub fn new(model_name: String) -> Self {
        Self {
            state: AppState::Idle,
            start_time: None,
            token_count: 0,
            model_name,
            spinner_frame: 0,
        }
    }

    pub fn set_state(&mut self, state: AppState) {
        if state == AppState::Processing && self.start_time.is_none() {
            self.start_time = Some(Instant::now());
        } else if state != AppState::Processing {
            self.start_time = None;
        }
        self.state = state;
    }

    pub fn set_token_count(&mut self, count: usize) {
        self.token_count = count;
    }

    pub fn tick(&mut self) {
        self.spinner_frame = (self.spinner_frame + 1) % 10;
    }

    fn get_spinner(&self) -> &'static str {
        const SPINNER_FRAMES: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
        SPINNER_FRAMES[self.spinner_frame]
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        if area.width < 3 {
            return;
        }

        let (status_icon, status_color, status_text) = match self.state {
            AppState::Idle => (" ● ", THEME.comment, "IDLE"),
            AppState::Processing => (format!(" {} ", self.get_spinner()), THEME.cyan, "BUSY"),
            AppState::Success => (" ✓ ", THEME.green, "DONE"),
            AppState::Error => (" ✗ ", THEME.red, "ERR "),
        };

        let mut left_spans = vec![
            Span::styled(
                status_icon,
                Style::default()
                    .fg(THEME.bg)
                    .bg(status_color)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!(" {} ", status_text),
                Style::default()
                    .fg(status_color)
                    .bg(THEME.current_line)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
        ];

        if self.state == AppState::Processing {
            if let Some(start) = self.start_time {
                let elapsed = start.elapsed();
                left_spans.push(Span::styled(
                    format!(
                        " {}.{:01}s ",
                        elapsed.as_secs(),
                        elapsed.subsec_millis() / 100
                    ),
                    Style::default().fg(THEME.comment),
                ));
            }
        }

        if self.token_count > 0 {
            left_spans.push(Span::styled(
                format!(" {} tokens ", self.token_count),
                Style::default().fg(THEME.orange).add_modifier(Modifier::DIM),
            ));
        }

        let right_text = format!(" {} ", self.model_name.to_uppercase());
        let right_span = Span::styled(
            &right_text,
            Style::default()
                .fg(THEME.purple)
                .bg(THEME.current_line)
                .add_modifier(Modifier::BOLD),
        );

        let left_width: usize = left_spans.iter().map(|s| s.content.chars().count()).sum();
        let right_width = right_text.chars().count();
        let available = (area.width as usize).saturating_sub(left_width + right_width);

        if available > 0 {
            left_spans.push(Span::raw(" ".repeat(available)));
        }
        left_spans.push(right_span);

        let line = Line::from(left_spans);
        let paragraph = Paragraph::new(line).style(Style::default().bg(THEME.bg));

        frame.render_widget(paragraph, area);
    }
}

impl Default for StatusBar {
    fn default() -> Self {
        Self::new("claude-3-sonnet".to_string())
    }
}
