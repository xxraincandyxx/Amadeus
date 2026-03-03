use std::time::Instant;

use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState},
    Frame,
};

use crate::ui::components::compaction_animation::CompactionAnimator;
use crate::ui::components::markdown::render_markdown;
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
    User { content: String, timestamp: Instant, turn: usize },
    Assistant { content: String, timestamp: Instant, turn: usize },
    /// Extended thinking/reasoning content from the model
    Thinking { content: String, timestamp: Instant, turn: usize, is_collapsed: bool },
    ToolGroup { group: ToolGroup, turn: usize },
    /// Compression/compaction operation (gemini-cli style)
    Compression { compression: CompressionItem },
}

impl HistoryItem {
    pub fn tool_group(group: ToolGroup, turn: usize) -> Self {
        Self::ToolGroup { group, turn }
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
}

impl MessagesComponent {
    pub fn new() -> Self {
        let colors = get_colors();
        Self {
            items: Vec::new(),
            scroll_state: ScrollState::new(),
            scrollbar: AnimatedScrollbar::new(colors.scrollbar.thumb, colors.scrollbar.thumb_hover),
            streaming_text: None,
            streaming_thinking: None,
            pending_tool_group: None,
            pending_compression: None,
            compaction_animator: CompactionAnimator::new(),
            turn_counter: 0,
            current_turn: 0,
            tool_expansion_enabled: false,
        }
    }

    /// Get the current turn number
    fn next_turn(&mut self) -> usize {
        self.turn_counter += 1;
        self.turn_counter
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
        if self.scroll_state.auto_scroll {
            self.scroll_state.scroll_to_bottom();
            self.scrollbar.flash();
        }
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
        // Finalize any pending thinking
        self.finalize_thinking();
        let turn = self.current_turn;
        self.items.push(HistoryItem::Assistant {
            content: text,
            timestamp: Instant::now(),
            turn,
        });
        if self.scroll_state.auto_scroll {
            self.scroll_state.scroll_to_bottom();
            self.scrollbar.flash();
        }
    }

    /// Update streaming thinking content
    pub fn update_thinking(&mut self, thinking: &str) {
        if let Some(ref mut existing) = self.streaming_thinking {
            existing.push_str(thinking);
        } else {
            self.streaming_thinking = Some(thinking.to_string());
        }
        if self.scroll_state.auto_scroll {
            self.scroll_state.scroll_to_bottom();
            self.scrollbar.flash();
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
                if self.scroll_state.auto_scroll {
                    self.scroll_state.scroll_to_bottom();
                    self.scrollbar.flash();
                }
            }
        }
    }

    /// Start a pending compression operation (shows animated display)
    pub fn start_compression(&mut self) {
        self.pending_compression = Some(CompressionItem::pending());
        self.compaction_animator.start();
        if self.scroll_state.auto_scroll {
            self.scroll_state.scroll_to_bottom();
            self.scrollbar.flash();
        }
    }

    /// Complete the pending compression with results
    /// Shows completion result in the animation box before transitioning to history
    pub fn complete_compression(&mut self, original_tokens: usize, new_tokens: usize) {
        // Set animator to completed state (shows result in animation box)
        self.compaction_animator.complete(original_tokens, new_tokens);
        // Keep pending_compression for rendering, will be cleared after display duration
        if self.scroll_state.auto_scroll {
            self.scroll_state.scroll_to_bottom();
            self.scrollbar.flash();
        }
    }

    /// Complete compression with "not beneficial" result
    pub fn complete_compression_not_beneficial(&mut self, original_tokens: usize) {
        self.compaction_animator.stop();
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
        // Set animator to failed state
        self.compaction_animator.fail(error.clone());
        // Keep pending_compression for rendering
        if self.scroll_state.auto_scroll {
            self.scroll_state.scroll_to_bottom();
            self.scrollbar.flash();
        }
    }

