use std::time::Instant;

use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState},
    Frame,
};

use crate::ui::components::markdown::render_markdown;
use crate::ui::components::spinner::GeminiSpinner;
use crate::ui::components::tool_group::{render_tool_group_with_limit, ToolGroup};
use crate::ui::get_colors;
use crate::ui::scroll::{AnimatedScrollbar, ScrollState};

/// Status of a compression operation
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CompressionStatus {
    /// Compression is in progress
    Pending,
    /// Compression completed successfully
    Compressed,
    /// Compression did not reduce token count
    NotBeneficial,
    /// Compression failed with an error
    Failed,
    /// Nothing to compress
    Noop,
}

/// Data for a compression history item
#[derive(Debug, Clone)]
pub struct CompressionItem {
    pub is_pending: bool,
    pub original_token_count: Option<usize>,
    pub new_token_count: Option<usize>,
    pub status: CompressionStatus,
    pub error_message: Option<String>,
}

impl CompressionItem {
    pub fn pending() -> Self {
        Self {
            is_pending: true,
            original_token_count: None,
            new_token_count: None,
            status: CompressionStatus::Pending,
            error_message: None,
        }
    }

    pub fn completed(original: usize, new: usize) -> Self {
        Self {
            is_pending: false,
            original_token_count: Some(original),
            new_token_count: Some(new),
            status: CompressionStatus::Compressed,
            error_message: None,
        }
    }

    pub fn not_beneficial(original: usize) -> Self {
        Self {
            is_pending: false,
            original_token_count: Some(original),
            new_token_count: Some(original),
            status: CompressionStatus::NotBeneficial,
            error_message: None,
        }
    }

    pub fn failed(message: String) -> Self {
        Self {
            is_pending: false,
            original_token_count: None,
            new_token_count: None,
            status: CompressionStatus::Failed,
            error_message: Some(message),
        }
    }

    pub fn noop() -> Self {
        Self {
            is_pending: false,
            original_token_count: None,
            new_token_count: None,
            status: CompressionStatus::Noop,
            error_message: None,
        }
    }
}

#[derive(Debug, Clone)]
pub enum HistoryItem {
    User { content: String, timestamp: Instant },
    Assistant { content: String, timestamp: Instant },
    ToolGroup { group: ToolGroup },
    /// Compression/compaction operation (gemini-cli style)
    Compression { compression: CompressionItem },
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

    pub fn compression(compression: CompressionItem) -> Self {
        Self::Compression { compression }
    }
}

pub struct MessagesComponent {
    items: Vec<HistoryItem>,
    scroll_state: ScrollState,
    scrollbar: AnimatedScrollbar,
    streaming_text: Option<String>,
    pending_tool_group: Option<ToolGroup>,
    /// Pending compression item with animated spinner (gemini-cli style)
    pending_compression: Option<CompressionItem>,
    /// Spinner for pending compression animation
    compression_spinner: GeminiSpinner,
}

impl MessagesComponent {
    pub fn new() -> Self {
        let colors = get_colors();
        Self {
            items: Vec::new(),
            scroll_state: ScrollState::new(),
            scrollbar: AnimatedScrollbar::new(colors.scrollbar.thumb, colors.scrollbar.thumb_hover),
            streaming_text: None,
            pending_tool_group: None,
            pending_compression: None,
            compression_spinner: GeminiSpinner::new(),
        }
    }

    pub fn add_user(&mut self, content: String) {
        self.finalize_pending_tool_group();
        self.items.push(HistoryItem::user(content));
        if self.scroll_state.auto_scroll {
            self.scroll_state.scroll_to_bottom();
            self.scrollbar.flash();
        }
    }

    pub fn add_assistant(&mut self, content: String) {
        self.finalize_pending_tool_group();
        self.streaming_text = None;
        self.items.push(HistoryItem::assistant(content));
        if self.scroll_state.auto_scroll {
            self.scroll_state.scroll_to_bottom();
            self.scrollbar.flash();
        }
    }

    pub fn update_streaming_text(&mut self, text: &str) {
        self.streaming_text = Some(text.to_string());
        if self.scroll_state.auto_scroll {
            self.scroll_state.scroll_to_bottom();
            self.scrollbar.flash();
        }
    }

    pub fn finalize_assistant(&mut self, text: String) {
        self.finalize_pending_tool_group();
        self.streaming_text = None;
        self.items.push(HistoryItem::assistant(text));
        if self.scroll_state.auto_scroll {
            self.scroll_state.scroll_to_bottom();
            self.scrollbar.flash();
        }
    }

    /// Start a pending compression operation (shows animated spinner)
    pub fn start_compression(&mut self) {
        self.pending_compression = Some(CompressionItem::pending());
        self.compression_spinner.start();
        if self.scroll_state.auto_scroll {
            self.scroll_state.scroll_to_bottom();
            self.scrollbar.flash();
        }
    }

