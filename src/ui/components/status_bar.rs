use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::ui::get_colors;

/// Status bar showing token speed and token counts.
pub struct StatusBar {
    /// Start time of current request.
    start_time: Option<std::time::Instant>,
    /// Estimated input tokens (from messages).
    input_tokens: u32,
    /// Estimated output tokens (counted from text deltas).
    output_tokens: u32,
    /// Last calculation time for speed.
    last_calc_time: Option<std::time::Instant>,
    /// Output tokens at last calculation.
    last_output_tokens: u32,
    /// Calculated tokens per second.
    tokens_per_second: f32,
    /// Whether the model is currently thinking.
    thinking: bool,
}

impl StatusBar {
    pub fn new() -> Self {
        Self {
            start_time: None,
            input_tokens: 0,
            output_tokens: 0,
            last_calc_time: None,
            last_output_tokens: 0,
            tokens_per_second: 0.0,
            thinking: false,
        }
    }

    /// Start tracking a new request.
    pub fn start(&mut self) {
        self.start_time = Some(std::time::Instant::now());
        self.input_tokens = 0;
        self.output_tokens = 0;
        self.last_calc_time = Some(std::time::Instant::now());
        self.last_output_tokens = 0;
        self.tokens_per_second = 0.0;
        self.thinking = true;
    }

    /// Update with text delta to count output tokens.
    /// Estimates ~4 characters per token.
    pub fn update_text(&mut self, delta: &str) {
        // Estimate tokens: ~4 chars per token
        let estimated_tokens = (delta.chars().count() as f32 / 4.0).ceil() as u32;
        self.output_tokens = self.output_tokens.saturating_add(estimated_tokens);
    }

    /// Update input token count.
    pub fn update_input_tokens(&mut self, tokens: u32) {
        self.input_tokens = tokens;
    }

    /// Set thinking state.
    pub fn set_thinking(&mut self, thinking: bool) {
        self.thinking = thinking;
    }

    /// Tick to recalculate speed.
    pub fn tick(&mut self) {
        if let Some(last_time) = self.last_calc_time {
            let elapsed = last_time.elapsed().as_secs_f32();

            // Recalculate every 200ms
            if elapsed >= 0.2 && self.output_tokens > self.last_output_tokens {
                let token_delta = self.output_tokens.saturating_sub(self.last_output_tokens);
                if elapsed > 0.0 {
                    self.tokens_per_second = token_delta as f32 / elapsed;
                }
                self.last_calc_time = Some(std::time::Instant::now());
                self.last_output_tokens = self.output_tokens;
            }
        }
    }

    /// Stop tracking and reset.
    pub fn stop(&mut self) {
        self.start_time = None;
        self.input_tokens = 0;
        self.output_tokens = 0;
        self.last_calc_time = None;
        self.last_output_tokens = 0;
        self.tokens_per_second = 0.0;
        self.thinking = false;
    }

    /// Check if active.
    pub fn is_active(&self) -> bool {
        self.start_time.is_some()
    }

    /// Format token count.
    fn format_tokens(count: u32) -> String {
        if count >= 1000 {
            format!("{:.1}k", count as f32 / 1000.0)
        } else {
            count.to_string()
        }
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        if area.width < 10 || area.height < 1 || self.start_time.is_none() {
            return;
        }

        let colors = get_colors();
        let mut spans = Vec::new();

        // Status text (thinking / generating)
        let status = if self.thinking {
            "thinking"
        } else {
            "generating"
        };
        spans.push(Span::styled(
            status,
            Style::default().fg(colors.text.secondary),
        ));
        spans.push(Span::styled(" • ", Style::default().fg(colors.ui.dark)));

        // Token speed
        if self.tokens_per_second >= 1.0 {
            spans.push(Span::styled(
                format!("{} tok/s", self.tokens_per_second.floor()),
                Style::default().fg(colors.text.primary),
            ));
            spans.push(Span::styled(" • ", Style::default().fg(colors.ui.dark)));
        }

        // Upload tokens (input)
        spans.push(Span::styled(
            "▲",
            Style::default().fg(colors.status.success),
        ));
        spans.push(Span::styled(
            format!(" {}", Self::format_tokens(self.input_tokens)),
            Style::default().fg(colors.text.secondary),
        ));
        spans.push(Span::styled(" • ", Style::default().fg(colors.ui.dark)));

        // Download tokens (output)
        spans.push(Span::styled(
            "▼",
            Style::default().fg(colors.text.accent),
        ));
        spans.push(Span::styled(
            format!(" {}", Self::format_tokens(self.output_tokens)),
            Style::default().fg(colors.text.secondary),
        ));

        // Thinking indicator
        if self.thinking {
            spans.push(Span::styled(" • ", Style::default().fg(colors.ui.dark)));
            spans.push(Span::styled(
                "⟡",
                Style::default().fg(colors.text.accent).add_modifier(Modifier::BOLD),
            ));
        }

        let line = Line::from(spans);
        let paragraph = Paragraph::new(line)
            .style(Style::default().bg(colors.background.primary))
            .left_aligned();

        frame.render_widget(paragraph, area);
    }
}

impl Default for StatusBar {
    fn default() -> Self {
        Self::new()
    }
}
