use std::time::Instant;

use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState},
    Frame,
};

use crate::ui::colors::THEME;
use crate::ui::components::markdown::render_markdown;

#[derive(Debug, Clone, PartialEq)]
pub enum MessageRole {
    User,
    Assistant,
    Tool,
}

#[derive(Debug, Clone)]
pub struct Message {
    pub role: MessageRole,
    pub content: String,
    #[allow(dead_code)]
    pub timestamp: Instant,
    pub tool_name: Option<String>,
    pub is_collapsed: bool,
}

pub struct MessagesComponent {
    messages: Vec<Message>,
    scroll_offset: usize,
    auto_scroll: bool,
    streaming_text: Option<String>,
    total_lines: usize,
    viewport_height: usize,
}

impl MessagesComponent {
    pub fn new() -> Self {
        Self {
            messages: Vec::new(),
            scroll_offset: 0,
            auto_scroll: true,
            streaming_text: None,
            total_lines: 0,
            viewport_height: 0,
        }
    }

    pub fn add_user(&mut self, content: String) {
        self.messages.push(Message {
            role: MessageRole::User,
            content,
            timestamp: Instant::now(),
            tool_name: None,
            is_collapsed: false,
        });
        if self.auto_scroll {
            self.scroll_to_bottom();
        }
    }

    pub fn add_assistant(&mut self, content: String) {
        self.streaming_text = None;
        self.messages.push(Message {
            role: MessageRole::Assistant,
            content,
            timestamp: Instant::now(),
            tool_name: None,
            is_collapsed: false,
        });
        if self.auto_scroll {
            self.scroll_to_bottom();
        }
    }

    pub fn update_streaming_text(&mut self, text: &str) {
        self.streaming_text = Some(text.to_string());
        if self.auto_scroll {
            self.scroll_to_bottom();
        }
    }

    pub fn finalize_assistant(&mut self, text: String) {
        self.streaming_text = None;
        self.messages.push(Message {
            role: MessageRole::Assistant,
            content: text,
            timestamp: Instant::now(),
            tool_name: None,
            is_collapsed: false,
        });
        if self.auto_scroll {
            self.scroll_to_bottom();
        }
    }

    pub fn add_tool(&mut self, tool_name: String, content: String) {
        self.messages.push(Message {
            role: MessageRole::Tool,
            content,
            timestamp: Instant::now(),
            tool_name: Some(tool_name),
            is_collapsed: true,
        });
        if self.auto_scroll {
            self.scroll_to_bottom();
        }
    }

    pub fn scroll_up(&mut self, lines: usize) {
        self.auto_scroll = false;
        self.scroll_offset = self.scroll_offset.saturating_sub(lines);
    }

    pub fn scroll_down(&mut self, lines: usize) {
        self.scroll_offset = self.scroll_offset.saturating_add(lines);
    }

    pub fn scroll_to_bottom(&mut self) {
        self.scroll_offset = usize::MAX;
        self.auto_scroll = true;
    }

    pub fn scroll_to_ratio(&mut self, ratio: f32) {
        self.auto_scroll = false;
        let max_scroll = self.total_lines.saturating_sub(self.viewport_height);
        self.scroll_offset = ((ratio * max_scroll as f32) as usize).min(max_scroll);
    }

    pub fn toggle_collapse(&mut self, index: usize) {
        if let Some(msg) = self.messages.get_mut(index) {
            if msg.role == MessageRole::Tool {
                msg.is_collapsed = !msg.is_collapsed;
            }
        }
    }

    pub fn collapse_all(&mut self) {
        for msg in &mut self.messages {
            if msg.role == MessageRole::Tool {
                msg.is_collapsed = true;
            }
        }
    }