    /// Complete the pending compression with results
    pub fn complete_compression(&mut self, original_tokens: usize, new_tokens: usize) {
        self.compression_spinner.stop();
        let completed = CompressionItem::completed(original_tokens, new_tokens);
        self.items.push(HistoryItem::compression(completed));
        self.pending_compression = None;
        if self.scroll_state.auto_scroll {
            self.scroll_state.scroll_to_bottom();
            self.scrollbar.flash();
        }
    }

    /// Complete compression with "not beneficial" result
    pub fn complete_compression_not_beneficial(&mut self, original_tokens: usize) {
        self.compression_spinner.stop();
        let completed = CompressionItem::not_beneficial(original_tokens);
        self.items.push(HistoryItem::compression(completed));
        self.pending_compression = None;
        if self.scroll_state.auto_scroll {
            self.scroll_state.scroll_to_bottom();
            self.scrollbar.flash();
        }
    }

    /// Complete compression with error
    pub fn complete_compression_failed(&mut self, error: String) {
        self.compression_spinner.stop();
        let completed = CompressionItem::failed(error);
        self.items.push(HistoryItem::compression(completed));
        self.pending_compression = None;
        if self.scroll_state.auto_scroll {
            self.scroll_state.scroll_to_bottom();
            self.scrollbar.flash();
        }
    }

    /// Complete compression with nothing to compress
    pub fn complete_compression_noop(&mut self) {
        self.compression_spinner.stop();
        let completed = CompressionItem::noop();
        self.items.push(HistoryItem::compression(completed));
        self.pending_compression = None;
        if self.scroll_state.auto_scroll {
            self.scroll_state.scroll_to_bottom();
            self.scrollbar.flash();
        }
    }

