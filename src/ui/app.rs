use std::path::PathBuf;
use std::time::{Duration, Instant};

use crossterm::{
    cursor::MoveTo,
    event::{KeyCode, KeyModifiers, MouseButton, MouseEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode},
    terminal::{Clear, ClearType},
};
use futures::StreamExt;
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    text::Line,
    Terminal, TerminalOptions, Viewport,
};
use tokio::sync::mpsc;
use tracing::{debug, info, warn};
use unicode_width::UnicodeWidthChar;

use crate::agent::events::{AgentEvent, ApprovalDecision, ApprovalRequest};
use crate::agent::loop_agent::{create_approval_channels, Agent};
use crate::client::LLMClient;
use crate::error::Result;
use crate::ui::components::{
    render_markdown, ApprovalDialog, ApprovalResponse, FileSidebar, Footer, HelpSidebar,
    InputComponent, LoadingIndicator, MessagesComponent, Sidebar, StatusBar, StreamingState,
};
use crate::ui::event::{AppEvent, EventHandler};
use crate::ui::{get_theme, next_theme, SidebarKind};

struct StreamingBuffer {
    text: String,
    last_flush: Instant,
    allocated_height: u16,
    last_printed_height: u16,
}

impl StreamingBuffer {
    fn new() -> Self {
        Self {
            text: String::new(),
            last_flush: Instant::now(),
            allocated_height: 0,
            last_printed_height: 0,
        }
    }

    fn push(&mut self, delta: &str) {
        self.text.push_str(delta);
    }

    fn should_flush(&self) -> bool {
        let time_elapsed = self.last_flush.elapsed() >= Duration::from_millis(150);
        let chars_accumulated = self.text.len() >= 32;
        time_elapsed || chars_accumulated
    }

    fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    fn clear(&mut self) {
        self.text.clear();
        self.allocated_height = 0;
        self.last_printed_height = 0;
    }
}

/// A simple stateful filter to remove XML-like tags and their content from a stream
struct TagFilter {
    buffer: String,
    suppressing: bool,
    /// Tags that trigger suppression of content between opening and closing
    tags: Vec<String>,
}

impl TagFilter {
    fn new() -> Self {
        Self {
            buffer: String::new(),
            suppressing: false,
            tags: vec![
                "Claude_TalktoUser".to_string(),
                "think".to_string(),
                "thought".to_string(),
                "thinking".to_string(),
            ],
        }
    }

    fn process(&mut self, delta: &str) -> String {
        let mut result = String::new();
        self.buffer.push_str(delta);

        while !self.buffer.is_empty() {
            if !self.suppressing {
                if let Some(start_idx) = self.buffer.find('<') {
                    // Output everything before the '<'
                    result.push_str(&self.buffer[..start_idx]);
                    self.buffer.drain(..start_idx);

                    // Check if we have the full tag
                    if let Some(end_idx) = self.buffer.find('>') {
                        let tag_content = &self.buffer[1..end_idx];
                        let tag_name = tag_content.strip_prefix('/').unwrap_or(tag_content);

                        // Check if it's one of our target tags
                        if self.tags.iter().any(|t| tag_name == t) {
                            // If it's an opening tag, start suppressing
                            if !tag_content.starts_with('/') {
                                self.suppressing = true;
                            }
                            self.buffer.drain(..=end_idx);
                        } else {
                            // Not a target tag, emit the '<' and continue from next char
                            result.push('<');
                            self.buffer.drain(..1);
                        }
                    } else {
                        // We have a '<' but no '>', check if it looks like a tag
                        let next_char = self.buffer.get(1..2).and_then(|s| s.chars().next());
                        let looks_like_tag =
                            next_char.map_or(false, |c| c.is_alphanumeric() || c == '/');

                        if !looks_like_tag || self.buffer.len() > 100 {
                            result.push('<');
                            self.buffer.drain(..1);
                        } else {
                            // Keep the buffer starting from '<' and stop processing
                            break;
                        }
                    }
                } else {
                    // No '<' found, output everything
                    result.push_str(&self.buffer);
                    self.buffer.clear();
                }
            } else {
                // We are suppressing content
                if let Some(start_idx) = self.buffer.find('<') {
                    if let Some(end_idx) = self.buffer[start_idx..].find('>') {
                        let full_end_idx = start_idx + end_idx;
                        let tag_content = &self.buffer[start_idx + 1..full_end_idx];

                        // Check if it's a closing tag of one of our target tags
                        if tag_content.starts_with('/') {
                            let tag_name = &tag_content[1..];
                            if self.tags.iter().any(|t| tag_name == t) {
                                self.suppressing = false;
                            }
                        }

                        self.buffer.drain(..=full_end_idx);
                    } else {
                        // Wait for more data
                        if self.buffer.len() > 500 {
                            // Safety valve: stop suppressing if we've gone too long without a tag
                            self.suppressing = false;
                            self.buffer.drain(..=start_idx);
                        } else {
                            // Keep everything from the first '<' for more data
                            self.buffer.drain(..start_idx);
                            break;
                        }
                    }
                } else {
                    // No '<' in suppressing mode, just clear the buffer
                    self.buffer.clear();
                }
            }
        }

        result
    }

    /// Flush any remaining content that isn't being suppressed
    fn finalize(&mut self) -> String {
        let res = if !self.suppressing {
            self.buffer.clone()
        } else {
            String::new()
        };
        self.buffer.clear();
        self.suppressing = false;
        res
    }
}

pub enum AppMode {
    Normal,
    Input,
    Approval,
}

