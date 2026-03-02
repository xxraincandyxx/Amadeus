use std::time::Instant;

use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::ui::get_colors;

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
    pub is_mesh: bool,
}

impl StatusBar {
    pub fn new(model_name: String) -> Self {
        Self {
            state: AppState::Idle,
            start_time: None,
            token_count: 0,
            model_name,
            spinner_frame: 0,
            is_mesh: false,
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

        let colors = get_colors();

        let (status_icon, status_color, status_text) = match self.state {
            AppState::Idle => (" ● ".to_string(), colors.ui.comment, "IDLE"),
            AppState::Processing => (
                format!(" {} ", self.get_spinner()),
                colors.text.link,
                "BUSY",
            ),
            AppState::Success => (" ✓ ".to_string(), colors.status.success, "DONE"),
            AppState::Error => (" ✗ ".to_string(), colors.status.error, "ERR "),
        };

        let mut left_spans = vec![
            Span::styled(
                status_icon,
                Style::default()
                    .fg(colors.background.primary)
                    .bg(status_color)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!(" {} ", status_text),
                Style::default()
                    .fg(status_color)
                    .bg(colors.ui.dark)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
        ];

        if self.is_mesh {
            left_spans.push(Span::styled(
                " MESH ",
                Style::default()
                    .fg(colors.background.primary)
                    .bg(colors.text.accent)
                    .add_modifier(Modifier::BOLD),
            ));
            left_spans.push(Span::raw(" "));
        }

        if self.state == AppState::Processing {
            if let Some(start) = self.start_time {
                let elapsed = start.elapsed();
                left_spans.push(Span::styled(
                    format!(
                        " {}.{:01}s ",
                        elapsed.as_secs(),
                        elapsed.subsec_millis() / 100
                    ),
                    Style::default().fg(colors.ui.comment),
                ));
            }
        }

        if self.token_count > 0 {
            left_spans.push(Span::styled(
                format!(" {} tokens ", self.token_count),
                Style::default()
                    .fg(colors.status.warning)
                    .add_modifier(Modifier::DIM),
            ));
        }

        let right_text = format!(" {} ", self.model_name.to_uppercase());
        let right_span = Span::styled(
            &right_text,
            Style::default()
                .fg(colors.text.accent)
                .bg(colors.ui.dark)
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
        let paragraph = Paragraph::new(line).style(Style::default().bg(colors.background.primary));

        frame.render_widget(paragraph, area);
    }
}

impl Default for StatusBar {
    fn default() -> Self {
        Self::new("claude-3-sonnet".to_string())
    }
}