    /// Check if compression is in progress
    pub fn is_compression_pending(&self) -> bool {
        self.pending_compression.is_some()
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

        if self.scroll_state.auto_scroll {
            self.scroll_state.scroll_to_bottom();
            self.scrollbar.flash();
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
        if self.scroll_state.auto_scroll {
            self.scroll_state.scroll_to_bottom();
            self.scrollbar.flash();
        }
    }

    /// Update progress for a running tool.
    pub fn update_tool_progress(&mut self, tool_id: &str, message: String, percent: Option<u8>) {
        if let Some(ref mut group) = self.pending_tool_group {
            if let Some(tool) = group.tools.iter_mut().find(|t| t.id == tool_id) {
                tool.progress_message = Some(message);
                tool.progress_percent = percent;
            }
        }
        if self.scroll_state.auto_scroll {
            self.scroll_state.scroll_to_bottom();
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
        self.scroll_state.scroll_up(lines);
        self.scrollbar.flash();
    }

    pub fn scroll_down(&mut self, lines: usize) {
        self.scroll_state.scroll_down(lines);
        self.scrollbar.flash();
    }

    pub fn scroll_page_up(&mut self) {
        self.scroll_state.scroll_page_up();
        self.scrollbar.flash();
    }

    pub fn scroll_page_down(&mut self) {
        self.scroll_state.scroll_page_down();
        self.scrollbar.flash();
    }

    pub fn scroll_to_top(&mut self) {
        self.scroll_state.scroll_to_top();
        self.scrollbar.flash();
    }

    pub fn scroll_to_bottom(&mut self) {
        self.scroll_state.scroll_to_bottom();
        self.scrollbar.flash();
    }

    pub fn scroll_to_ratio(&mut self, ratio: f32) {
        self.scroll_state.scroll_to_ratio(ratio);
        self.scrollbar.flash();
    }

    pub fn flash_scrollbar(&mut self) {
        self.scrollbar.flash();
    }

    pub fn update_scrollbar_colors(&mut self) {
        let colors = get_colors();
        self.scrollbar =
            AnimatedScrollbar::new(colors.scrollbar.thumb, colors.scrollbar.thumb_hover);
    }

    /// Tick for animation updates (compression spinner, etc.)
    pub fn tick(&mut self) {
        if self.pending_compression.is_some() {
            self.compression_spinner.tick();
        }
    }

    pub fn render(&mut self, frame: &mut Frame, area: Rect) {
        if area.width < 3 || area.height < 1 {
            return;
        }

        self.scrollbar.update();

        let colors = get_colors();
        let mut lines: Vec<Line> = Vec::new();
        let content_width = area.width.saturating_sub(4) as usize;

        for item in self.items.iter() {
            match item {
                HistoryItem::User { content, .. } => {
                    let content_lines = render_markdown(content, content_width);
                    for (i, content_line) in content_lines.into_iter().enumerate() {
                        let mut spans = Vec::new();
                        if i == 0 {
                            spans.push(Span::styled(
                                "> ",
                                Style::default()
                                    .fg(colors.text.link)
                                    .add_modifier(Modifier::BOLD),
                            ));
                        } else {
                            spans.push(Span::raw("  "));
                        }
                        spans.extend(content_line.spans.into_iter());
                        lines.push(Line::from(spans));
                    }
                    lines.push(Line::from(""));
                }

                HistoryItem::Assistant { content, .. } => {
                    let content_lines = render_markdown(content, content_width);
                    for (i, content_line) in content_lines.into_iter().enumerate() {
                        let mut spans = Vec::new();
                        if i == 0 {
                            spans.push(Span::styled(
                                "✦ ",
                                Style::default()
                                    .fg(colors.text.accent)
                                    .add_modifier(Modifier::BOLD),
                            ));
                        } else {
                            spans.push(Span::raw("  "));
                        }
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

                HistoryItem::Compression { compression } => {
                    let text = Self::get_compression_text(compression);
                    let color = if compression.status == CompressionStatus::Failed {
                        colors.status.error
                    } else if compression.status == CompressionStatus::NotBeneficial {
                        colors.status.warning
                    } else {
                        colors.status.success
                    };

                    lines.push(Line::from(vec![
                        Span::styled("✦ ", Style::default().fg(color)),
                        Span::styled(text, Style::default().fg(color)),
                    ]));
                    lines.push(Line::from(""));
                }
            }
        }

        // Render pending compression with animated spinner (gemini-cli style)
        if let Some(ref _compression) = self.pending_compression {
            let spinner_text = self.compression_spinner.get_frame();
            let spinner_color = self.compression_spinner.get_current_color();

            lines.push(Line::from(vec![
                Span::styled(format!("{} ", spinner_text), Style::default().fg(spinner_color)),
                Span::styled(
                    "Compacting chat history...",
                    Style::default()
                        .fg(colors.text.accent)
                        .add_modifier(Modifier::ITALIC),
                ),
            ]));
            lines.push(Line::from(""));
        }

        if let Some(ref group) = self.pending_tool_group {
            let tool_lines = render_tool_group_with_limit(group, area, area.height as usize);
            for line in tool_lines {
                lines.push(line);
            }
            lines.push(Line::from(""));
        }

        if let Some(ref streaming) = self.streaming_text {
            let content_lines = render_markdown(streaming, content_width);
            for (i, content_line) in content_lines.into_iter().enumerate() {
                let mut spans = Vec::new();
                if i == 0 {
                    spans.push(Span::styled(
                        "✦ ",
                        Style::default()
                            .fg(colors.text.accent)
                            .add_modifier(Modifier::BOLD),
                    ));
                } else {
                    spans.push(Span::raw("  "));
                }
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
                            .fg(colors.ui.comment)
                            .add_modifier(Modifier::ITALIC),
                    ),
                ]),
            ];
            frame.render_widget(
                Paragraph::new(empty_lines).style(Style::default().bg(colors.background.primary)),
                area,
            );
            return;
        }

        let visible_lines = area.height as usize;
        self.scroll_state.update_content(total_lines, visible_lines);
        let scroll_offset = self.scroll_state.effective_offset();

        let visible_lines_vec: Vec<Line> = lines
            .into_iter()
            .skip(scroll_offset)
            .take(visible_lines)
            .collect();

        let paragraph =
            Paragraph::new(visible_lines_vec).style(Style::default().bg(colors.background.primary));

        frame.render_widget(paragraph, area);

        if total_lines > visible_lines {
            let thumb_color = self.scrollbar.thumb_color();

            let scrollbar = Scrollbar::default()
                .orientation(ScrollbarOrientation::VerticalRight)
                .begin_symbol(None)
                .end_symbol(None)
                .thumb_symbol("▐")
                .track_symbol(Some("│"))
                .style(Style::default().fg(thumb_color));

            let mut scrollbar_state = ScrollbarState::new(total_lines)
                .position(scroll_offset)
                .viewport_content_length(visible_lines);

            frame.render_stateful_widget(scrollbar, area, &mut scrollbar_state);
        }
    }

    /// Get display text for a compression item (gemini-cli style)
    fn get_compression_text(compression: &CompressionItem) -> String {
        match compression.status {
            CompressionStatus::Compressed => {
                let original = compression.original_token_count.unwrap_or(0);
                let new = compression.new_token_count.unwrap_or(0);
                let saved = original.saturating_sub(new);
                let percent = if original > 0 {
                    saved * 100 / original
                } else {
                    0
                };
                format!(
                    "Chat history compacted: {} → {} tokens (saved {}%, ~{} tokens)",
                    original, new, percent, saved
                )
            }
            CompressionStatus::NotBeneficial => {
                let original = compression.original_token_count.unwrap_or(0);
                if original < 50000 {
                    "Compression was not beneficial for this history size.".to_string()
                } else {
                    "Compression did not reduce size. Try again with more context.".to_string()
                }
            }
            CompressionStatus::Failed => {
                if let Some(ref error) = compression.error_message {
                    format!("Compression failed: {}", error)
                } else {
                    "Compression failed with an unknown error.".to_string()
                }
            }
            CompressionStatus::Noop => {
                "Nothing to compact - context is already minimal.".to_string()
            }
            CompressionStatus::Pending => {
                "Compacting chat history...".to_string()
            }
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