pub struct App<C: LLMClient> {
    agent: Agent<C>,
    mode: AppMode,
    messages: MessagesComponent,
    input: InputComponent,
    footer: Footer,
    loading_indicator: LoadingIndicator,
    status_bar: StatusBar,
    sidebar: Option<Sidebar>,
    should_quit: bool,
    workdir: PathBuf,
    stream_rx: Option<mpsc::Receiver<AgentEvent>>,
    stream_abort: Option<tokio::task::JoinHandle<()>>,
    current_text: String,
    messages_area: Rect,
    sidebar_area: Rect,
    mesh_supervisor_addr: Option<String>,
    /// Approval dialog state
    approval_dialog: Option<ApprovalDialog>,
    /// Channel to send approval decisions back to agent (for current stream)
    approval_dec_tx: Option<mpsc::Sender<(String, ApprovalDecision)>>,
    /// Channel to receive approval requests from agent (for current stream)
    approval_req_rx: Option<mpsc::Receiver<ApprovalRequest>>,
    /// Channel to receive compaction results from background task
    compaction_result_rx: Option<
        mpsc::Receiver<
            std::result::Result<
                crate::agent::compaction::CompactionResult,
                crate::error::AgentError,
            >,
        >,
    >,
    /// Whether the current stream is running in background
    is_background: bool,
    /// Buffer for streaming text before flushing to terminal
    streaming_buffer: StreamingBuffer,
    /// Whether we need to flush and add spacing after stream completes
    needs_final_flush: bool,
    /// Error message to display (if any)
    pending_error: Option<String>,
    /// Pending user input to display (set by handle_key, flushed in run_loop)
    pending_user_input: Option<String>,
    /// Flag to flush buffer before compaction
    flush_before_compaction: bool,
    /// Filter for removing internal tags
    tag_filter: TagFilter,
    /// Configurable viewport height as percentage of terminal height
    viewport_height_percent: u16,
}

