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
use crate::ui::components::tool_group::{render_tool_group_with_limit, ToolGroup};

#[derive(Debug, Clone)]
pub enum HistoryItem {
    User { content: String, timestamp: Instant },
    Assistant { content: String, timestamp: Instant },
    ToolGroup { group: ToolGroup },
}

impl HistoryItem {
    pub fn user(content: String) -> Self {
        Self::User {
            content,
            timestamp: Instant::now(),
        }
    }

    pub fn assistant(content: String) -> Self {
        Self::Assistant {
            content,
            timestamp: Instant::now(),
        }
    }

    pub fn tool_group(group: ToolGroup) -> Self {
        Self::ToolGroup { group }
    }
}

pub struct MessagesComponent {
    items: Vec<HistoryItem>,
    scroll_offset: usize,
    auto_scroll: bool,
    streaming_text: Option<String>,
    total_lines: usize,
    viewport_height: usize,
    pending_tool_group: Option<ToolGroup>,
}

impl MessagesComponent {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            scroll_offset: 0,
            auto_scroll: true,
            streaming_text: None,
            total_lines: 0,
            viewport_height: 0,
            pending_tool_group: None,
        }
    }

    pub fn add_user(&mut self, content: String) {
        self.finalize_pending_tool_group();
        self.items.push(HistoryItem::user(content));
        if self.auto_scroll {
            self.scroll_to_bottom();
        }
    }

    pub fn add_assistant(&mut self, content: String) {
        self.finalize_pending_tool_group();
        self.streaming_text = None;
        self.items.push(HistoryItem::assistant(content));
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
        self.finalize_pending_tool_group();
        self.streaming_text = None;
        self.items.push(HistoryItem::assistant(text));
        if self.auto_scroll {
            self.scroll_to_bottom();
        }
    }

    pub fn start_tool(&mut self, tool_id: String, tool_name: String, command: Option<String>) {
        use crate::ui::components::tool_group::ToolCall;

        if self.pending_tool_group.is_none() {
            self.pending_tool_group = Some(ToolGroup::new());
        }

        if let Some(ref mut group) = self.pending_tool_group {
            let mut tool = ToolCall::new(tool_id, tool_name);
            if let Some(cmd) = command {
                tool = tool.with_command(cmd);
            }
            group.add_tool(tool);
        }

        if self.auto_scroll {
            self.scroll_to_bottom();
        }
    }

    pub fn complete_tool(
        &mut self,
        tool_id: &str,
        output: String,
        is_error: bool,
        command: Option<String>,
    ) {
        if let Some(ref mut group) = self.pending_tool_group {
            group.update_tool(tool_id, output, is_error);
            if let Some(cmd) = command {
                if let Some(tool) = group.tools.iter_mut().find(|t| t.id == tool_id) {
                    tool.command = Some(cmd);
                }
            }
        }
        if self.auto_scroll {
            self.scroll_to_bottom();
        }
    }

    fn finalize_pending_tool_group(&mut self) {
        if let Some(group) = self.pending_tool_group.take() {
            if !group.is_empty() {
                self.items.push(HistoryItem::tool_group(group));
            }
        }
    }

    pub fn collapse_all_tools(&mut self) {
        for item in &mut self.items {
            if let HistoryItem::ToolGroup { group } = item {
                group.collapse_all();
            }
        }
        if let Some(ref mut group) = self.pending_tool_group {
            group.collapse_all();
        }
    }

    pub fn expand_all_tools(&mut self) {
        for item in &mut self.items {
            if let HistoryItem::ToolGroup { group } = item {
                group.expand_all();
            }
        }
        if let Some(ref mut group) = self.pending_tool_group {
            group.expand_all();
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

    pub fn render(&mut self, frame: &mut Frame, area: Rect) {
        if area.width < 3 || area.height < 1 {
            return;
        }

        let mut lines: Vec<Line> = Vec::new();
        let content_width = area.width.saturating_sub(4) as usize;

        for item in self.items.iter() {
            match item {
                HistoryItem::User { content, .. } => {
                    lines.push(Line::from(vec![
                        Span::styled(
                            " ❯ ",
                            Style::default().fg(THEME.cyan).add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(
                            "YOU",
                            Style::default().fg(THEME.fg).add_modifier(Modifier::BOLD),
                        ),
                    ]));

                    let content_lines = render_markdown(content, content_width);
                    for content_line in content_lines {
                        let mut spans = vec![Span::raw("   ")];
                        spans.extend(content_line.spans.into_iter());
                        lines.push(Line::from(spans));
                    }
                    lines.push(Line::from(""));
                }

                HistoryItem::Assistant { content, .. } => {
                    lines.push(Line::from(vec![
                        Span::styled(
                            " ❯ ",
                            Style::default()
                                .fg(THEME.purple)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(
                            "AMADEUS",
                            Style::default().fg(THEME.fg).add_modifier(Modifier::BOLD),
                        ),
                    ]));

                    let content_lines = render_markdown(content, content_width);
                    for content_line in content_lines {
                        let mut spans = vec![Span::raw("   ")];
                        spans.extend(content_line.spans.into_iter());
                        lines.push(Line::from(spans));
                    }
                    lines.push(Line::from(""));
                }

                HistoryItem::ToolGroup { group } => {
                    let tool_lines =
                        render_tool_group_with_limit(group, area, area.height as usize);
                    for line in tool_lines {
                        lines.push(line);
                    }
                    lines.push(Line::from(""));
                }
            }
        }

        if let Some(ref group) = self.pending_tool_group {
            let tool_lines = render_tool_group_with_limit(group, area, area.height as usize);
            for line in tool_lines {
                lines.push(line);
            }
            lines.push(Line::from(""));
        }

        if let Some(ref streaming) = self.streaming_text {
            lines.push(Line::from(vec![
                Span::styled(
                    " ❯ ",
                    Style::default()
                        .fg(THEME.purple)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    "AMADEUS",
                    Style::default().fg(THEME.fg).add_modifier(Modifier::BOLD),
                ),
            ]));

            let content_lines = render_markdown(streaming, content_width);
            for content_line in content_lines {
                let mut spans = vec![Span::raw("   ")];
                spans.extend(content_line.spans.into_iter());
                lines.push(Line::from(spans));
            }
            lines.push(Line::from(""));
        }

        let total_lines = lines.len();
        if total_lines == 0 {
            let empty_lines = vec![
                Line::from(""),
                Line::from(vec![
                    Span::raw("   "),
                    Span::styled(
                        "Waiting for your instructions...",
                        Style::default()
                            .fg(THEME.comment)
                            .add_modifier(Modifier::ITALIC),
                    ),
                ]),
            ];
            frame.render_widget(
                Paragraph::new(empty_lines).style(Style::default().bg(THEME.bg)),
                area,
            );
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
                .style(Style::default().fg(THEME.border));

            let mut scrollbar_state = ScrollbarState::new(total_lines)
                .position(scroll_offset)
                .viewport_content_length(visible_lines);

            frame.render_stateful_widget(scrollbar, area, &mut scrollbar_state);
        }
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

impl Default for MessagesComponent {
    fn default() -> Self {
        Self::new()
    }
}
