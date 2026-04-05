// @amadeus-header
// summary: TUI component implementation for messages.
// layer: ui
// status: active
// feature_flags:
// - tui
// provides:
// - module: crate::ui::components::messages
// - type: crate::ui::components::messages::CompressionStatus
// - type: crate::ui::components::messages::CompressionItem
// - type: crate::ui::components::messages::HistoryItem
// - type: crate::ui::components::messages::MessagesComponent
// uses:
// - module: crate::ui::components::compaction_animation::CompactionAnimator
// - module: crate::ui::components::markdown::render_markdown
// - module: crate::ui::components::tool_group
// - module: crate::ui::get_colors
// - runtime: ratatui terminal rendering
// - runtime: tracing instrumentation
// invariants:
// - Listed interfaces stay aligned with the implementation in this file.
// side_effects: none
// tests:
// - tests/messages_test.rs
// @end-amadeus-header

use std::time::Instant;

use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};
use tracing::debug;

use crate::ui::components::compaction_animation::CompactionAnimator;
use crate::ui::components::markdown::render_markdown;
use crate::ui::components::tool_group::{render_tool_group_with_limit, ToolGroup};
use crate::ui::get_colors;

fn truncate_text(s: &str, max_len: usize) -> String {
    if s.chars().count() <= max_len {
        s.to_string()
    } else if max_len <= 3 {
        "...".to_string()
    } else {
        let keep = max_len - 3;
        let prefix: String = s.chars().take(keep).collect();
        format!("{}...", prefix)
    }
}

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
    User {
        content: String,
        timestamp: Instant,
        turn: usize,
    },
    Assistant {
        content: String,
        timestamp: Instant,
        turn: usize,
    },
    SubAgentPrompt {
        content: String,
        depth: usize,
        turn: usize,
    },
    /// Extended thinking/reasoning content from the model
    Thinking {
        content: String,
        timestamp: Instant,
        turn: usize,
        is_collapsed: bool,
    },
    ToolGroup {
        group: ToolGroup,
        turn: usize,
    },
    /// Compression/compaction operation (gemini-cli style)
    Compression {
        compression: CompressionItem,
    },
    /// Inline context usage report (like Claude Code /context)
    ContextReport {
        info: crate::ui::components::ContextInfo,
        turn: usize,
    },
}

impl HistoryItem {
    pub fn tool_group(group: ToolGroup, turn: usize) -> Self {
        Self::ToolGroup { group, turn }
    }

    pub fn subagent_prompt(content: String, depth: usize, turn: usize) -> Self {
        Self::SubAgentPrompt {
            content,
            depth,
            turn,
        }
    }

    pub fn compression(compression: CompressionItem) -> Self {
        Self::Compression { compression }
    }

    pub fn context_report(info: crate::ui::components::ContextInfo, turn: usize) -> Self {
        Self::ContextReport { info, turn }
    }
}

pub struct MessagesComponent {
    items: Vec<HistoryItem>,
    streaming_text: Option<String>,
    /// Streaming thinking content
    streaming_thinking: Option<String>,
    pending_tool_group: Option<ToolGroup>,
    /// Pending compression item with animated display
    pending_compression: Option<CompressionItem>,
    /// Beautiful animated compaction display
    compaction_animator: CompactionAnimator,
    /// Turn counter for conversation tracking
    turn_counter: usize,
    /// Current streaming turn (for assistant responses)
    current_turn: usize,
    /// Whether tool groups are expanded (showing all tools)
    tool_expansion_enabled: bool,
    last_rendered_index: usize,
    last_rendered_turn: Option<usize>,
    skip_next_assistant_history_item: bool,
    /// Skip the welcome/dashboard block on the next scrollback flush (after multi-session switch).
    suppress_dashboard_on_next_scrollback_flush: bool,
    /// Vertical scroll offset (number of lines scrolled from top)
    scroll_offset: usize,
}