impl<C: LLMClient + Clone + 'static> App<C> {
    pub fn new(agent: Agent<C>, workdir: PathBuf, model_name: String) -> Self {
        let footer = Footer::new(model_name.clone());
        let loading_indicator = LoadingIndicator::new();
        let status_bar = StatusBar::new();

        Self {
            agent,
            mode: AppMode::Input,
            messages: MessagesComponent::new(),
            input: InputComponent::new(),
            footer,
            loading_indicator,
            status_bar,
            sidebar: None,
            should_quit: false,
            workdir,
            stream_rx: None,
            stream_abort: None,
            current_text: String::new(),
            messages_area: Rect::default(),
            sidebar_area: Rect::default(),
            mesh_supervisor_addr: None,
            approval_dialog: None,
            approval_dec_tx: None,
            approval_req_rx: None,
            compaction_result_rx: None,
            is_background: false,
            streaming_buffer: StreamingBuffer::new(),
            needs_final_flush: false,
            pending_error: None,
            pending_user_input: None,
            flush_before_compaction: false,
            tag_filter: TagFilter::new(),
            viewport_height_percent: 32,
        }
    }

    pub fn set_mesh_mode(&mut self, addr: &str) {
        self.mesh_supervisor_addr = Some(addr.to_string());
        self.footer.set_mesh(true);
    }

    pub async fn run(&mut self) -> Result<()> {
        enable_raw_mode()?;

        let res = self.run_loop().await;

        disable_raw_mode()?;
        // Use a temporary terminal to show cursor if needed
        let backend = CrosstermBackend::new(std::io::stdout());
        let mut terminal = Terminal::new(backend)?;
        terminal.show_cursor()?;

        res
    }

    fn find_fluggable_index(&self, text: &str) -> Option<usize> {
        // Find double newline which indicates end of paragraph
        if let Some(idx) = text.find("\n\n") {
            return Some(idx + 2);
        }

        // Also check if we have a closed code block that hasn't been flushed
        // This is a bit trickier as we need to distinguish opening from closing fences
        let mut count = 0;
        for (idx, _) in text.match_indices("```") {
            count += 1;
            let last_idx = idx + 3;
            // Check for newline after closing fence
            if count % 2 == 0 {
                if let Some(after) = text[last_idx..].find('\n') {
                    return Some(last_idx + after + 1);
                }
            }
        }

        None
    }

    async fn run_loop(&mut self) -> Result<()> {
        let stdout = std::io::stdout();
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::with_options(
            backend,
            TerminalOptions {
                viewport: Viewport::Inline(3),
            },
        )?;

        let mut events = EventHandler::new(Duration::from_millis(100));
        loop {
            if self.should_quit {
                break;
            }

            let width = terminal.size()?.width;

            // Render dashboard if needed (startup or after compaction)
            // This happens BEFORE terminal.draw() to ensure dashboard appears above input box
            if self.messages.should_render_dashboard_to_history() {
                let mut dashboard_lines = self.messages.render_dashboard_lines(width);
                if !dashboard_lines.is_empty() {
                    dashboard_lines.push(Line::from("")); // Add newline after dashboard
                    let height = dashboard_lines.len() as u16;
                    terminal.insert_before(height, |buf| {
                        let area = Rect::new(0, 0, buf.area.width, height);
                        let p = ratatui::widgets::Paragraph::new(dashboard_lines.clone());
                        ratatui::widgets::Widget::render(p, area, buf);
                    })?;
                }
                self.messages.mark_dashboard_rendered();
            }

            terminal.draw(|f| self.render(f))?;

            // Periodic flush for responsiveness
            if !self.streaming_buffer.is_empty() && self.streaming_buffer.should_flush() {
                self.flush_streaming_buffer(&mut terminal)?;
            }

            tokio::select! {
                event = events.next() => {
                    self.handle_event(event?).await?;

                    // Flush pending user input if any
                    if self.pending_user_input.is_some() {
                        self.flush_user_input(&mut terminal)?;
                    }

                    // Handle compaction flush
                    if self.flush_before_compaction {
                        self.flush_streaming_buffer(&mut terminal)?;
                        self.flush_before_compaction = false;
                        self.start_compaction();
                    }

                    // Switch to fast tick rate when streaming starts
                    if self.stream_rx.is_some() {
                        events.set_tick_rate(Duration::from_millis(16));
                    }
                }

                agent_event = async {
                    match &mut self.stream_rx {
                        Some(rx) => rx.recv().await,
                        None => std::future::pending().await,
                    }
                } => {
                    if let Some(event) = agent_event {
                        if self.handle_agent_event(event) {
                            self.stream_rx = None;
                            self.stream_abort = None;
                            events.set_tick_rate(Duration::from_millis(100));
                        }

                        // Batch process remaining available events (max 100 per batch)
                        const MAX_BATCH_SIZE: usize = 100;
                        let events_to_process: Vec<AgentEvent> = if let Some(ref mut rx) = self.stream_rx {
                            let mut collected = Vec::new();
                            let mut batch_count = 0;
                            while batch_count < MAX_BATCH_SIZE {
                                match rx.try_recv() {
                                    Ok(event) => {
                                        collected.push(event);
                                        batch_count += 1;
                                    }
                                    Err(mpsc::error::TryRecvError::Empty) |
                                    Err(mpsc::error::TryRecvError::Disconnected) => break,
                                }
                            }
                            if !collected.is_empty() {
                                debug!(batch_size = collected.len(), "Batch processing events");
                            }
                            collected
                        } else {
                            Vec::new()
                        };

                        for event in events_to_process {
                            if self.handle_agent_event(event) {
                                self.stream_rx = None;
                                self.stream_abort = None;
                                events.set_tick_rate(Duration::from_millis(100));
                                break;
                            }
                        }

                        // Flush buffer periodically during streaming
                        if !self.streaming_buffer.is_empty() && self.streaming_buffer.should_flush() {
                            self.flush_streaming_buffer(&mut terminal)?;
                        }

                        // Handle final flush after stream completes
                        if self.needs_final_flush {
                            self.flush_and_finalize(&mut terminal)?;
                        }

                        // Force immediate redraw to show streaming progress
                        terminal.draw(|f| self.render(f))?;
                    } else {
                        self.stream_rx = None;
                        self.stream_abort = None;
                        events.set_tick_rate(Duration::from_millis(100));
                    }
                }

                // Poll for approval requests from agent
                approval_request = async {
                    match &mut self.approval_req_rx {
                        Some(rx) => rx.recv().await,
                        None => std::future::pending().await,
                    }
                } => {
                    if let Some(request) = approval_request {
                        // Flush buffer before showing approval dialog
                        self.flush_streaming_buffer(&mut terminal)?;
                        self.show_approval_dialog(request);
                    }
                }
            }
        }

        Ok(())
    }

    fn flush_streaming_buffer(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    ) -> Result<()> {
        if self.streaming_buffer.is_empty() {
            return Ok(());
        }

        const BOTTOM_UI_HEIGHT: u16 = 3;

        let stdout = std::io::stdout();
        let mut stdout_lock = stdout.lock();

        let terminal_size = terminal.size()?;
        let terminal_width = terminal_size.width;
        let terminal_height = terminal_size.height;
        let content_width = terminal_width.saturating_sub(4) as usize;

        let max_viewport_height = (terminal_height * self.viewport_height_percent) / 100;

        // --- 1. PARAGRAPH FLUSHING ---
        if let Some(flush_idx) = self.find_fluggable_index(&self.streaming_buffer.text) {
            let fluggable_text = self.streaming_buffer.text[..flush_idx].to_string();
            let fluggable_lines = render_markdown(&fluggable_text, content_width);
            let flush_height = fluggable_lines.len() as u16;

            if flush_height > 0 {
                // SURGICAL CLEAR of the existing hole only
                if self.streaming_buffer.allocated_height > 0 {
                    let top_row = terminal_height
                        .saturating_sub(BOTTOM_UI_HEIGHT)
                        .saturating_sub(self.streaming_buffer.allocated_height);

                    for row_offset in 0..self.streaming_buffer.allocated_height {
                        execute!(
                            stdout_lock,
                            MoveTo(0, top_row + row_offset),
                            Clear(ClearType::UntilNewLine)
                        )?;
                    }
                }

                // 2. Commit to history.
                terminal.insert_before(flush_height, |buf| {
                    for (i, line) in fluggable_lines.into_iter().enumerate() {
                        if i >= flush_height as usize {
                            break;
                        }
                        // Use Line's native widget implementation for robust rendering.
                        // This handles CJK widths and formatting perfectly.
                        use ratatui::widgets::Widget;
                        line.render(
                            Rect::new(2, i as u16, buf.area.width.saturating_sub(2), 1),
                            buf,
                        );
                    }
                })?;

                self.streaming_buffer.text = self.streaming_buffer.text[flush_idx..].to_string();
                self.streaming_buffer.allocated_height = 0;
                self.streaming_buffer.last_printed_height = 0;

                if self.streaming_buffer.is_empty() {
                    terminal.draw(|f| self.render(f))?;
                    return Ok(());
                }
            }
        }

        // --- 2. LIVE VIEWPORT ---
        let rendered_lines = render_markdown(&self.streaming_buffer.text, content_width);
        let needed_height = rendered_lines.len() as u16;

        let visible_height = std::cmp::min(needed_height, max_viewport_height);
        let drop_count = needed_height.saturating_sub(visible_height) as usize;

        // Ensure hole exists
        if self.streaming_buffer.allocated_height == 0 {
            terminal.insert_before(visible_height, |_| {})?;
            self.streaming_buffer.allocated_height = visible_height;
        } else if visible_height > self.streaming_buffer.allocated_height {
            let diff = visible_height - self.streaming_buffer.allocated_height;
            terminal.insert_before(diff, |_| {})?;
            self.streaming_buffer.allocated_height = visible_height;
        }

        // Surgical redraw viewport
        let top_row = terminal_height
            .saturating_sub(BOTTOM_UI_HEIGHT)
            .saturating_sub(self.streaming_buffer.allocated_height);

        // CLEAR ONLY THE ALLOCATED ROWS surgically
        for row_offset in 0..self.streaming_buffer.allocated_height {
            execute!(
                stdout_lock,
                MoveTo(0, top_row + row_offset),
                Clear(ClearType::UntilNewLine)
            )?;
        }

        // Draw ONLY the visible lines in their correct positions
        for (i, line) in rendered_lines.into_iter().skip(drop_count).enumerate() {
            if i >= visible_height as usize {
                break;
            }
            execute!(stdout_lock, MoveTo(2, top_row + i as u16))?;
            for span in line.spans {
                print!("{}", span.content);
            }
        }

        // Restore cursor and Ratatui UI
        execute!(stdout_lock, MoveTo(0, terminal_height.saturating_sub(1)))?;
        let _ = std::io::Write::flush(&mut std::io::stdout());

        terminal.draw(|f| self.render(f))?;

        self.streaming_buffer.last_printed_height = visible_height;
        self.streaming_buffer.last_flush = Instant::now();

        Ok(())
    }

    fn flush_and_finalize(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    ) -> Result<()> {
        const BOTTOM_UI_HEIGHT: u16 = 3;

        // Finalize filtering first to get all text
        let remainder = self.tag_filter.finalize();
        if !remainder.is_empty() {
            self.streaming_buffer.push(&remainder);
            self.current_text.push_str(&remainder);
        }

        if self.streaming_buffer.is_empty() {
            // Surgical clear of residual hole
            if self.streaming_buffer.allocated_height > 0 {
                let stdout = std::io::stdout();
                let mut stdout_lock = stdout.lock();
                let terminal_height = terminal.size()?.height;
                let top_row = terminal_height
                    .saturating_sub(BOTTOM_UI_HEIGHT)
                    .saturating_sub(self.streaming_buffer.allocated_height);

                for row_offset in 0..self.streaming_buffer.allocated_height {
                    execute!(
                        stdout_lock,
                        MoveTo(0, top_row + row_offset),
                        Clear(ClearType::UntilNewLine)
                    )?;
                }
                self.streaming_buffer.allocated_height = 0;
                terminal.draw(|f| self.render(f))?;
            }
            self.needs_final_flush = false;
            return Ok(());
        }

        // --- FINAL COMMIT TO HISTORY ---
        // 1. Surgical clear of viewport rows
        if self.streaming_buffer.allocated_height > 0 {
            let stdout = std::io::stdout();
            let mut stdout_lock = stdout.lock();
            let terminal_height = terminal.size()?.height;
            let top_row = terminal_height
                .saturating_sub(BOTTOM_UI_HEIGHT)
                .saturating_sub(self.streaming_buffer.allocated_height);

            for row_offset in 0..self.streaming_buffer.allocated_height {
                execute!(
                    stdout_lock,
                    MoveTo(0, top_row + row_offset),
                    Clear(ClearType::UntilNewLine)
                )?;
            }
        }

        // 2. Render and insert remaining lines
        let terminal_width = terminal.size()?.width;
        let content_width = terminal_width.saturating_sub(4) as usize;
        let rendered_lines = render_markdown(&self.streaming_buffer.text, content_width);
        let final_height = rendered_lines.len() as u16;

        if final_height > 0 {
            terminal.insert_before(final_height, |buf| {
                let area = Rect::new(0, 0, buf.area.width, final_height);
                for (i, line) in rendered_lines.into_iter().enumerate() {
                    if i >= final_height as usize {
                        break;
                    }
                    let mut x = 2; // Left padding
                    for span in line.spans {
                        let cell_idx = (area.y + i as u16) * buf.area.width + (area.x + x);
                        if cell_idx < buf.content.len() as u16 {
                            buf.content[cell_idx as usize].set_symbol(&span.content);
                        }
                        x += span.content.len() as u16;
                    }
                }
            })?;
        }

        // Add consistent spacing
        terminal.insert_before(1, |_| {})?;

        // Handle error message if present
        if let Some(error_msg) = self.pending_error.take() {
            let error_line = Line::styled(
                format!("Error: {}", error_msg),
                ratatui::style::Style::default().fg(ratatui::style::Color::Red),
            );
            terminal.insert_before(2, |buf| {
                let area = Rect::new(0, 0, buf.area.width, 2);
                let p = ratatui::widgets::Paragraph::new(vec![Line::from(""), error_line]);
                ratatui::widgets::Widget::render(p, area, buf);
            })?;
        }

        // Reset state
        self.streaming_buffer.clear();
        self.streaming_buffer.allocated_height = 0;
        self.streaming_buffer.last_printed_height = 0;
        self.needs_final_flush = false;
        Ok(())
    }

    fn flush_user_input(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    ) -> Result<()> {
        if let Some(user_input) = self.pending_user_input.take() {
            let user_lines = vec![
                Line::styled(
                    format!("> {}", user_input),
                    ratatui::style::Style::default().fg(ratatui::style::Color::Cyan),
                ),
                Line::from(""),
            ];
            let height = user_lines.len() as u16;
            terminal.insert_before(height, |buf| {
                let area = Rect::new(0, 0, buf.area.width, height);
                let p = ratatui::widgets::Paragraph::new(user_lines);
                ratatui::widgets::Widget::render(p, area, buf);
            })?;
        }
        Ok(())
    }

    async fn handle_event(&mut self, event: AppEvent) -> Result<()> {
        match event {
            AppEvent::Key(key) => self.handle_key(key).await?,
            AppEvent::Mouse(mouse) => self.handle_mouse(mouse),
            AppEvent::Resize(_, _) => {}
            AppEvent::Tick => {
                self.footer.tick();
                self.loading_indicator.tick();
                self.messages.tick();
                self.status_bar.tick();
                // Poll for compaction result (non-blocking)
                self.poll_compaction_result();
            }
        }
        Ok(())
    }

    fn handle_mouse(&mut self, event: crossterm::event::MouseEvent) {
        match event.kind {
            MouseEventKind::Down(MouseButton::Left) | MouseEventKind::Drag(MouseButton::Left) => {
                let scrollbar_x = self.messages_area.x + self.messages_area.width.saturating_sub(1);
                if event.column == scrollbar_x
                    && event.row >= self.messages_area.y
                    && event.row < self.messages_area.y + self.messages_area.height
                {
                    let relative_y = event.row - self.messages_area.y;
                    let height = self.messages_area.height.max(1);
                    let ratio = relative_y as f32 / height as f32;
                    self.messages.scroll_to_ratio(ratio);
                }
            }
            MouseEventKind::ScrollUp => {
                if self.is_mouse_in_messages_area(event.column, event.row) {
                    self.messages.scroll_up(3);
                }
            }
            MouseEventKind::ScrollDown => {
                if self.is_mouse_in_messages_area(event.column, event.row) {
                    self.messages.scroll_down(3);
                }
            }
            _ => {}
        }
    }

    fn is_mouse_in_messages_area(&self, x: u16, y: u16) -> bool {
        x >= self.messages_area.x
            && x < self.messages_area.x + self.messages_area.width
            && y >= self.messages_area.y
            && y < self.messages_area.y + self.messages_area.height
    }

    fn handle_agent_event(&mut self, event: AgentEvent) -> bool {
        match event {
            AgentEvent::TextDelta { delta } => {
                debug!(delta_len = delta.len(), delta = %delta, "Received TextDelta");

                // Filter out internal tags
                let filtered_delta = self.tag_filter.process(&delta);
                if filtered_delta.is_empty() {
                    return false;
                }

                // Accumulate in buffer
                self.streaming_buffer.push(&filtered_delta);
                self.current_text.push_str(&filtered_delta);

                // Update internal state
                if !self.status_bar.is_active() {
                    self.status_bar.start();
                }
                self.status_bar.set_thinking(false);
                self.status_bar.update_text(&filtered_delta);
                debug!(
                    total_len = self.current_text.len(),
                    buffer_len = self.streaming_buffer.text.len(),
                    "Streaming text accumulated"
                );
            }

            AgentEvent::ThinkingDelta { delta } => {
                if !self.status_bar.is_active() {
                    self.status_bar.start();
                }
                self.status_bar.set_thinking(true);
                self.messages.update_thinking(&delta);
            }

            AgentEvent::ThinkingComplete { thinking } => {
                // Finalize the thinking block
                if !thinking.is_empty() {
                    self.messages.update_thinking(&thinking);
                }
                self.messages.finalize_thinking();
            }

            AgentEvent::ToolStart { id, name } => {
                self.status_bar.set_thinking(true);
                self.messages.start_tool(id, name, None);
            }

            AgentEvent::ToolComplete {
                id,
                name,
                input,
                output,
                is_error,
            } => {
                let command = if name == "bash" {
                    input
                        .get("command")
                        .and_then(|v: &serde_json::Value| v.as_str())
                        .map(String::from)
                } else {
                    None
                };
                self.messages.complete_tool(&id, output, is_error, command);
            }

            AgentEvent::Done { result } => {
                debug!(text_len = result.text.len(), "Agent stream completed");

                self.messages.finalize_assistant(result.text);
                self.loading_indicator
                    .set_streaming_state(StreamingState::Idle);
                self.current_text.clear();
                self.status_bar.stop();
                if self.is_background {
                    self.is_background = false;
                    self.footer.set_background(false);
                    self.footer.set_status_message("Background task completed");
                }

                // Set flag to flush and add spacing
                self.needs_final_flush = true;
                return true;
            }

            AgentEvent::Error { message } => {
                if self.current_text.is_empty() {
                    self.messages.add_assistant(format!("Error: {}", message));
                } else {
                    self.messages
                        .finalize_assistant(format!("{}\n\nError: {}", self.current_text, message));
                }
                self.loading_indicator
                    .set_streaming_state(StreamingState::Idle);
                self.current_text.clear();
                self.status_bar.stop();
                if self.is_background {
                    self.is_background = false;
                    self.footer.set_background(false);
                    self.footer.set_status_message("Background task failed");
                }

                // Set flag to flush and show error
                self.needs_final_flush = true;
                self.pending_error = Some(message);
                return true;
            }

            AgentEvent::SessionSaved { path } => {
                info!(path = %path, "Session log saved to disk");
            }

            AgentEvent::ToolInputDelta { .. } => {
                self.status_bar.set_thinking(true);
            }

            AgentEvent::ApprovalRequired { request } => {
                // This event is just for logging/notification.
                // The actual approval dialog is triggered via the approval channel
                // which is polled in the main select! loop.
                info!(tool = %request.tool, reason = %request.reason, "Approval required for tool execution");
            }

            AgentEvent::TokenUsage {
                input_tokens,
                output_tokens,
                total_tokens,
            } => {
                // Only update input tokens from API - output tokens tracked from text deltas
                self.status_bar.update_input_tokens(input_tokens);

                // Calculate context window percentage
                let context_size = self.agent.config().context_window_size;
                if context_size > 0 {
                    let percent =
                        ((total_tokens as f32 / context_size as f32) * 100.0).min(100.0) as u8;
                    self.footer.set_context_percent(percent);
                }

                info!(
                    input = input_tokens,
                    output = output_tokens,
                    total = total_tokens,
                    "Token usage"
                );
            }

            AgentEvent::ToolProgress {
                id,
                message,
                percent,
            } => {
                info!(id = %id, message = %message, percent = ?percent, "Tool progress");
                // Update tool progress in messages component
                self.messages.update_tool_progress(&id, message, percent);
            }

            AgentEvent::Compaction {
                original_count,
                compacted_count,
                tokens_saved,
                messages_summarized,
            } => {
                info!(
                    original = original_count,
                    compacted = compacted_count,
                    tokens_saved = tokens_saved,
                    messages_summarized = messages_summarized,
                    "Context compaction performed"
                );
                // Note: Compaction happens automatically to manage context window
                // The user is informed via the footer context percentage indicator
            }
        }

        false
    }

    async fn handle_key(&mut self, key: crossterm::event::KeyEvent) -> Result<()> {
        match self.mode {
            AppMode::Normal => self.handle_normal_key(key).await,
            AppMode::Input => self.handle_input_key(key).await,
            AppMode::Approval => self.handle_approval_key(key).await,
        }
    }

    async fn handle_approval_key(&mut self, key: crossterm::event::KeyEvent) -> Result<()> {
        match (key.modifiers, key.code) {
            (KeyModifiers::NONE, KeyCode::Up) => {
                if let Some(ref mut dialog) = self.approval_dialog {
                    dialog.select_previous();
                }
            }
            (KeyModifiers::NONE, KeyCode::Down) => {
                if let Some(ref mut dialog) = self.approval_dialog {
                    dialog.select_next();
                }
            }
            (KeyModifiers::NONE, KeyCode::Enter) => {
                self.submit_approval().await;
            }
            (KeyModifiers::NONE, KeyCode::Esc) => {
                // Cancel/deny on escape
                self.deny_approval().await;
            }
            (KeyModifiers::NONE, KeyCode::Char('y')) => {
                // Quick approve
                if let Some(ref mut dialog) = self.approval_dialog {
                    dialog.selected = 0; // Approve
                }
                self.submit_approval().await;
            }
            (KeyModifiers::NONE, KeyCode::Char('n')) => {
                // Quick deny
                self.deny_approval().await;
            }
            (KeyModifiers::NONE, KeyCode::Char('a')) => {
                // Quick always approve
                if let Some(ref mut dialog) = self.approval_dialog {
                    dialog.selected = 2; // Always Approve
                }
                self.submit_approval().await;
            }
            _ => {}
        }
        Ok(())
    }

    async fn submit_approval(&mut self) {
        if let Some(dialog) = self.approval_dialog.take() {
            let request_id = dialog.request_id.clone();
            let response = dialog.get_response();
            let decision = match response {
                ApprovalResponse::Approve => ApprovalDecision::Approve,
                ApprovalResponse::Deny => ApprovalDecision::Deny,
                ApprovalResponse::AlwaysApprove => ApprovalDecision::AlwaysApprove,
            };

            info!(request_id = %request_id, decision = ?decision, "Sending approval decision");

            // Send decision back to agent with the matching request ID
            if let Some(ref tx) = self.approval_dec_tx {
                let _ = tx.send((request_id, decision)).await;
            }

            // Return to input mode
            self.mode = AppMode::Input;
        }
    }

    async fn deny_approval(&mut self) {
        if let Some(dialog) = self.approval_dialog.take() {
            let request_id = dialog.request_id.clone();

            info!(request_id = %request_id, "Denying approval request");

            // Send denial back to agent
            if let Some(ref tx) = self.approval_dec_tx {
                let _ = tx.send((request_id, ApprovalDecision::Deny)).await;
            }
        }
        self.mode = AppMode::Input;
    }

    /// Show an approval dialog for a tool execution request.
    fn show_approval_dialog(&mut self, request: ApprovalRequest) {
        info!(tool = %request.tool, request_id = %request.id, "Showing approval dialog");

        self.approval_dialog = Some(ApprovalDialog::with_id(
            request.id,
            &request.tool,
            &request.input,
            &request.reason,
        ));
        self.mode = AppMode::Approval;
    }

    async fn handle_normal_key(&mut self, key: crossterm::event::KeyEvent) -> Result<()> {
        match (key.modifiers, key.code) {
            (KeyModifiers::NONE, KeyCode::Char('q')) => {
                self.should_quit = true;
            }
            (KeyModifiers::CONTROL, KeyCode::Char('d')) => {
                self.should_quit = true;
            }
            (KeyModifiers::NONE, KeyCode::Char('i')) => {
                self.mode = AppMode::Input;
            }
            (KeyModifiers::CONTROL, KeyCode::Char('t')) => {
                next_theme();
                let theme_name = get_theme().name();
                self.messages.update_scrollbar_colors();
                info!("Switched to theme: {}", theme_name);
            }
            (KeyModifiers::CONTROL, KeyCode::Char('k')) => {
                // Manual context compaction
                self.start_compaction();
            }
            (KeyModifiers::NONE, KeyCode::Esc) => {
                self.sidebar = None;
                self.messages.collapse_all_tools();
            }
            (KeyModifiers::NONE, KeyCode::Up) => {
                if let Some(Sidebar::Files(ref mut sidebar)) = self.sidebar {
                    sidebar.select_up();
                }
            }
            (KeyModifiers::NONE, KeyCode::Down) => {
                if let Some(Sidebar::Files(ref mut sidebar)) = self.sidebar {
                    let visible_count = self.sidebar_area.height.saturating_sub(2) as usize;
                    sidebar.select_down(visible_count);
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Start manual context compaction as a background task (gemini-cli style).
    /// Returns immediately after setting pending state, allowing UI to animate.
    fn start_compaction(&mut self) {
        use crate::agent::compaction::ContextCompactor;

        // Don't start if already compacting
        if self.compaction_result_rx.is_some() {
            self.footer.set_status_message("Already compacting...");
            return;
        }

        let config = self.agent.config();
        let compaction_config = config.to_compaction_config();
        let preserve_recent = compaction_config.preserve_recent;
        let context_window_size = config.context_window_size;
        let history = self.agent.history();
        let client = self.agent.client();

        // Get current history state (synchronously for UI feedback)
        let history_arc = history.clone();
        let (current_count, is_short_history) = tokio::task::block_in_place(|| {
            let rt = tokio::runtime::Handle::current();
            rt.block_on(async {
                let guard = history_arc.read().await;
                let count = guard.len();
                (count, count <= preserve_recent)
            })
        });

        info!(messages = current_count, "Manual compaction triggered");

        // Warn if history is short, but still allow compaction
        if is_short_history {
            warn!(
                messages = current_count,
                preserve_recent = preserve_recent,
                "Compacting short history"
            );
            self.footer.set_status_message(format!(
                "⚠ Warning: Only {} messages (recommended: {}+)",
                current_count, preserve_recent
            ));
        }

        // Start pending compression animation immediately (gemini-cli style)
        self.messages.start_compression();

        // Create channel for result
        let (tx, rx) = mpsc::channel(1);
        self.compaction_result_rx = Some(rx);

        // Spawn background task for compaction
        let compactor = ContextCompactor::new(compaction_config);
        tokio::spawn(async move {
            let mut history_guard = history.write().await;
            let result = compactor
                .compact(&mut history_guard, &client, context_window_size)
                .await;

            // Send result through channel (ignore error if receiver was dropped)
            let _ = tx.send(result).await;
        });
    }

    /// Poll for compaction result and update UI when complete.
    fn poll_compaction_result(&mut self) {
        if let Some(ref mut rx) = self.compaction_result_rx {
            match rx.try_recv() {
                Ok(Ok(result)) => {
                    info!(
                        original_messages = result.original_count,
                        compacted_messages = result.compacted_count,
                        original_tokens = result.original_tokens,
                        new_tokens = result.new_tokens,
                        tokens_saved = result.tokens_saved,
                        "Manual compaction complete"
                    );

                    // Use actual estimated token counts from compaction
                    let original_tokens = result.original_tokens;
                    let final_tokens = result.new_tokens;

                    // Check if compaction was beneficial
                    if result.tokens_saved > 0 {
                        self.messages
                            .complete_compression(original_tokens, final_tokens);
                    } else {
                        self.messages
                            .complete_compression_not_beneficial(original_tokens);
                    }

                    self.compaction_result_rx = None;
                }
                Ok(Err(e)) => {
                    info!(error = %e, "Manual compaction failed");
                    self.messages.complete_compression_failed(e.to_string());
                    self.compaction_result_rx = None;
                }
                Err(mpsc::error::TryRecvError::Empty) => {
                    // Still in progress, continue animating
                }
                Err(mpsc::error::TryRecvError::Disconnected) => {
                    // Task failed without sending result
                    self.messages
                        .complete_compression_failed("Compaction task crashed".to_string());
                    self.compaction_result_rx = None;
                }
            }
        }
    }

    async fn handle_input_key(&mut self, key: crossterm::event::KeyEvent) -> Result<()> {
        match (key.modifiers, key.code) {
            (KeyModifiers::NONE, KeyCode::Enter) => {
                self.submit_input().await?;
            }
            (KeyModifiers::CONTROL, KeyCode::Enter) => {
                self.input.insert_newline();
            }
            (KeyModifiers::NONE, KeyCode::Esc) => {
                if self.stream_rx.is_some() {
                    self.cancel_stream();
                } else {
                    self.mode = AppMode::Normal;
                    self.sidebar = None;
                    self.messages.collapse_all_tools();
                }
            }
            (KeyModifiers::NONE, KeyCode::Up) => {
                if self.stream_rx.is_none() {
                    self.input.history_up();
                }
            }
            (KeyModifiers::NONE, KeyCode::Down) => {
                if self.stream_rx.is_none() {
                    self.input.history_down();
                }
            }
            // Ctrl+O: Toggle tool expansion
            (KeyModifiers::CONTROL, KeyCode::Char('o')) => {
                self.messages.toggle_tool_expansion();
            }
            // Handle 'B' key with various modifier combinations
            (_, KeyCode::Char('b') | KeyCode::Char('B')) => {
                let mods = key.modifiers;
                // Ctrl+Alt+B or Cmd+Alt+B: Run in background
                if (mods.contains(KeyModifiers::CONTROL) && mods.contains(KeyModifiers::ALT))
                    || (mods.contains(KeyModifiers::SUPER) && mods.contains(KeyModifiers::ALT))
                {
                    if self.stream_rx.is_some() {
                        self.run_in_background();
                    }
                }
                // Ctrl+Shift+B or Cmd+Shift+B: Toggle Files sidebar
                else if (mods.contains(KeyModifiers::CONTROL)
                    && mods.contains(KeyModifiers::SHIFT))
                    || (mods.contains(KeyModifiers::SUPER) && mods.contains(KeyModifiers::SHIFT))
                {
                    self.toggle_sidebar(SidebarKind::Files);
                }
                // Ctrl+B or Cmd+B: Toggle Files sidebar (legacy)
                else if mods.contains(KeyModifiers::CONTROL) || mods.contains(KeyModifiers::SUPER)
                {
                    self.toggle_sidebar(SidebarKind::Files);
                }
                // Alt+B: Toggle Help sidebar
                else if mods.contains(KeyModifiers::ALT) {
                    self.toggle_sidebar(SidebarKind::Help);
                }
            }
            (KeyModifiers::ALT, KeyCode::Char('s')) => {
                self.toggle_sidebar(SidebarKind::Skills);
            }
            (KeyModifiers::CONTROL, KeyCode::Char('d')) => {
                self.should_quit = true;
            }
            (KeyModifiers::CONTROL, KeyCode::Char('k')) => {
                // Manual context compaction (also available in input mode)
                self.start_compaction();
            }
            (KeyModifiers::NONE, KeyCode::Char('q')) => {
                if self.input.get_input().trim().is_empty() && self.stream_rx.is_none() {
                    self.should_quit = true;
                } else if self.stream_rx.is_none() {
                    self.input.handle_char('q');
                }
            }
            (KeyModifiers::NONE, KeyCode::Backspace) => {
                if self.stream_rx.is_none() {
                    self.input.handle_backspace();
                }
            }
            (KeyModifiers::NONE, KeyCode::Delete) => {
                if self.stream_rx.is_none() {
                    self.input.handle_delete();
                }
            }
            (KeyModifiers::NONE, KeyCode::Left) => {
                if self.stream_rx.is_none() {
                    self.input.move_cursor_left();
                }
            }
            (KeyModifiers::NONE, KeyCode::Right) => {
                if self.stream_rx.is_none() {
                    self.input.move_cursor_right();
                }
            }
            (KeyModifiers::NONE, KeyCode::Home) => {
                if self.stream_rx.is_none() {
                    self.input.move_cursor_line_start();
                }
            }
            (KeyModifiers::NONE, KeyCode::End) => {
                if self.stream_rx.is_none() {
                    self.input.move_cursor_line_end();
                }
            }
            (KeyModifiers::SHIFT, KeyCode::Up) => {
                self.messages.scroll_up(1);
            }
            (KeyModifiers::SHIFT, KeyCode::Down) => {
                self.messages.scroll_down(1);
            }
            (KeyModifiers::NONE, KeyCode::PageUp) => {
                self.messages.scroll_page_up();
            }
            (KeyModifiers::NONE, KeyCode::PageDown) => {
                self.messages.scroll_page_down();
            }
            (KeyModifiers::CONTROL, KeyCode::Home) => {
                self.messages.scroll_to_top();
            }
            (KeyModifiers::SHIFT, KeyCode::Home) => {
                self.messages.scroll_to_top();
            }
            (KeyModifiers::CONTROL, KeyCode::End) => {
                self.messages.scroll_to_bottom();
            }
            (KeyModifiers::SHIFT, KeyCode::End) => {
                self.messages.scroll_to_bottom();
            }
            (KeyModifiers::NONE | KeyModifiers::SHIFT, KeyCode::Char(c)) => {
                if self.stream_rx.is_none() {
                    self.input.handle_char(c);
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn cancel_stream(&mut self) {
        if let Some(handle) = self.stream_abort.take() {
            handle.abort();
        }
        self.stream_rx = None;

        if !self.current_text.is_empty() {
            self.messages
                .finalize_assistant(format!("{}\n\n[Cancelled]", self.current_text));
        } else {
            self.messages.add_assistant("[Cancelled]".to_string());
        }

        self.current_text.clear();
        self.loading_indicator
            .set_streaming_state(StreamingState::Idle);
        self.status_bar.stop();
    }

    /// Move the current stream to background mode, allowing user to continue interacting
    fn run_in_background(&mut self) {
        if self.stream_rx.is_some() {
            self.is_background = true;
            self.mode = AppMode::Normal;
            self.footer.set_background(true);
        }
    }

    fn toggle_sidebar(&mut self, kind: SidebarKind) {
        self.sidebar = match (&self.sidebar, kind) {
            (Some(Sidebar::Files(_)), SidebarKind::Files) => None,
            (Some(Sidebar::Help(_)), SidebarKind::Help) => None,
            (Some(Sidebar::Skills(_)), SidebarKind::Skills) => None,
            (_, SidebarKind::Files) => Some(Sidebar::Files(FileSidebar::new(self.workdir.clone()))),
            (_, SidebarKind::Help) => Some(Sidebar::Help(HelpSidebar::new())),
            (_, SidebarKind::Skills) => {
                // Load skills from the skills registry
                let skills = crate::skills::registry::SkillRegistry::load_from_dir(
                    &self.workdir.join(".amadeus/skills"),
                )
                .unwrap_or_default();
                Some(Sidebar::Skills(crate::ui::components::SkillSidebar::new(
                    skills.into_skills(),
                )))
            }
        };
    }

    async fn submit_input(&mut self) -> Result<()> {
        if self.stream_rx.is_some() {
            return Ok(());
        }

        let input = self.input.get_input();
        let trimmed = input.trim();

        if trimmed.is_empty() || trimmed == "q" || trimmed == "exit" {
            if trimmed == "q" || trimmed == "exit" {
                self.should_quit = true;
            }
            self.input.clear();
            return Ok(());
        }

        // --- SLASH COMMANDS ---
        if trimmed.starts_with('/') {
            let command = trimmed.to_lowercase();
            if command == "/compact" || command == "/compress" {
                self.input.clear();
                // Set flag to flush buffer before compaction
                self.flush_before_compaction = true;
                return Ok(());
            }
            // Add other slash commands here in the future
        }

        self.messages.add_user(trimmed.to_string());

        // Store user input for flushing in run_loop
        self.pending_user_input = Some(trimmed.to_string());

        self.input.clear();
        self.current_text.clear();
        self.loading_indicator
            .set_streaming_state(StreamingState::Responding);

        // --- MESH DELEGATION ---
        if let Some(addr) = self.mesh_supervisor_addr.clone() {
            let prompt = trimmed.to_string();
            let (tx, rx) = mpsc::channel(64);

            let handle = tokio::spawn(async move {
                let client = reqwest::Client::new();
                let url = format!("{}/tasks", addr);

                let body = serde_json::json!({
                    "id": format!("tui-{}", uuid::Uuid::new_v4()),
                    "prompt": prompt,
                    "capabilities": ["bash"]
                });

                match client.post(&url).json(&body).send().await {
                    Ok(resp) => {
                        if resp.status().is_success() {
                            if let Ok(result) = resp.json::<serde_json::Value>().await {
                                let text = result["output"].as_str().unwrap_or("Done").to_string();
                                let _ = tx
                                    .send(AgentEvent::Done {
                                        result: crate::agent::events::RunResult {
                                            text,
                                            tool_calls: Vec::new(),
                                        },
                                    })
                                    .await;
                            }
                        } else {
                            let error_body = resp
                                .text()
                                .await
                                .unwrap_or_else(|_| "Unknown error".to_string());
                            let _ = tx
                                .send(AgentEvent::Error {
                                    message: format!("Supervisor error ({}): {}", addr, error_body),
                                })
                                .await;
                        }
                    }
                    Err(e) => {
                        let _ = tx
                            .send(AgentEvent::Error {
                                message: format!("Connection failed to {}: {}", addr, e),
                            })
                            .await;
                    }
                }
            });

            self.stream_rx = Some(rx);
            self.stream_abort = Some(handle);
            return Ok(());
        }

        let agent = self.agent.clone();
        let prompt = trimmed.to_string();

        // Create approval channels for this stream
        let (channels, approval_handle) = create_approval_channels();
        self.approval_dec_tx = Some(approval_handle.decision_tx);
        self.approval_req_rx = Some(approval_handle.request_rx);

        let (tx, rx) = mpsc::channel(64);
        let handle = tokio::spawn(async move {
            // Add to history first
            {
                let history_arc = agent.history();
                let mut history = history_arc.write().await;
                history.push(crate::agent::messages::Message::user(&prompt));
            }

            let mut stream = agent.run_stream_with_approval(Some(channels));
            while let Some(event) = stream.next().await {
                match event {
                    Ok(e) => {
                        let is_done = matches!(e, AgentEvent::Done { .. });
                        if tx.send(e).await.is_err() {
                            break;
                        }
                        if is_done {
                            break;
                        }
                    }
                    Err(e) => {
                        let error_msg = e.to_string();
                        let _ = tx.send(AgentEvent::Error { message: error_msg }).await;
                        break;
                    }
                }
            }
        });

        self.stream_rx = Some(rx);
        self.stream_abort = Some(handle);

        Ok(())
    }

    fn render(&mut self, frame: &mut ratatui::Frame) {
        let size = frame.area();

        // Restore partitioned layout to ensure history and input are rendered correctly
        let input_height = self.input.height();
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0), Constraint::Length(input_height)])
            .split(size);

        let messages_area = layout[0];
        let input_area = layout[1];

        // Render messages (history)
        self.messages.render(frame, messages_area);

        // Render input box at its reserved bottom shelf
        self.input.render(frame, input_area);

        // Render approval dialog on top if active
        if let Some(ref dialog) = self.approval_dialog {
            dialog.render(frame, size);
        }
    }
}