    /// Complete compression with nothing to compress
    pub fn complete_compression_noop(&mut self) {
        self.compaction_animator.stop();
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

    /// Tick for animation updates (compaction animator, etc.)
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

        if self.scroll_state.auto_scroll {
            self.scroll_state.scroll_to_bottom();
            self.scrollbar.flash();
        }
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

        self.scrollbar.update();

        let colors = get_colors();
        let mut lines: Vec<Line> = Vec::new();
        let content_width = area.width.saturating_sub(4) as usize;
        let mut last_turn: Option<usize> = None;

        for item in self.items.iter() {
            // Get the turn number for this item
            let item_turn = match item {
                HistoryItem::User { turn, .. } => Some(*turn),
                HistoryItem::Assistant { turn, .. } => Some(*turn),
                HistoryItem::Thinking { turn, .. } => Some(*turn),
                HistoryItem::ToolGroup { turn, .. } => Some(*turn),
                HistoryItem::Compression { .. } => None,
            };

            // Add turn separator if this is a new turn
            if let Some(turn) = item_turn {
                if last_turn.map_or(true, |lt| lt != turn) {
                    // Turn separator line
                    if last_turn.is_some() {
                        lines.push(Line::from(""));
                    }
                    lines.push(Self::render_turn_separator(turn, &colors));
                    lines.push(Line::from(""));
                    last_turn = Some(turn);
                }
            }

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
                                    .add_modifier(Modifier::BOLD),
                            ));
                        } else {
                            spans.push(Span::raw("    "));
                        }
                        spans.extend(content_line.spans.into_iter());
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
                                Style::default()
                                    .fg(colors.text.accent)
                                    .add_modifier(Modifier::BOLD),
                            ));
                        } else {
                            spans.push(Span::raw("    "));
                        }
                        spans.extend(content_line.spans.into_iter());
                        lines.push(Line::from(spans));
                    }
                    lines.push(Line::from(""));
                }

                HistoryItem::ToolGroup { group, .. } => {
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

                HistoryItem::Thinking { content, is_collapsed, .. } => {
                    // Thinking block header
                    let collapse_icon = if *is_collapsed { "+" } else { "-" };
                    lines.push(Line::from(vec![
                        Span::styled("┌─ ", Style::default().fg(colors.text.secondary)),
                        Span::styled("[", Style::default().fg(colors.ui.dark)),
                        Span::styled(collapse_icon, Style::default().fg(colors.text.accent)),
                        Span::styled("] ", Style::default().fg(colors.ui.dark)),
                        Span::styled("thinking", Style::default()
                            .fg(colors.text.secondary)
                            .add_modifier(Modifier::ITALIC)),
                        Span::styled(" ─", Style::default().fg(colors.text.secondary)),
                        Span::styled("─".repeat(20), Style::default().fg(colors.ui.dark)),
                    ]));

                    // Thinking content (if not collapsed)
                    if !is_collapsed {
                        for thinking_line in content.lines() {
                            lines.push(Line::from(vec![
                                Span::styled("│ ", Style::default().fg(colors.ui.dark)),
                                Span::styled(thinking_line.to_string(), Style::default()
                                    .fg(colors.text.secondary)
                                    .add_modifier(Modifier::ITALIC)),
                            ]));
                        }
                    }

                    // Thinking block footer
                    lines.push(Line::from(vec![
                        Span::styled("└", Style::default().fg(colors.text.secondary)),
                        Span::styled("─".repeat(30), Style::default().fg(colors.ui.dark)),
                    ]));
                    lines.push(Line::from(""));
                }
            }
        }

        // Render pending compression with beautiful animated display
        if let Some(ref _compression) = self.pending_compression {
            use crate::ui::components::compaction_animation::CompactionState;

            let state = self.compaction_animator.state();

            match state {
                CompactionState::Running => {
                    // Running animation
                    let spinner = self.compaction_animator.spinner_frame();
                    let animated_color = self.compaction_animator.get_animated_color();
                    let progress_color = self.compaction_animator.get_progress_color();
                    let message = self.compaction_animator.current_message();
                    let progress = self.compaction_animator.progress();
                    let progress_bar = self.compaction_animator.render_progress_bar_smooth(20);
                    let elapsed = self.compaction_animator.elapsed_string();

                    // Top border with spinner
                    lines.push(Line::from(vec![
                        Span::styled("╭─", Style::default().fg(colors.ui.dark)),
                        Span::styled("─".repeat(40), Style::default().fg(colors.ui.dark)),
                    ]));

                    // Main message line with animated spinner
                    lines.push(Line::from(vec![
                        Span::styled("│ ", Style::default().fg(colors.ui.dark)),
                        Span::styled(format!("{} ", spinner), Style::default().fg(animated_color)),
                        Span::styled(
                            format!("{}...", message),
                            Style::default()
                                .fg(colors.text.primary)
                                .add_modifier(Modifier::BOLD),
                        ),
                    ]));

                    // Progress bar line
                    lines.push(Line::from(vec![
                        Span::styled("│ ", Style::default().fg(colors.ui.dark)),
                        Span::styled("   ", Style::default()),
                        Span::styled(progress_bar, Style::default().fg(progress_color)),
                        Span::styled(
                            format!(" {:>3}%", progress),
                            Style::default().fg(colors.text.secondary),
                        ),
                        Span::styled(
                            format!("  {}", elapsed),
                            Style::default().fg(colors.ui.comment),
                        ),
                    ]));

                    // Bottom border
                    lines.push(Line::from(vec![
                        Span::styled("╰─", Style::default().fg(colors.ui.dark)),
                        Span::styled("─".repeat(40), Style::default().fg(colors.ui.dark)),
                    ]));
                }
                CompactionState::Completed => {
                    // Show completion result
                    if let Some(result) = self.compaction_animator.result() {
                        let success_color = self.compaction_animator.get_success_color();
                        let saved = result.original_tokens.saturating_sub(result.new_tokens);
                        let percent = if result.original_tokens > 0 {
                            saved * 100 / result.original_tokens
                        } else {
                            0
                        };

                        // Success box
                        lines.push(Line::from(vec![
                            Span::styled("╭─", Style::default().fg(success_color)),
                            Span::styled("─".repeat(40), Style::default().fg(colors.ui.dark)),
                        ]));

                        lines.push(Line::from(vec![
                            Span::styled("│ ", Style::default().fg(colors.ui.dark)),
                            Span::styled("✓ ", Style::default().fg(success_color).add_modifier(Modifier::BOLD)),
                            Span::styled(
                                "Chat history compacted",
                                Style::default()
                                    .fg(colors.text.primary)
                                    .add_modifier(Modifier::BOLD),
                            ),
                        ]));

                        // Token reduction line
                        let bar = self.compaction_animator.render_completion_bar(16);
                        lines.push(Line::from(vec![
                            Span::styled("│ ", Style::default().fg(colors.ui.dark)),
                            Span::styled("   ", Style::default()),
                            Span::styled(bar, Style::default().fg(success_color)),
                            Span::styled(
                                format!("  {} → {} tokens", result.original_tokens, result.new_tokens),
                                Style::default().fg(colors.text.secondary),
                            ),
                        ]));

                        // Saved line
                        lines.push(Line::from(vec![
                            Span::styled("│ ", Style::default().fg(colors.ui.dark)),
                            Span::styled(
                                format!("   Saved {}% (~{} tokens)", percent, saved),
                                Style::default().fg(success_color),
                            ),
                        ]));

                        lines.push(Line::from(vec![
                            Span::styled("╰─", Style::default().fg(success_color)),
                            Span::styled("─".repeat(40), Style::default().fg(colors.ui.dark)),
                        ]));
                    }
                }
                CompactionState::Failed => {
                    // Show error result
                    if let Some(result) = self.compaction_animator.result() {
                        let error_color = self.compaction_animator.get_error_color();
                        let error_msg = result.error_message.as_deref().unwrap_or("Unknown error");

                        lines.push(Line::from(vec![
                            Span::styled("╭─", Style::default().fg(error_color)),
                            Span::styled("─".repeat(40), Style::default().fg(colors.ui.dark)),
                        ]));

                        lines.push(Line::from(vec![
                            Span::styled("│ ", Style::default().fg(colors.ui.dark)),
                            Span::styled("✗ ", Style::default().fg(error_color).add_modifier(Modifier::BOLD)),
                            Span::styled(
                                "Compaction failed",
                                Style::default()
                                    .fg(colors.text.primary)
                                    .add_modifier(Modifier::BOLD),
                            ),
                        ]));

                        lines.push(Line::from(vec![
                            Span::styled("│ ", Style::default().fg(colors.ui.dark)),
                            Span::styled(
                                format!("   {}", error_msg),
                                Style::default().fg(error_color),
                            ),
                        ]));

                        lines.push(Line::from(vec![
                            Span::styled("╰─", Style::default().fg(error_color)),
                            Span::styled("─".repeat(40), Style::default().fg(colors.ui.dark)),
                        ]));
                    }
                }
                CompactionState::Idle => {
                    // Should not happen when pending_compression is Some
                }
            }
            lines.push(Line::from(""));
        }

        if let Some(ref group) = self.pending_tool_group {
            // Add turn separator for pending tools if needed
            if last_turn.map_or(true, |lt| lt != self.current_turn) {
                if last_turn.is_some() {
                    lines.push(Line::from(""));
                }
                lines.push(Self::render_turn_separator(self.current_turn, &colors));
                lines.push(Line::from(""));
                last_turn = Some(self.current_turn);
            }

            let tool_lines = render_tool_group_with_limit(group, area, area.height as usize);
            for line in tool_lines {
                lines.push(line);
            }
            lines.push(Line::from(""));
        }

        // Render streaming thinking (if any)
        if let Some(ref thinking) = self.streaming_thinking {
            lines.push(Line::from(vec![
                Span::styled("┌─ ", Style::default().fg(colors.text.secondary)),
                Span::styled("thinking", Style::default()
                    .fg(colors.text.secondary)
                    .add_modifier(Modifier::ITALIC)),
                Span::styled(" ─", Style::default().fg(colors.text.secondary)),
                Span::styled("─".repeat(20), Style::default().fg(colors.ui.dark)),
            ]));

            for thinking_line in thinking.lines() {
                lines.push(Line::from(vec![
                    Span::styled("│ ", Style::default().fg(colors.ui.dark)),
                    Span::styled(thinking_line.to_string(), Style::default()
                        .fg(colors.text.secondary)
                        .add_modifier(Modifier::ITALIC)),
                ]));
            }

            lines.push(Line::from(vec![
                Span::styled("└", Style::default().fg(colors.text.secondary)),
                Span::styled("─".repeat(30), Style::default().fg(colors.ui.dark)),
            ]));
            lines.push(Line::from(""));
        }

        if let Some(ref streaming) = self.streaming_text {
            // Add turn separator for streaming if needed
            if last_turn.map_or(true, |lt| lt != self.current_turn) {
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
                        format!("✦ [{}] ", self.current_turn),
                        Style::default()
                            .fg(colors.text.accent)
                            .add_modifier(Modifier::BOLD),
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

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_scroll_methods() {
        let mut messages = MessagesComponent::new();

        // Add some content
        for i in 0..100 {
            messages.add_user(format!("Message {}", i));
        }

        messages.scroll_up(5);
        messages.scroll_down(3);
        messages.scroll_to_top();
        messages.scroll_to_bottom();
        messages.scroll_page_up();
        messages.scroll_page_down();
        // Just verify these don't panic
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