impl MessagesComponent {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            streaming_text: None,
            streaming_thinking: None,
            pending_tool_group: None,
            pending_compression: None,
            compaction_animator: CompactionAnimator::new(),
            turn_counter: 0,
            current_turn: 0,
            tool_expansion_enabled: false,
            last_rendered_index: 0,
            last_rendered_turn: None,
            skip_next_assistant_history_item: false,
            suppress_dashboard_on_next_scrollback_flush: false,
            scroll_offset: 0,
        }
    }

    /// Reset scrollback rendering state after the host terminal's shared scrollback was cleared
    /// (e.g. switching agent sessions). The next `take_unrendered_lines` will re-emit committed
    /// history without duplicating streamed assistant lines.
    pub fn reset_scrollback_cursor_for_session_switch(&mut self) {
        self.last_rendered_index = 0;
        self.last_rendered_turn = None;
        self.skip_next_assistant_history_item = false;
        self.suppress_dashboard_on_next_scrollback_flush = !self.items.is_empty();
    }

    /// Get the current turn number
    fn next_turn(&mut self) -> usize {
        self.turn_counter += 1;
        self.turn_counter
    }

    pub fn current_turn(&self) -> usize {
        self.current_turn
    }

    pub fn take_unrendered_lines(&mut self, width: u16) -> Vec<Line<'static>> {
        let colors = get_colors();
        let mut lines = Vec::new();
        let mut last_turn = self.last_rendered_turn;
        let mut skipped_streamed_assistant = false;

        if self.last_rendered_index == 0
            && self.last_rendered_turn.is_none()
            && !self.items.is_empty()
            && !self.suppress_dashboard_on_next_scrollback_flush
        {
            let dashboard_lines = self.render_dashboard_lines(width);
            if !dashboard_lines.is_empty() {
                lines.extend(dashboard_lines);
                lines.push(Line::from(""));
            }
        }
        self.suppress_dashboard_on_next_scrollback_flush = false;

        for item in self.items[self.last_rendered_index..].iter() {
            let should_skip = !skipped_streamed_assistant
                && self.skip_next_assistant_history_item
                && matches!(item, HistoryItem::Assistant { .. });

            if should_skip {
                skipped_streamed_assistant = true;
                continue;
            }

            let item_turn = Self::item_turn(item);

            if let Some(turn) = item_turn {
                if last_turn != Some(turn) {
                    if last_turn.is_some() {
                        lines.push(Line::from(""));
                    }
                    lines.push(Self::render_turn_separator(turn, &colors));
                    lines.push(Line::from(""));
                    last_turn = Some(turn);
                }
            }

            lines.extend(Self::render_item(item, width));
        }

        self.last_rendered_index = self.items.len();
        self.last_rendered_turn = last_turn;
        if skipped_streamed_assistant {
            self.skip_next_assistant_history_item = false;
        }
        lines
    }

    pub fn mark_last_item_rendered(&mut self) {
        if self.last_rendered_index + 1 == self.items.len() {
            if let Some(item) = self.items.last() {
                self.last_rendered_turn = Self::item_turn(item);
            }
            self.last_rendered_index = self.items.len();
        }
    }

    pub fn should_prefix_current_turn(&self) -> bool {
        self.last_rendered_turn != Some(self.current_turn)
    }

    pub fn note_stream_chunk_rendered(&mut self) {
        self.last_rendered_turn = Some(self.current_turn);
        self.skip_next_assistant_history_item = true;
    }

    pub fn has_completed_pending_tool_group(&self) -> bool {
        self.pending_tool_group
            .as_ref()
            .map(|group| !group.is_empty() && !group.has_pending())
            .unwrap_or(false)
    }

    pub fn flush_completed_pending_tool_group(&mut self) -> bool {
        if !self.has_completed_pending_tool_group() {
            return false;
        }

        let turn = self.current_turn;
        self.finalize_pending_tool_group_with_turn(turn);
        true
    }

    pub fn add_user(&mut self, content: String) {
        self.finalize_pending_tool_group();
        let turn = self.next_turn();
        self.current_turn = turn;
        self.items.push(HistoryItem::User {
            content,
            timestamp: Instant::now(),
            turn,
        });
    }

    pub fn add_assistant(&mut self, content: String) {
        self.finalize_pending_tool_group();
        self.streaming_text = None;
        let turn = self.current_turn;
        self.items.push(HistoryItem::Assistant {
            content,
            timestamp: Instant::now(),
            turn,
        });
    }

    pub fn add_subagent_prompt(&mut self, content: String, depth: usize) {
        self.finalize_pending_tool_group();
        let turn = self.current_turn;
        self.items
            .push(HistoryItem::subagent_prompt(content, depth, turn));
    }

    pub fn add_context_report(&mut self, info: crate::ui::components::ContextInfo, turn: usize) {
        self.items.push(HistoryItem::context_report(info, turn));
    }

    pub fn update_streaming_text(&mut self, text: &str) {
        debug!(text_len = text.len(), "Updating streaming text");
        self.streaming_text = if text.is_empty() {
            None
        } else {
            Some(text.to_string())
        };
    }

    pub fn clear_streaming_text(&mut self) {
        self.streaming_text = None;
        self.streaming_thinking = None;
        self.pending_tool_group = None;
    }

    pub fn render_assistant_chunk(
        content: &str,
        turn: usize,
        width: u16,
        include_prefix: bool,
    ) -> Vec<Line<'static>> {
        let colors = get_colors();
        let content_lines = render_markdown(content, width.saturating_sub(4) as usize);
        let mut lines = Vec::new();

        for (i, content_line) in content_lines.into_iter().enumerate() {
            let mut spans = Vec::new();
            if i == 0 {
                if include_prefix {
                    spans.push(Span::styled(
                        format!("✦ [{}] ", turn),
                        Style::default().fg(colors.text.accent),
                    ));
                } else {
                    spans.push(Span::raw("   "));
                }
            } else {
                spans.push(Span::raw("   "));
            }
            spans.extend(content_line.spans.into_iter());
            lines.push(Line::from(spans));
        }

        lines
    }

    pub fn flush_streaming_chunk(&mut self) {
        // No-op - chunking removed, streaming text is set directly
    }

    pub fn finalize_assistant(&mut self, text: String) {
        debug!(text_len = text.len(), "Finalizing assistant response");
        self.flush_streaming_chunk();
        self.finalize_pending_tool_group();
        self.streaming_text = None;
        // Finalize any pending thinking
        self.finalize_thinking();
        let turn = self.current_turn;
        self.items.push(HistoryItem::Assistant {
            content: text,
            timestamp: Instant::now(),
            turn,
        });
    }

    /// Update streaming thinking content
    pub fn update_thinking(&mut self, thinking: &str) {
        if let Some(ref mut existing) = self.streaming_thinking {
            existing.push_str(thinking);
        } else {
            self.streaming_thinking = Some(thinking.to_string());
        }
    }

    /// Finalize the pending thinking block
    pub fn finalize_thinking(&mut self) {
        if let Some(thinking) = self.streaming_thinking.take() {
            if !thinking.is_empty() {
                let turn = self.current_turn;
                self.items.push(HistoryItem::Thinking {
                    content: thinking,
                    timestamp: Instant::now(),
                    turn,
                    is_collapsed: false,
                });
            }
        }
    }

    /// Start a pending compression operation (shows animated display)
    pub fn start_compression(&mut self) {
        self.pending_compression = Some(CompressionItem::pending());
        self.compaction_animator.start();
    }

    /// Complete the pending compression with results
    /// Shows completion result in the animation box before transitioning to history
    pub fn complete_compression(&mut self, original_tokens: usize, new_tokens: usize) {
        // Set animator to completed state (shows result in animation box)
        self.compaction_animator
            .complete(original_tokens, new_tokens);
        // Keep pending_compression for rendering, will be cleared after display duration
    }

    /// Complete compression with "not beneficial" result
    pub fn complete_compression_not_beneficial(&mut self, original_tokens: usize) {
        // Treat as a "no change" completion - show result briefly
        self.compaction_animator
            .complete(original_tokens, original_tokens);
    }

    /// Complete compression with error
    pub fn complete_compression_failed(&mut self, error: String) {
        // Set animator to failed state
        self.compaction_animator.fail(error.clone());
        // Keep pending_compression for rendering
    }

    /// Complete compression with nothing to compress
    pub fn complete_compression_noop(&mut self) {
        // Show a brief "nothing to do" completion
        self.compaction_animator.complete(0, 0);
    }

    /// Check if compression is in progress
    pub fn is_compression_pending(&self) -> bool {
        self.pending_compression.is_some()
    }

    pub fn render_pending_compaction_preview(
        &self,
        _max_width: usize,
    ) -> Option<Vec<Line<'static>>> {
        let compression = self.pending_compression.as_ref()?;
        let colors = get_colors();

        if compression.status == CompressionStatus::Pending && self.compaction_animator.is_active()
        {
            let pulse = self.compaction_animator.spinner_frame();
            let msg = self.compaction_animator.current_message();
            let pct = self.compaction_animator.progress();
            let elapsed = self.compaction_animator.elapsed_string();
            let line = format!("  {msg}{pulse}  ·  {pct}%  ·  {elapsed}");
            return Some(vec![Line::from(vec![Span::styled(
                line,
                Style::default().fg(colors.text.secondary),
            )])]);
        }

        let color = match compression.status {
            CompressionStatus::Pending => colors.status.warning,
            CompressionStatus::Compressed => colors.status.success,
            CompressionStatus::Failed => colors.status.error,
            CompressionStatus::Noop | CompressionStatus::NotBeneficial => colors.ui.comment,
        };

        Some(vec![Line::from(vec![
            Span::styled("✦ ", Style::default().fg(color)),
            Span::styled(
                Self::get_compression_text(compression),
                Style::default().fg(color),
            ),
        ])])
    }

    /// Update scrollbar colors (called on theme change)
    pub fn update_scrollbar_colors(&mut self) {
        // No-op for now - scrollbar colors are fetched dynamically in render
        // This method exists for API compatibility
    }

    /// Scroll up by the specified number of lines
    pub fn scroll_up(&mut self, lines: usize) {
        self.scroll_offset = self.scroll_offset.saturating_sub(lines);
    }

    /// Scroll down by the specified number of lines
    pub fn scroll_down(&mut self, lines: usize) {
        // Note: we don't track max lines here, so just increment
        // The render method will clamp appropriately
        self.scroll_offset = self.scroll_offset.saturating_add(lines);
    }

    /// Scroll up by one page
    pub fn scroll_page_up(&mut self) {
        // Page size will be determined at render time
        // For now, scroll by a reasonable default
        self.scroll_offset = self.scroll_offset.saturating_sub(20);
    }

    /// Scroll down by one page
    pub fn scroll_page_down(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_add(20);
    }

    /// Scroll to the top of the content
    pub fn scroll_to_top(&mut self) {
        self.scroll_offset = 0;
    }

    /// Scroll to the bottom of the content (follow new messages)
    pub fn scroll_to_bottom(&mut self) {
        // Set to a large value - render will clamp it
        self.scroll_offset = usize::MAX;
    }

    /// Scroll to a specific ratio (0.0 = top, 1.0 = bottom)
    pub fn scroll_to_ratio(&mut self, ratio: f32) {
        // This will be properly calculated at render time based on content height
        // For now, we just store a proportional offset
        // The ratio will be applied during render when we know the total lines
        self.scroll_offset = (ratio * 10000.0) as usize;
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
    }

    /// Finalize the pending tool group with a turn number
    fn finalize_pending_tool_group_with_turn(&mut self, turn: usize) {
        if let Some(group) = self.pending_tool_group.take() {
            if !group.is_empty() {
                self.items.push(HistoryItem::ToolGroup { group, turn });
            }
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
    }

    /// Update progress for a running tool.
    pub fn update_tool_progress(&mut self, tool_id: &str, message: String, percent: Option<u8>) {
        if let Some(ref mut group) = self.pending_tool_group {
            if let Some(tool) = group.tools.iter_mut().find(|t| t.id == tool_id) {
                tool.progress_message = Some(message);
                tool.progress_percent = percent;
            }
        }
    }

    fn finalize_pending_tool_group(&mut self) {
        let turn = self.current_turn;
        self.finalize_pending_tool_group_with_turn(turn);
    }

    pub fn collapse_all_tools(&mut self) {
        for item in &mut self.items {
            if let HistoryItem::ToolGroup { group, .. } = item {
                group.collapse_all();
            }
        }
        if let Some(ref mut group) = self.pending_tool_group {
            group.collapse_all();
        }
    }

    pub fn expand_all_tools(&mut self) {
        for item in &mut self.items {
            if let HistoryItem::ToolGroup { group, .. } = item {
                group.expand_all();
            }
        }
        if let Some(ref mut group) = self.pending_tool_group {
            group.expand_all();
        }
    }

    /// Toggle tool expansion state for all tool groups
    /// When expanded, all tools are shown; when collapsed, only threshold number shown
    pub fn toggle_tool_expansion(&mut self) {
        self.tool_expansion_enabled = !self.tool_expansion_enabled;

        // Apply to all tool groups in history
        for item in &mut self.items {
            if let HistoryItem::ToolGroup { group, .. } = item {
                group.is_expanded = self.tool_expansion_enabled;
                if self.tool_expansion_enabled {
                    group.expand_all();
                } else {
                    group.collapse_all();
                }
            }
        }

        // Also apply to pending tool group
        if let Some(ref mut group) = self.pending_tool_group {
            group.is_expanded = self.tool_expansion_enabled;
            if self.tool_expansion_enabled {
                group.expand_all();
            } else {
                group.collapse_all();
            }
        }
    }

    pub fn tick(&mut self) {
        if self.pending_compression.is_some() {
            self.compaction_animator.tick();

            // Check if we should transition completed result to history
            if self.compaction_animator.should_transition_to_history() {
                self.transition_compression_to_history();
            }
        }
    }

    /// Transition the completed compression result to history
    fn transition_compression_to_history(&mut self) {
        if let Some(result) = self.compaction_animator.result() {
            if let Some(ref error) = result.error_message {
                // Failed compaction
                let completed = CompressionItem::failed(error.clone());
                self.items.push(HistoryItem::compression(completed));
            } else if result.original_tokens > 0 {
                // Successful compaction
                let completed =
                    CompressionItem::completed(result.original_tokens, result.new_tokens);
                self.items.push(HistoryItem::compression(completed));
            }
        }

        self.pending_compression = None;
        self.compaction_animator.stop();
    }

    /// Render a turn separator line
    fn render_turn_separator(turn: usize, colors: &crate::ui::SemanticColors) -> Line<'static> {
        Line::from(vec![
            Span::styled("─".repeat(8), Style::default().fg(colors.ui.dark)),
            Span::styled(
                format!(" turn {} ", turn),
                Style::default().fg(colors.ui.comment),
            ),
            Span::styled("─".repeat(8), Style::default().fg(colors.ui.dark)),
        ])
    }

    pub fn render(&mut self, frame: &mut Frame, area: Rect) {
        if area.width < 3 || area.height < 1 {
            return;
        }

        let colors = get_colors();
        let mut lines: Vec<Line> = Vec::new();
        let content_width = area.width.saturating_sub(4) as usize;
        let mut last_turn: Option<usize> = None;

        let has_history = !self.items.is_empty()
            || self.streaming_text.is_some()
            || self.streaming_thinking.is_some()
            || self.pending_tool_group.is_some()
            || self.pending_compression.is_some();

        if !has_history {
            let dashboard_lines = self.render_dashboard_lines(area.width);
            if !dashboard_lines.is_empty() {
                frame.render_widget(Paragraph::new(dashboard_lines), area);
            }
            return;
        }

        for item in self.items[self.last_rendered_index..].iter() {
            let item_turn = Self::item_turn(item);

            if let Some(turn) = item_turn {
                if last_turn != Some(turn) {
                    if last_turn.is_some() {
                        lines.push(Line::from(""));
                    }
                    lines.push(Self::render_turn_separator(turn, &colors));
                    lines.push(Line::from(""));
                    last_turn = Some(turn);
                }
            }
            lines.extend(Self::render_item(item, content_width as u16));
        }

        // Render pending compression
        if self.pending_compression.is_some() {
            if let Some(preview_lines) = self.render_pending_compaction_preview(content_width) {
                lines.extend(preview_lines);
            }
            lines.push(Line::from(""));
        }

        // Render pending tool group
        if let Some(ref group) = self.pending_tool_group {
            let tool_lines = render_tool_group_with_limit(group, area, area.height as usize);
            for line in tool_lines {
                lines.push(line);
            }
            lines.push(Line::from(""));
        }

        // Render streaming thinking
        if let Some(ref thinking) = self.streaming_thinking {
            lines.push(Line::from(vec![
                Span::styled("┌─ ", Style::default().fg(colors.text.secondary)),
                Span::styled(
                    "thinking",
                    Style::default()
                        .fg(colors.text.secondary)
                        .add_modifier(Modifier::ITALIC),
                ),
                Span::styled(" ─", Style::default().fg(colors.text.secondary)),
                Span::styled("─".repeat(20), Style::default().fg(colors.ui.dark)),
            ]));

            for thinking_line in thinking.lines() {
                lines.push(Line::from(vec![
                    Span::styled("│ ", Style::default().fg(colors.ui.dark)),
                    Span::styled(
                        thinking_line.to_string(),
                        Style::default()
                            .fg(colors.text.secondary)
                            .add_modifier(Modifier::ITALIC),
                    ),
                ]));
            }

            lines.push(Line::from(vec![
                Span::styled("└", Style::default().fg(colors.text.secondary)),
                Span::styled("─".repeat(30), Style::default().fg(colors.ui.dark)),
            ]));
            lines.push(Line::from(""));
        }

        if let Some(ref streaming) = self.streaming_text {
            if last_turn != Some(self.current_turn) {
                if last_turn.is_some() {
                    lines.push(Line::from(""));
                }
                lines.push(Self::render_turn_separator(self.current_turn, &colors));
                lines.push(Line::from(""));
            }

            let content_lines = render_markdown(streaming, content_width);
            for (i, content_line) in content_lines.into_iter().enumerate() {
                let mut spans = Vec::new();
                if i == 0 {
                    spans.push(Span::styled(
                        format!("[{}] ", self.current_turn),
                        Style::default().fg(colors.text.accent),
                    ));
                } else {
                    spans.push(Span::raw("    "));
                }
                spans.extend(content_line.spans.into_iter());
                lines.push(Line::from(spans));
            }
            lines.push(Line::from(""));
        }

        let total_lines = lines.len();
        if total_lines == 0 {
            return;
        }

        // Just render the tail elements if exceeding view box
        let view_height = area.height as usize;
        let start_idx = total_lines.saturating_sub(view_height);

        let visible_lines: Vec<Line> = lines.into_iter().skip(start_idx).collect();
        frame.render_widget(Paragraph::new(visible_lines), area);
    }

    /// Renders a single history item statically.
    pub fn render_item(item: &HistoryItem, width: u16) -> Vec<Line<'static>> {
        let colors = get_colors();
        let content_width = width.saturating_sub(4) as usize;
        let mut lines = Vec::new();

        match item {
            HistoryItem::User { content, turn, .. } => {
                let content_lines = render_markdown(content, content_width);
                for (i, content_line) in content_lines.into_iter().enumerate() {
                    let mut spans = Vec::new();
                    if i == 0 {
                        spans.push(Span::styled(
                            format!("> [{}] ", turn),
                            Style::default()
                                .fg(colors.text.link)
                                .bg(colors.background.message),
                        ));
                    } else {
                        spans.push(Span::styled(
                            "   ",
                            Style::default().bg(colors.background.message),
                        ));
                    }
                    spans.extend(content_line.spans.into_iter().map(|span| {
                        Span::styled(
                            span.content.to_string(),
                            span.style
                                .patch(Style::default().bg(colors.background.message)),
                        )
                    }));
                    lines.push(Line::from(spans));
                }
                lines.push(Line::from(""));
            }

            HistoryItem::Assistant { content, turn, .. } => {
                let content_lines = render_markdown(content, content_width);
                for (i, content_line) in content_lines.into_iter().enumerate() {
                    let mut spans = Vec::new();
                    if i == 0 {
                        spans.push(Span::styled(
                            format!("✦ [{}] ", turn),
                            Style::default().fg(colors.text.accent),
                        ));
                    } else {
                        spans.push(Span::raw("   "));
                    }
                    spans.extend(content_line.spans.into_iter());
                    lines.push(Line::from(spans));
                }
                lines.push(Line::from(""));
            }

            HistoryItem::SubAgentPrompt {
                content,
                depth,
                turn,
            } => {
                let border_color = colors.status.warning;
                let body_color = colors.text.secondary;
                let inner_width = content_width.max(12);
                let title =
                    truncate_text(&format!(" sub-agent d{} [{}] ", depth, turn), inner_width);
                let top_fill = inner_width.saturating_sub(title.chars().count()).max(1);
                let top = format!("┌{}{}┐", title, "─".repeat(top_fill));
                let bottom = format!("└{}┘", "─".repeat(inner_width.max(2)));

                lines.push(Line::from(vec![Span::styled(
                    top,
                    Style::default().fg(border_color),
                )]));

                let wrapped = render_markdown(content, inner_width.saturating_sub(2));
                for line in wrapped {
                    let mut spans = vec![Span::styled("│ ", Style::default().fg(border_color))];
                    if line.spans.is_empty() {
                        spans.push(Span::styled("(empty)", Style::default().fg(body_color)));
                    } else {
                        spans.extend(line.spans.into_iter().map(|span| {
                            Span::styled(span.content.to_string(), Style::default().fg(body_color))
                        }));
                    }
                    lines.push(Line::from(spans));
                }

                lines.push(Line::from(vec![Span::styled(
                    bottom,
                    Style::default().fg(border_color),
                )]));
                lines.push(Line::from(""));
            }

            HistoryItem::ToolGroup { group, .. } => {
                let dummy_area = Rect::new(0, 0, width, 1000);
                let tool_lines = render_tool_group_with_limit(group, dummy_area, 1000);
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

            HistoryItem::ContextReport { info, turn } => {
                lines.extend(Self::render_context_report(info, *turn, content_width));
            }

            HistoryItem::Thinking {
                content,
                is_collapsed,
                ..
            } => {
                let collapse_icon = if *is_collapsed { "+" } else { "-" };
                lines.push(Line::from(vec![
                    Span::styled("┌─ ", Style::default().fg(colors.text.secondary)),
                    Span::styled("[", Style::default().fg(colors.ui.dark)),
                    Span::styled(collapse_icon, Style::default().fg(colors.text.accent)),
                    Span::styled("] ", Style::default().fg(colors.ui.dark)),
                    Span::styled(
                        "thinking",
                        Style::default()
                            .fg(colors.text.secondary)
                            .add_modifier(Modifier::ITALIC),
                    ),
                    Span::styled(" ─", Style::default().fg(colors.text.secondary)),
                    Span::styled("─".repeat(20), Style::default().fg(colors.ui.dark)),
                ]));

                if !is_collapsed {
                    for thinking_line in content.lines() {
                        lines.push(Line::from(vec![
                            Span::styled("│ ", Style::default().fg(colors.ui.dark)),
                            Span::styled(
                                thinking_line.to_string(),
                                Style::default()
                                    .fg(colors.text.secondary)
                                    .add_modifier(Modifier::ITALIC),
                            ),
                        ]));
                    }
                }

                lines.push(Line::from(vec![
                    Span::styled("└", Style::default().fg(colors.text.secondary)),
                    Span::styled("─".repeat(30), Style::default().fg(colors.ui.dark)),
                ]));
                lines.push(Line::from(""));
            }
        }
        lines
    }

    fn item_turn(item: &HistoryItem) -> Option<usize> {
        match item {
            HistoryItem::User { turn, .. } => Some(*turn),
            HistoryItem::Assistant { turn, .. } => Some(*turn),
            HistoryItem::SubAgentPrompt { turn, .. } => Some(*turn),
            HistoryItem::Thinking { turn, .. } => Some(*turn),
            HistoryItem::ToolGroup { turn, .. } => Some(*turn),
            HistoryItem::Compression { .. } => None,
            HistoryItem::ContextReport { turn, .. } => Some(*turn),
        }
    }

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
            CompressionStatus::Pending => "Compacting chat history...".to_string(),
        }
    }

    fn render_context_report(
        info: &crate::ui::components::ContextInfo,
        turn: usize,
        _content_width: usize,
    ) -> Vec<Line<'static>> {
        let colors = get_colors();
        let mut lines = Vec::new();

        let total = info.context_window_size as usize;
        let pct = info.usage_percent();

        // Autocompact buffer (fixed reservation for compaction headroom)
        let compaction_threshold: f64 = 75.0; // matches CompactionConfig default
        let max_buffer: f64 = 100.0 - compaction_threshold; // 25% max
        let used_f: f64 = pct as f64;
        let remaining: f64 = (100.0 - used_f).max(0.0);
        let buffer_pct: f64 = max_buffer.min(remaining);
        let free_pct: f64 = remaining - buffer_pct;
        let buffer_tokens = (total as f64 * buffer_pct / 100.0) as usize;
        let free_tokens = (total as f64 * free_pct / 100.0) as usize;

        // ── Header line (matching turn indicator style) ──
        lines.push(Line::from(vec![Span::styled(
            format!("> [{}] /context", turn),
            Style::default()
                .fg(colors.text.link)
                .bg(colors.background.message),
        )]));

        // ── Block bar (10 chars, each = 10%) ──
        let bar_width = 10usize;
        let used_chars = ((pct as f64 / 100.0) * bar_width as f64).round() as usize;
        let buffer_chars = ((buffer_pct / 100.0) * bar_width as f64).round() as usize;
        let free_chars = bar_width
            .saturating_sub(used_chars)
            .saturating_sub(buffer_chars);

        // ── Block bar (10 chars, each = 10%) ──
        // Build segments with individual colors
        let fill_color = ratatui::style::Color::Rgb(133, 153, 0); // Green
        let buffer_color = ratatui::style::Color::Rgb(120, 120, 120); // Gray
        let empty_color = colors.ui.dark;

        let mut bar_spans = vec![Span::styled("⛀ ", Style::default().fg(colors.text.accent))];

        if used_chars > 0 {
            let filled = (0..used_chars).map(|_| "⛁").collect::<Vec<_>>().join(" ");
            bar_spans.push(Span::styled(
                format!("{} ", filled),
                Style::default().fg(fill_color),
            ));
        }

        if buffer_chars > 0 {
            let buffered = (0..buffer_chars).map(|_| "⛝").collect::<Vec<_>>().join(" ");
            bar_spans.push(Span::styled(
                format!("{} ", buffered),
                Style::default().fg(buffer_color),
            ));
        }

        if free_chars > 0 {
            let empty = (0..free_chars).map(|_| "⛶").collect::<Vec<_>>().join(" ");
            bar_spans.push(Span::styled(
                format!("{} ", empty),
                Style::default().fg(empty_color),
            ));
        }

        bar_spans.push(Span::styled(
            format!("  {}", info.model_name),
            Style::default().fg(colors.text.primary),
        ));
        lines.push(Line::from(bar_spans));

        // Token count on next line
        lines.push(Line::from(vec![Span::styled(
            format!(
                "    {} / {} tokens ({}%)",
                crate::ui::components::ContextInfo::fmt_tokens(info.used_tokens()),
                crate::ui::components::ContextInfo::fmt_tokens(total),
                pct,
            ),
            Style::default().fg(colors.text.secondary),
        )]));

        lines.push(Line::from(""));

        // ── Category breakdown ──
        lines.push(Line::from(vec![Span::styled(
            "  Estimated usage by category",
            Style::default().fg(colors.ui.comment),
        )]));

        // System prompt
        let sys_pct = info.pct_of(info.system_prompt_tokens);
        lines.push(Line::from(vec![
            Span::raw("  ⛁ "),
            Span::styled("System prompt", Style::default().fg(colors.text.primary)),
            Span::styled(
                format!(
                    ": {} tokens ({:.1}%)",
                    crate::ui::components::ContextInfo::fmt_tokens(info.system_prompt_tokens),
                    sys_pct,
                ),
                Style::default().fg(colors.text.secondary),
            ),
        ]));

        // System tools
        let tools_pct = info.pct_of(info.tools_tokens);
        lines.push(Line::from(vec![
            Span::raw("  ⛁ "),
            Span::styled("System tools", Style::default().fg(colors.text.primary)),
            Span::styled(
                format!(
                    ": {} tokens ({:.1}%)",
                    crate::ui::components::ContextInfo::fmt_tokens(info.tools_tokens),
                    tools_pct,
                ),
                Style::default().fg(colors.text.secondary),
            ),
        ]));

        // MCP tools
        let mcp_pct = info.pct_of(info.mcp_tools_tokens);
        lines.push(Line::from(vec![
            Span::raw("  ⛁ "),
            Span::styled("MCP tools", Style::default().fg(colors.text.primary)),
            Span::styled(
                format!(
                    ": {} tokens ({:.1}%)",
                    crate::ui::components::ContextInfo::fmt_tokens(info.mcp_tools_tokens),
                    mcp_pct,
                ),
                Style::default().fg(colors.text.secondary),
            ),
        ]));

        // Memory files
        let mem_pct = info.pct_of(info.memory_files_tokens);
        lines.push(Line::from(vec![
            Span::raw("  ⛁ "),
            Span::styled("Memory files", Style::default().fg(colors.text.primary)),
            Span::styled(
                format!(
                    ": {} tokens ({:.1}%)",
                    crate::ui::components::ContextInfo::fmt_tokens(info.memory_files_tokens),
                    mem_pct,
                ),
                Style::default().fg(colors.text.secondary),
            ),
        ]));

        // Skills
        let skills_pct = info.pct_of(info.skills_tokens);
        lines.push(Line::from(vec![
            Span::raw("  ⛁ "),
            Span::styled("Skills", Style::default().fg(colors.text.primary)),
            Span::styled(
                format!(
                    ": {} tokens ({:.1}%)",
                    crate::ui::components::ContextInfo::fmt_tokens(info.skills_tokens),
                    skills_pct,
                ),
                Style::default().fg(colors.text.secondary),
            ),
        ]));

        // Messages
        let conv_pct = info.pct_of(info.conversation_tokens);
        lines.push(Line::from(vec![
            Span::raw("  ⛁ "),
            Span::styled("Messages", Style::default().fg(colors.text.primary)),
            Span::styled(
                format!(
                    ": {} tokens ({:.1}%)",
                    crate::ui::components::ContextInfo::fmt_tokens(info.conversation_tokens),
                    conv_pct,
                ),
                Style::default().fg(colors.text.secondary),
            ),
        ]));

        // Free space
        lines.push(Line::from(vec![
            Span::raw("  ⛶ "),
            Span::styled("Free space", Style::default().fg(colors.text.primary)),
            Span::styled(
                format!(
                    ": {} ({:.1}%)",
                    crate::ui::components::ContextInfo::fmt_tokens(free_tokens),
                    free_pct,
                ),
                Style::default().fg(colors.text.secondary),
            ),
        ]));

        // Autocompact buffer
        lines.push(Line::from(vec![
            Span::raw("  ⛝ "),
            Span::styled(
                "Autocompact buffer",
                Style::default().fg(colors.text.primary),
            ),
            Span::styled(
                format!(
                    ": {} tokens ({:.1}%)",
                    crate::ui::components::ContextInfo::fmt_tokens(buffer_tokens),
                    buffer_pct,
                ),
                Style::default().fg(colors.text.secondary),
            ),
        ]));

        lines.push(Line::from(""));

        // ── MCP tools section ──
        lines.push(Line::from(vec![
            Span::styled("❯ ", Style::default().fg(colors.text.accent)),
            Span::styled("MCP tools", Style::default().fg(colors.text.primary)),
            Span::styled(" · /mcp", Style::default().fg(colors.ui.comment)),
        ]));
        if info.mcp_tool_details.is_empty() {
            lines.push(Line::from(vec![
                Span::raw("  └ "),
                Span::styled("(none)", Style::default().fg(colors.ui.comment)),
            ]));
        } else {
            for (name, tokens) in &info.mcp_tool_details {
                lines.push(Line::from(vec![
                    Span::raw("  └ "),
                    Span::styled(name.clone(), Style::default().fg(colors.text.primary)),
                    Span::styled(
                        format!(
                            ": {} tokens",
                            crate::ui::components::ContextInfo::fmt_tokens(*tokens)
                        ),
                        Style::default().fg(colors.text.secondary),
                    ),
                ]));
            }
        }

        lines.push(Line::from(""));

        // ── Memory files section ──
        lines.push(Line::from(vec![
            Span::styled("❯ ", Style::default().fg(colors.text.accent)),
            Span::styled("Memory files", Style::default().fg(colors.text.primary)),
            Span::styled(" · /memory", Style::default().fg(colors.ui.comment)),
        ]));
        if info.memory_file_details.is_empty() {
            lines.push(Line::from(vec![
                Span::raw("  └ "),
                Span::styled("(none)", Style::default().fg(colors.ui.comment)),
            ]));
        } else {
            for (name, tokens) in &info.memory_file_details {
                lines.push(Line::from(vec![
                    Span::raw("  └ "),
                    Span::styled(name.clone(), Style::default().fg(colors.text.primary)),
                    Span::styled(
                        format!(
                            ": {} tokens",
                            crate::ui::components::ContextInfo::fmt_tokens(*tokens)
                        ),
                        Style::default().fg(colors.text.secondary),
                    ),
                ]));
            }
        }

        lines.push(Line::from(""));

        // ── Skills section ──
        lines.push(Line::from(vec![
            Span::styled("❯ ", Style::default().fg(colors.text.accent)),
            Span::styled("Skills", Style::default().fg(colors.text.primary)),
            Span::styled(" · /skills", Style::default().fg(colors.ui.comment)),
        ]));
        if info.skill_details.is_empty() {
            lines.push(Line::from(vec![
                Span::raw("  └ "),
                Span::styled("(none)", Style::default().fg(colors.ui.comment)),
            ]));
        } else {
            for (name, tokens) in &info.skill_details {
                lines.push(Line::from(vec![
                    Span::raw("  └ "),
                    Span::styled(name.clone(), Style::default().fg(colors.text.primary)),
                    Span::styled(
                        format!(
                            ": {} tokens",
                            crate::ui::components::ContextInfo::fmt_tokens(*tokens)
                        ),
                        Style::default().fg(colors.text.secondary),
                    ),
                ]));
            }
        }

        lines.push(Line::from(""));

        // ── User section (from .amadeus/skills/) ──
        lines.push(Line::from(vec![
            Span::styled("❯ ", Style::default().fg(colors.text.accent)),
            Span::styled("User", Style::default().fg(colors.text.primary)),
        ]));
        if info.tool_details.is_empty() {
            lines.push(Line::from(vec![
                Span::raw("  └ "),
                Span::styled("(none)", Style::default().fg(colors.ui.comment)),
            ]));
        } else {
            for (name, tokens) in &info.tool_details {
                lines.push(Line::from(vec![
                    Span::raw("  └ "),
                    Span::styled(name.clone(), Style::default().fg(colors.text.primary)),
                    Span::styled(
                        format!(
                            ": {} tokens",
                            crate::ui::components::ContextInfo::fmt_tokens(*tokens)
                        ),
                        Style::default().fg(colors.text.secondary),
                    ),
                ]));
            }
        }

        lines
    }

    fn get_mascot(&self, colors: &crate::ui::SemanticColors, width: u16) -> Vec<Line<'static>> {
        let accent = colors.text.accent;
        let style = Style::default().fg(accent);
        let width = width as usize;

        const FACE_ART: [&str; 8] = [
            "⠀⠀⠀⠀⣀⣤⣴⠶⠶⣦⣤⣀⠀⠀⠀⠀",
            "⠀⠀⣠⡾⠋⢁⣶⣿⣿⣶⡈⠙⢷⣄⠀⠀",
            "⠀⣼⢏⣠⣤⣘⣿⣿⣿⡿⣃⣤⣄⡹⣧⠀",
            "⢰⡏⣿⣿⣿⣿⡆⢹⡏⢰⣿⣿⣿⣿⢻⡆",
            "⠸⣇⠙⠿⠿⢿⣧⣸⣇⣼⡿⠿⠿⠋⣼⠇",
            "⠀⢻⣄⠀⠀⠀⠙⢿⡿⠋⠀⠀⠀⣠⡟⠀",
            "⠀⠀⠙⢦⣄⠀⠀⢸⡇⠀⠀⣠⡴⠋⠀⠀",
            "⠀⠀⠀⠀⠉⠛⠳⠶⠶⠞⠛⠉⠀⠀⠀⠀",
        ];

        const FULL_ART: [&str; 12] = [
            "                          ⠀⠀⠀⠀⣀⣤⣴⠶⠶⣦⣤⣀⠀⠀⠀⠀",
            "                          ⠀⠀⣠⡾⠋⢁⣶⣿⣿⣶⡈⠙⢷⣄⠀⠀",
            "                          ⠀⣼⢏⣠⣤⣘⣿⣿⣿⡿⣃⣤⣄⡹⣧⠀",
            "                          ⢰⡏⣿⣿⣿⣿⡆⢹⡏⢰⣿⣿⣿⣿⢻⡆",
            "                          ⠸⣇⠙⠿⠿⢿⣧⣸⣇⣼⡿⠿⠿⠋⣼⠇",
            "                          ⠀⢻⣄⠀⠀⠀⠙⢿⡿⠋⠀⠀⠀⣠⡟⠀",
            "                          ⠀⠀⠙⢦⣄⠀⠀⢸⡇⠀⠀⣠⡴⠋⠀⠀",
            "                          ⠀⠀⠀⠀⠉⠛⠳⠶⠶⠞⠛⠉⠀⠀⠀⠀",
            "⠀⢀⣴⣾⣿⣿⣷⣤⡀⢸⣿⣿⣿⣿⣿⣿⣿⣦⠀⢀⣤⣶⣿⣿⣷⣦⣀⠀⠀⠀⠀⢸⣿⣿⣷⡀⠀⠀⣠⣶⣾⣿⣿⣦⣄⠀⢸⣿⣇⣠⣶⣿⡿⠛⣠⣶⣿⣿⣿⣦⣄⠀",
            "⢠⣿⡿⠉⠀⠈⠙⣿⣿⢸⣿⣿⠀⠀⠀⠈⠿⠿⠀⣾⣿⠋⠀⠀⠙⢻⣿⡇⠙⠻⣿⣷⣦⣀⠀⠀⠀⣸⣿⡏⠁⠀⠉⢻⣿⡇⢸⣿⣿⡿⠛⠁⠀⣼⣿⠏⠁⠀⠉⢻⣿⡇",
            "⠘⣿⣷⡀⠀⠀⠀⣿⣯⢨⣽⣿⣷⣦⣀⠀⠀⠀⠀⢿⣿⣄⠀⠀⠀⢸⣿⡅⣶⣶⡀⠙⠻⣿⣷⣤⡀⢻⣿⣇⡀⠀⠀⢸⣿⡇⢨⣿⣿⣷⣤⡀⠀⢻⣿⣆⡀⠀⠀⢸⣿⡅",
            "⠀⠈⠻⢿⣿⣿⠧⣿⣿⠘⠛⠛⠙⠻⣿⣷⣦⡀⠀⠈⠻⠿⣿⣿⡷⢸⣿⡇⠹⣿⣿⣿⣿⣿⣿⣿⡇⠀⠙⠿⣿⣿⡿⢼⣿⡇⠘⠛⠋⠙⠿⣿⣷⣤⠙⠿⣿⣿⡿⢼⣿⡇",
        ];

        let face_width = FACE_ART
            .iter()
            .map(|line| line.chars().count())
            .max()
            .unwrap_or(0);
        let full_width = FULL_ART
            .iter()
            .map(|line| line.chars().count())
            .max()
            .unwrap_or(0);

        let art: &[&str] = if width >= full_width {
            &FULL_ART
        } else if width >= face_width {
            &FACE_ART
        } else {
            return vec![];
        };

        let art_width = art
            .iter()
            .map(|line| line.chars().count())
            .max()
            .unwrap_or(0);
        let padding = " ".repeat(width.saturating_sub(art_width) / 2);

        art.iter()
            .map(|line| Line::from(Span::styled(format!("{}{}", padding, line), style)))
            .collect()
    }

    pub fn render_dashboard_lines(&self, width: u16) -> Vec<Line<'static>> {
        let colors = get_colors();
        let mut lines = Vec::new();
        let width = width as usize;

        if width < 20 {
            return lines;
        }

        let accent = colors.text.accent;
        let dark = colors.ui.dark;
        let secondary = colors.text.secondary;
        let comment = colors.ui.comment;
        let link = colors.text.link;

        // Header Title
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled(" Amadeus v0.1.0 ", Style::default().fg(accent)),
            Span::styled(
                "─".repeat(width.saturating_sub(18)),
                Style::default().fg(dark),
            ),
        ]));
        lines.push(Line::from(""));

        // Mascot (already centered in get_mascot)
        for line in self.get_mascot(&colors, width as u16) {
            lines.push(line);
        }
        lines.push(Line::from(""));

        // Info
        let cwd = std::env::current_dir().unwrap_or_default();
        let cwd_str = cwd.to_string_lossy();
        let project_name = cwd_str.split('/').next_back().unwrap_or("");

        lines.push(
            Line::from(vec![
                Span::styled(" amadeus ", Style::default().fg(secondary)),
                Span::styled("◈", Style::default().fg(comment)),
                Span::styled(
                    " Premium CLI Coding Interface",
                    Style::default().fg(secondary),
                ),
            ])
            .alignment(ratatui::layout::Alignment::Center),
        );

        lines.push(
            Line::from(vec![
                Span::styled(" path ", Style::default().fg(comment)),
                Span::styled(format!("~{}", project_name), Style::default().fg(link)),
            ])
            .alignment(ratatui::layout::Alignment::Center),
        );
        lines.push(Line::from(""));

        lines
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

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{backend::TestBackend, Terminal};

    #[test]
    fn test_messages_new() {
        let messages = MessagesComponent::new();
        assert!(messages.is_empty());
        assert_eq!(messages.len(), 0);
        assert_eq!(messages.turn_counter, 0);
    }

    #[test]
    fn test_add_user_message() {
        let mut messages = MessagesComponent::new();
        messages.add_user("Hello".to_string());

        assert_eq!(messages.len(), 1);
        assert_eq!(messages.turn_counter, 1);
    }

    #[test]
    fn test_add_assistant_message() {
        let mut messages = MessagesComponent::new();
        messages.add_user("Hello".to_string());
        messages.add_assistant("Hi there!".to_string());

        assert_eq!(messages.len(), 2);
        // Turn counter should still be 1 (same turn)
        assert_eq!(messages.turn_counter, 1);
    }

    #[test]
    fn test_turn_counter_increments() {
        let mut messages = MessagesComponent::new();

        // First turn
        messages.add_user("Q1".to_string());
        assert_eq!(messages.turn_counter, 1);

        messages.add_assistant("A1".to_string());

        // Second turn
        messages.add_user("Q2".to_string());
        assert_eq!(messages.turn_counter, 2);

        messages.add_assistant("A2".to_string());
    }

    #[test]
    fn test_update_streaming_text() {
        let mut messages = MessagesComponent::new();
        messages.add_user("Hello".to_string());

        messages.update_streaming_text("Partial response");
        assert!(messages.streaming_text.is_some());

        messages.finalize_assistant("Full response".to_string());
        assert!(messages.streaming_text.is_none());
    }

    #[test]
    fn test_thinking_delta() {
        let mut messages = MessagesComponent::new();
        messages.add_user("Hello".to_string());

        messages.update_thinking("Thinking about this...");
        assert!(messages.streaming_thinking.is_some());

        messages.update_thinking(" Still thinking...");
        let thinking = messages.streaming_thinking.as_ref().unwrap();
        assert!(thinking.contains("Thinking"));
        assert!(thinking.contains("Still"));
    }

    #[test]
    fn test_finalize_thinking() {
        let mut messages = MessagesComponent::new();
        messages.add_user("Hello".to_string());

        messages.update_thinking("My reasoning process");
        messages.finalize_thinking();

        assert!(messages.streaming_thinking.is_none());
        assert_eq!(messages.len(), 2); // User + Thinking
    }

    #[test]
    fn test_compression_item_pending() {
        let compression = CompressionItem::pending();
        assert!(compression.is_pending);
        assert_eq!(compression.status, CompressionStatus::Pending);
    }

    #[test]
    fn test_compression_item_completed() {
        let compression = CompressionItem::completed(1000, 500);
        assert!(!compression.is_pending);
        assert_eq!(compression.status, CompressionStatus::Compressed);
        assert_eq!(compression.original_token_count, Some(1000));
        assert_eq!(compression.new_token_count, Some(500));
    }

    #[test]
    fn test_compression_item_not_beneficial() {
        let compression = CompressionItem::not_beneficial(100);
        assert!(!compression.is_pending);
        assert_eq!(compression.status, CompressionStatus::NotBeneficial);
    }

    #[test]
    fn test_compression_item_failed() {
        let compression = CompressionItem::failed("Test error".to_string());
        assert!(!compression.is_pending);
        assert_eq!(compression.status, CompressionStatus::Failed);
        assert_eq!(compression.error_message, Some("Test error".to_string()));
    }

    #[test]
    fn test_compression_item_noop() {
        let compression = CompressionItem::noop();
        assert!(!compression.is_pending);
        assert_eq!(compression.status, CompressionStatus::Noop);
    }

    #[test]
    fn test_start_compression() {
        let mut messages = MessagesComponent::new();
        messages.start_compression();

        assert!(messages.pending_compression.is_some());
        assert!(messages.is_compression_pending());
    }

    #[test]
    fn test_complete_compression() {
        let mut messages = MessagesComponent::new();
        messages.start_compression();
        messages.complete_compression(1000, 500);

        // After completion, pending_compression is still set (showing result)
        // until the transition timeout (1.5 seconds in real time)
        assert!(messages.pending_compression.is_some());
        assert!(messages.compaction_animator.is_showing_result());

        // Verify the result is stored correctly
        let result = messages.compaction_animator.result().unwrap();
        assert_eq!(result.original_tokens, 1000);
        assert_eq!(result.new_tokens, 500);
    }

    #[test]
    fn test_pending_compression_renders_animation() {
        let backend = TestBackend::new(80, 8);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        let mut messages = MessagesComponent::new();
        messages.start_compression();
        messages.tick();

        terminal
            .draw(|frame| messages.render(frame, frame.area()))
            .expect("render should succeed");

        let rendered = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();

        assert!(rendered.contains("Compacting context"));
        assert!(rendered.contains("%"));
    }

    #[test]
    fn test_get_compression_text_compressed() {
        let compression = CompressionItem::completed(1000, 500);
        let text = MessagesComponent::get_compression_text(&compression);

        assert!(text.contains("1000"));
        assert!(text.contains("500"));
        assert!(text.contains("50%")); // 500/1000 = 50% saved
    }

    #[test]
    fn test_get_compression_text_failed() {
        let compression = CompressionItem::failed("Network error".to_string());
        let text = MessagesComponent::get_compression_text(&compression);

        assert!(text.contains("failed"));
        assert!(text.contains("Network error"));
    }

    #[test]
    fn test_tool_tracking() {
        let mut messages = MessagesComponent::new();
        messages.add_user("Test".to_string());

        messages.start_tool("tool_1".to_string(), "bash".to_string(), None);
        assert!(messages.pending_tool_group.is_some());

        messages.complete_tool("tool_1", "output".to_string(), false, None);
    }

    #[test]
    fn test_collapse_expand_tools() {
        let mut messages = MessagesComponent::new();
        messages.add_user("Test".to_string());
        messages.start_tool("t1".to_string(), "bash".to_string(), None);
        messages.complete_tool("t1", "output".to_string(), false, None);
        messages.finalize_pending_tool_group();

        // These should not panic
        messages.collapse_all_tools();
        messages.expand_all_tools();
    }

    #[test]
    fn test_take_unrendered_lines_skips_streamed_assistant_after_tool_group() {
        let mut messages = MessagesComponent::new();
        messages.add_user("Test".to_string());
        messages.start_tool("t1".to_string(), "todo".to_string(), None);
        messages.complete_tool("t1", "[x] #1: Hello world".to_string(), false, None);

        messages.note_stream_chunk_rendered();
        messages.finalize_assistant("Tool handled".to_string());

        let tool_lines = messages.take_unrendered_lines(100);
        assert!(!tool_lines.is_empty());
        let rendered = tool_lines
            .iter()
            .map(|line| line.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(rendered.contains("todo"));
        assert!(!rendered.contains("Tool handled"));

        let assistant_lines = messages.take_unrendered_lines(100);
        assert!(assistant_lines.is_empty());
    }

    #[test]
    fn test_take_unrendered_lines_prepends_dashboard_before_first_turn() {
        let mut messages = MessagesComponent::new();
        messages.add_user("Hello".to_string());

        let lines = messages.take_unrendered_lines(100);
        let rendered = lines
            .iter()
            .map(|line| line.to_string())
            .collect::<Vec<_>>()
            .join("\n");

        assert!(rendered.contains("Amadeus v0.1.0"));
        assert!(!rendered.contains("Tips for getting started"));
        assert!(!rendered.contains("/help"));
        assert!(rendered.contains("turn 1"));
        assert!(rendered.contains("Hello"));
    }

    #[test]
    fn test_flush_completed_pending_tool_group_moves_tool_group_into_history() {
        let mut messages = MessagesComponent::new();
        messages.add_user("Test".to_string());
        messages.start_tool("t1".to_string(), "todo".to_string(), None);

        assert!(!messages.has_completed_pending_tool_group());

        messages.complete_tool("t1", "[x] #1: Hello world".to_string(), false, None);

        assert!(messages.has_completed_pending_tool_group());
        assert!(messages.flush_completed_pending_tool_group());
        assert!(messages.pending_tool_group.is_none());
        assert!(matches!(
            messages.items.last(),
            Some(HistoryItem::ToolGroup { .. })
        ));
    }

    #[test]
    fn test_history_item_turn_tracking() {
        let mut messages = MessagesComponent::new();

        messages.add_user("Q1".to_string());
        messages.add_assistant("A1".to_string());
        messages.add_user("Q2".to_string());
        messages.add_assistant("A2".to_string());

        // Check turn numbers
        match &messages.items[0] {
            HistoryItem::User { turn, .. } => assert_eq!(*turn, 1),
            _ => panic!("Expected user message"),
        }

        match &messages.items[1] {
            HistoryItem::Assistant { turn, .. } => assert_eq!(*turn, 1),
            _ => panic!("Expected assistant message"),
        }

        match &messages.items[2] {
            HistoryItem::User { turn, .. } => assert_eq!(*turn, 2),
            _ => panic!("Expected user message"),
        }
    }

    #[test]
    fn test_messages_default() {
        let messages = MessagesComponent::default();
        assert!(messages.is_empty());
    }
}
