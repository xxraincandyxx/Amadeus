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

    #[allow(dead_code)]
    fn format_duration(&self) -> String {
        if let Some(start) = self.start_time {
            let elapsed = start.elapsed();
            let secs = elapsed.as_secs();
            let millis = elapsed.subsec_millis();
            if secs > 0 {
                format!("{}.{}s", secs, millis / 100)
            } else {
                format!("{}ms", millis)
            }
        } else {
            String::new()
        }
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        if area.width < 3 {
            return;
        }

        let (status_icon, status_color, status_text) = match self.state {
            AppState::Idle => ("●", THEME.comment, "Ready"),
            AppState::Processing => (self.get_spinner(), THEME.cyan, "Processing"),
            AppState::Success => ("✓", THEME.green, "Done"),
            AppState::Error => ("✗", THEME.red, "Error"),
        };

        let mut left_spans = vec![
            Span::raw(" "),
            Span::styled(
                status_icon,
                Style::default()
                    .fg(status_color)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
            Span::styled(status_text, Style::default().fg(status_color)),
        ];

        if self.state == AppState::Processing {
            if let Some(start) = self.start_time {
                let elapsed = start.elapsed();
                left_spans.push(Span::styled(
                    format!(
                        " · {}.{:02}s",
                        elapsed.as_secs(),
                        elapsed.subsec_millis() / 10
                    ),
                    Style::default().fg(THEME.comment),
                ));
            }
        }

        if self.token_count > 0 {
            left_spans.push(Span::styled(
                format!(" · tokens: {}", self.token_count),
                Style::default().fg(THEME.comment),
            ));
        }

        let right_side = format!(" {} ", self.model_name);
        let right_width = right_side.chars().count() as u16;
        let left_width: usize = left_spans.iter().map(|s| s.content.chars().count()).sum();
        let available = area.width.saturating_sub(right_width) as usize;

        if left_width < available {
            left_spans.push(Span::raw(" ".repeat(available - left_width)));
        }

        left_spans.push(Span::styled(right_side, Style::default().fg(THEME.comment)));

        let line = Line::from(left_spans);
        let paragraph = Paragraph::new(line).style(Style::default().bg(THEME.current_line));

        frame.render_widget(paragraph, area);
    }
}

impl Default for StatusBar {
    fn default() -> Self {
        Self::new("claude-sonnet".to_string())
    }
}