    pub fn render(&mut self, frame: &mut Frame, area: Rect) {
        if area.width < 3 || area.height < 1 {
            return;
        }

        let mut lines: Vec<Line> = Vec::new();
        let content_width = area.width.saturating_sub(3) as usize;

        for msg in self.messages.iter() {
            let role_prefix: Span = match msg.role {
                MessageRole::User => Span::styled(
                    "[You] ",
                    Style::default()
                        .fg(THEME.user_msg)
                        .add_modifier(Modifier::BOLD),
                ),
                MessageRole::Assistant => Span::styled(
                    "[Assistant] ",
                    Style::default()
                        .fg(THEME.assistant_msg)
                        .add_modifier(Modifier::BOLD),
                ),
                MessageRole::Tool => {
                    let name = msg.tool_name.as_deref().unwrap_or("tool");
                    Span::styled(
                        format!("[Tool: {}] ", name),
                        Style::default()
                            .fg(THEME.orange)
                            .add_modifier(Modifier::BOLD),
                    )
                }
            };

            if msg.role == MessageRole::Tool && msg.is_collapsed {
                lines.push(Line::from(vec![
                    Span::raw(" "),
                    role_prefix,
                    Span::styled("(collapsed)", Style::default().fg(THEME.comment)),
                ]));
            } else {
                let content_lines = render_markdown(&msg.content, content_width);

                for (i, content_line) in content_lines.into_iter().enumerate() {
                    if i == 0 {
                        let mut spans = vec![Span::raw(" "), role_prefix.clone()];
                        spans.extend(content_line.spans.into_iter());
                        lines.push(Line::from(spans));
                    } else {
                        let mut spans = vec![Span::raw(" ")];
                        spans.extend(content_line.spans.into_iter());
                        lines.push(Line::from(spans));
                    }
                }
            }

            lines.push(Line::from(""));
        }

        if let Some(ref streaming) = self.streaming_text {
            let role_prefix = Span::styled(
                "[Assistant] ",
                Style::default()
                    .fg(THEME.assistant_msg)
                    .add_modifier(Modifier::BOLD),
            );

            let content_lines = render_markdown(streaming, content_width);

            for (i, content_line) in content_lines.into_iter().enumerate() {
                if i == 0 {
                    let mut spans = vec![Span::raw(" "), role_prefix.clone()];
                    spans.extend(content_line.spans.into_iter());
                    lines.push(Line::from(spans));
                } else {
                    let mut spans = vec![Span::raw(" ")];
                    spans.extend(content_line.spans.into_iter());
                    lines.push(Line::from(spans));
                }
            }

            lines.push(Line::from(""));
        }

        let total_lines = lines.len();
        if total_lines == 0 {
            let empty_text = Paragraph::new(Line::from(vec![
                Span::raw(" "),
                Span::styled(
                    "Start a conversation by typing a message below...",
                    Style::default().fg(THEME.comment),
                ),
            ]))
            .style(Style::default().bg(THEME.bg));
            frame.render_widget(empty_text, area);
            return;
        }

        let visible_lines = area.height as usize;
        self.viewport_height = visible_lines;
        self.total_lines = total_lines;
        let max_scroll = total_lines.saturating_sub(visible_lines);
        let scroll_offset = self.scroll_offset.min(max_scroll);

        let visible_lines_vec: Vec<Line> = lines
            .into_iter()
            .skip(scroll_offset)
            .take(visible_lines)
            .collect();

        let paragraph = Paragraph::new(visible_lines_vec).style(Style::default().bg(THEME.bg));

        frame.render_widget(paragraph, area);

        if total_lines > visible_lines {
            let scrollbar = Scrollbar::default()
                .orientation(ScrollbarOrientation::VerticalRight)
                .begin_symbol(None)
                .end_symbol(None)
                .style(Style::default().fg(THEME.comment));

            let scrollbar_area = Rect::new(
                area.x + area.width.saturating_sub(1),
                area.y,
                1,
                area.height,
            );

            let mut scrollbar_state = ScrollbarState::new(total_lines)
                .position(scroll_offset)
                .viewport_content_length(visible_lines);

            frame.render_stateful_widget(scrollbar, scrollbar_area, &mut scrollbar_state);
        }
    }

    pub fn len(&self) -> usize {
        self.messages.len()
    }

    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }
}

impl Default for MessagesComponent {
    fn default() -> Self {
        Self::new()
    }
}
