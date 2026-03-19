use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use crossterm::{
    event::{
        KeyCode, KeyModifiers, KeyboardEnhancementFlags, MouseButton, MouseEventKind,
        PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode},
};
use futures::StreamExt;
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Widget},
    Terminal, TerminalOptions, Viewport,
};
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

#[cfg(feature = "test-utils")]
use crate::test_utils::testflow::types::{TuiCellSnapshot, TuiFrameSnapshot};

use crate::agent::events::{AgentEvent, ApprovalDecision, ApprovalRequest};
use crate::agent::loop_agent::{create_approval_channels, Agent};
use crate::client::LLMClient;
use crate::error::Result;
use crate::ui::components::{
    render_markdown, ApprovalDialog, ApprovalResponse, ContextInfo, ContextSidebar, FileSidebar,
    Footer, HelpSidebar, InputComponent, LoadingIndicator, MessagesComponent, Sidebar, StatusBar,
    StreamingState,
};
use crate::ui::event::{AppEvent, EventHandler};
use crate::ui::{get_theme, next_theme, SidebarKind};

const STREAM_FLUSH_INTERVAL_MS: u64 = 150;
const STREAM_FLUSH_CHAR_THRESHOLD: usize = 32;
const DEFAULT_VIEWPORT_HEIGHT_PERCENT: u16 = 32;
const DEFAULT_SHELF_HEIGHT: u16 = 6;
const MIN_LIVE_VIEWPORT_WIDTH: u16 = 4;
const MIN_LIVE_VIEWPORT_HEIGHT: u16 = 3;
const TOOL_MONITOR_LINES_ENV: &str = "AMADEUS_TOOL_MONITOR_LINES";
const DEFAULT_TOOL_MONITOR_LINES: u16 = 16;
const MIN_TOOL_MONITOR_LINES: u16 = 6;
const MONITOR_NAV_HINT: &str = "^X i prev  ^X k next  ^X j back  ^X l enter";
const KEY_CHORD_SEPARATOR: &str = ", ";
const SUB_AGNET_TOOL_NAME: &str = "sub_agnet";

struct StreamingBuffer {
    text: String,
    last_flush: Instant,
}

impl StreamingBuffer {
    fn new() -> Self {
        Self {
            text: String::new(),
            last_flush: Instant::now(),
        }
    }

    fn push(&mut self, delta: &str) {
        self.text.push_str(delta);
    }

    fn should_flush(&self) -> bool {
        let time_elapsed =
            self.last_flush.elapsed() >= Duration::from_millis(STREAM_FLUSH_INTERVAL_MS);
        let chars_accumulated = self.text.len() >= STREAM_FLUSH_CHAR_THRESHOLD;
        time_elapsed || chars_accumulated
    }

    fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    fn clear(&mut self) {
        self.text.clear();
    }
}

/// A simple stateful filter to remove XML-like tags and their content from a stream
struct TagFilter {
    buffer: String,
    suppressing: bool,
    /// Tags that trigger suppression of content between opening and closing
    tags: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MonitorStatus {
    Pending,
    Success,
    Error,
}

#[derive(Debug, Clone)]
struct ToolMonitorNode {
    name: String,
    parent_id: Option<String>,
    input: String,
    output: String,
    status: MonitorStatus,
    progress_message: Option<String>,
    progress_percent: Option<u8>,
    children: Vec<String>,
}

impl ToolMonitorNode {
    fn new(_id: String, name: String, parent_id: Option<String>) -> Self {
        Self {
            name,
            parent_id,
            input: String::new(),
            output: String::new(),
            status: MonitorStatus::Pending,
            progress_message: None,
            progress_percent: None,
            children: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
struct ToolActivitySnapshot {
    tool_name: String,
    progress_message: Option<String>,
    progress_percent: Option<u8>,
    running_count: usize,
}

#[derive(Debug, Default)]
struct ToolMonitorState {
    nodes: HashMap<String, ToolMonitorNode>,
    root_ids: Vec<String>,
    selected_id: Option<String>,
    current_parent: Option<String>,
}

impl ToolMonitorState {
    fn new(preferred_lines: u16) -> Self {
        let _ = preferred_lines;
        Self::default()
    }

    fn clear(&mut self) {
        self.nodes.clear();
        self.root_ids.clear();
        self.selected_id = None;
        self.current_parent = None;
    }

    fn has_content(&self) -> bool {
        !self.nodes.is_empty()
    }

    fn has_running_tools(&self) -> bool {
        self.nodes
            .values()
            .any(|node| node.status == MonitorStatus::Pending)
    }

    fn clear_if_idle(&mut self) -> bool {
        if self.has_running_tools() {
            return false;
        }

        self.clear();
        true
    }

    fn start_tool(&mut self, id: String, name: String, parent_id: Option<String>) {
        let parent_for_node = parent_id.clone();
        self.nodes
            .entry(id.clone())
            .or_insert_with(|| ToolMonitorNode::new(id.clone(), name, parent_for_node));

        if let Some(parent_id) = parent_id {
            if let Some(parent) = self.nodes.get_mut(&parent_id) {
                if !parent.children.iter().any(|child| child == &id) {
                    parent.children.push(id.clone());
                }
            }
        } else if !self.root_ids.iter().any(|root| root == &id) {
            self.root_ids.push(id.clone());
        }

        if self.selected_id.is_none() {
            self.current_parent = self.nodes.get(&id).and_then(|node| node.parent_id.clone());
            self.selected_id = Some(id);
        }

        self.ensure_selection_valid();
    }

    fn append_input(&mut self, id: &str, delta: &str) {
        if let Some(node) = self.nodes.get_mut(id) {
            node.input.push_str(delta);
        }
    }

    fn append_output(&mut self, id: &str, delta: &str) {
        if let Some(node) = self.nodes.get_mut(id) {
            node.output.push_str(delta);
        }
    }

    fn update_progress(&mut self, id: &str, message: String, percent: Option<u8>) {
        if let Some(node) = self.nodes.get_mut(id) {
            node.progress_message = Some(message);
            node.progress_percent = percent;
        }
    }

    fn complete(&mut self, id: &str, output: String, is_error: bool) {
        if let Some(node) = self.nodes.get_mut(id) {
            node.output = output;
            node.status = if is_error {
                MonitorStatus::Error
            } else {
                MonitorStatus::Success
            };
            node.progress_message = None;
            node.progress_percent = None;
        }
        self.ensure_selection_valid();
    }

    fn selected_node(&self) -> Option<&ToolMonitorNode> {
        self.selected_id
            .as_ref()
            .and_then(|selected| self.nodes.get(selected))
    }

    fn current_sibling_ids(&self) -> Vec<String> {
        if let Some(parent_id) = &self.current_parent {
            self.nodes
                .get(parent_id)
                .map(|node| node.children.clone())
                .unwrap_or_default()
        } else {
            self.root_ids.clone()
        }
    }

    fn ensure_selection_valid(&mut self) {
        let siblings = self.current_sibling_ids();
        if siblings.is_empty() {
            self.selected_id = None;
            if self.current_parent.is_some() {
                self.current_parent = None;
                self.ensure_selection_valid();
            }
            return;
        }

        let selected_is_valid = self
            .selected_id
            .as_ref()
            .map(|selected| siblings.iter().any(|id| id == selected))
            .unwrap_or(false);

        if !selected_is_valid {
            self.selected_id = siblings.first().cloned();
        }
    }

    fn select_previous(&mut self) {
        self.select_with_offset(-1);
    }

    fn select_next(&mut self) {
        self.select_with_offset(1);
    }

    fn select_with_offset(&mut self, offset: isize) {
        let siblings = self.current_sibling_ids();
        if siblings.is_empty() {
            return;
        }

        let current_index = self
            .selected_id
            .as_ref()
            .and_then(|selected| siblings.iter().position(|id| id == selected))
            .unwrap_or(0) as isize;

        let len = siblings.len() as isize;
        let next_index = (current_index + offset).rem_euclid(len) as usize;
        self.selected_id = siblings.get(next_index).cloned();
    }

    fn enter_selected(&mut self) -> bool {
        let Some(selected_id) = self.selected_id.clone() else {
            return false;
        };

        let Some(selected) = self.nodes.get(&selected_id) else {
            return false;
        };

        if selected.children.is_empty() {
            return false;
        }

        self.current_parent = Some(selected_id);
        self.selected_id = selected.children.first().cloned();
        true
    }

    fn exit_parent(&mut self) -> bool {
        let Some(parent_id) = self.current_parent.clone() else {
            return false;
        };

        let next_parent = self
            .nodes
            .get(&parent_id)
            .and_then(|node| node.parent_id.clone());
        self.current_parent = next_parent;
        self.selected_id = Some(parent_id);
        self.ensure_selection_valid();
        true
    }

    fn running_tool_count(&self) -> usize {
        self.nodes
            .values()
            .filter(|node| node.status == MonitorStatus::Pending)
            .count()
    }

    fn active_snapshot(&self) -> Option<ToolActivitySnapshot> {
        let running_count = self.running_tool_count();
        let node = self
            .selected_node()
            .filter(|node| node.status == MonitorStatus::Pending)
            .or_else(|| {
                self.root_ids
                    .iter()
                    .find_map(|id| self.first_running_node(id))
            })?;

        Some(ToolActivitySnapshot {
            tool_name: node.name.clone(),
            progress_message: node.progress_message.clone(),
            progress_percent: node.progress_percent,
            running_count,
        })
    }

    fn first_running_node<'a>(&'a self, id: &str) -> Option<&'a ToolMonitorNode> {
        let node = self.nodes.get(id)?;
        if node.status == MonitorStatus::Pending {
            return Some(node);
        }

        node.children
            .iter()
            .find_map(|child_id| self.first_running_node(child_id))
    }
}

impl TagFilter {
    fn new() -> Self {
        Self {
            buffer: String::new(),
            suppressing: false,
            tags: vec![
                "Claude_TalkToUser".to_string(),
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
                            next_char.is_some_and(|c| c.is_alphanumeric() || c == '/');

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
                        if let Some(tag_name) = tag_content.strip_prefix('/') {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppMode {
    Normal,
    Input,
    Approval,
}

#[derive(Debug, Clone)]
enum SessionAction {
    None,
    Done,
    SpawnSubAgent {
        request_id: String,
        prompt: String,
        depth: usize,
    },
}

struct Session<C: LLMClient> {
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
    /// Flag to flush buffer before compaction
    flush_before_compaction: bool,
    /// Filter for removing internal tags
    tag_filter: TagFilter,
    /// Configurable viewport height as percentage of terminal height
    viewport_height_percent: u16,
    current_shelf_height: u16,
    tool_monitor: ToolMonitorState,
    monitor_navigation_prefix: bool,
    key_chord_steps: Vec<String>,
    last_subagent_output: Option<String>,
    last_result_text: Option<String>,
    last_context_sync: Instant,
    #[allow(dead_code)]
    subagent_depth: usize,
    session_label: String,
    session_id: usize,
    pending_approvals: VecDeque<ApprovalRequest>,
    last_error: Option<String>,
    parent_session_id: Option<usize>,
    parent_request_id: Option<String>,
}

impl<C: LLMClient + Clone + 'static> Session<C> {
    fn tool_monitor_line_count() -> u16 {
        std::env::var(TOOL_MONITOR_LINES_ENV)
            .ok()
            .and_then(|value| value.parse::<u16>().ok())
            .filter(|value| *value >= MIN_TOOL_MONITOR_LINES)
            .unwrap_or(DEFAULT_TOOL_MONITOR_LINES)
    }

    fn refresh_key_chord_hint(&mut self) {
        let hint = if self.key_chord_steps.is_empty() {
            None
        } else {
            Some(self.key_chord_steps.join(KEY_CHORD_SEPARATOR))
        };
        self.footer.set_key_chord_hint(hint);
    }

    fn clear_key_chord_hint(&mut self) {
        self.key_chord_steps.clear();
        self.refresh_key_chord_hint();
    }

    fn push_key_chord_step(&mut self, step: String) {
        self.key_chord_steps.push(step);
        self.refresh_key_chord_hint();
    }

    fn key_code_label(code: KeyCode) -> Option<String> {
        match code {
            KeyCode::Backspace => Some("backspace".to_string()),
            KeyCode::Enter => Some("enter".to_string()),
            KeyCode::Left => Some("left".to_string()),
            KeyCode::Right => Some("right".to_string()),
            KeyCode::Up => Some("up".to_string()),
            KeyCode::Down => Some("down".to_string()),
            KeyCode::Home => Some("home".to_string()),
            KeyCode::End => Some("end".to_string()),
            KeyCode::PageUp => Some("pageup".to_string()),
            KeyCode::PageDown => Some("pagedown".to_string()),
            KeyCode::Tab => Some("tab".to_string()),
            KeyCode::BackTab => Some("shift+tab".to_string()),
            KeyCode::Delete => Some("delete".to_string()),
            KeyCode::Insert => Some("insert".to_string()),
            KeyCode::F(number) => Some(format!("f{number}")),
            KeyCode::Char(ch) => Some(ch.to_ascii_lowercase().to_string()),
            KeyCode::Esc => Some("esc".to_string()),
            _ => None,
        }
    }

    fn key_chord_label(key: crossterm::event::KeyEvent) -> Option<String> {
        let code_label = Self::key_code_label(key.code)?;
        let mut parts = Vec::new();

        if key.modifiers.contains(KeyModifiers::CONTROL) {
            parts.push("ctrl".to_string());
        }
        if key.modifiers.contains(KeyModifiers::ALT) {
            parts.push("alt".to_string());
        }
        if key.modifiers.contains(KeyModifiers::SUPER) {
            parts.push("cmd".to_string());
        }
        if key.modifiers.contains(KeyModifiers::SHIFT)
            && !matches!(key.code, KeyCode::Char(_))
            && !matches!(key.code, KeyCode::BackTab)
        {
            parts.push("shift".to_string());
        }

        if parts.is_empty() {
            return Some(code_label);
        }

        parts.push(code_label);
        Some(parts.join("+"))
    }

    fn update_key_chord_hint(&mut self, key: crossterm::event::KeyEvent) {
        match key.code {
            KeyCode::Modifier(_) => {
                let mut labels = Vec::new();
                if key.modifiers.contains(KeyModifiers::CONTROL) {
                    labels.push("ctrl");
                }
                if key.modifiers.contains(KeyModifiers::ALT) {
                    labels.push("alt");
                }
                if key.modifiers.contains(KeyModifiers::SUPER) {
                    labels.push("cmd");
                }
                if key.modifiers.contains(KeyModifiers::SHIFT) {
                    labels.push("shift");
                }

                if labels.is_empty() {
                    self.clear_key_chord_hint();
                } else {
                    self.key_chord_steps = vec![labels.join("+")];
                    self.refresh_key_chord_hint();
                }
            }
            _ => {
                if let Some(step) = Self::key_chord_label(key) {
                    if key.modifiers == KeyModifiers::NONE {
                        if self.monitor_navigation_prefix {
                            self.push_key_chord_step(step);
                        } else {
                            self.key_chord_steps = vec![step];
                            self.refresh_key_chord_hint();
                        }
                    } else {
                        self.key_chord_steps = vec![step];
                        self.refresh_key_chord_hint();
                    }
                } else {
                    self.clear_key_chord_hint();
                }
            }
        }
    }

    fn resolve_done_text(
        result_text: String,
        last_subagent_output: &mut Option<String>,
    ) -> Option<String> {
        if !result_text.trim().is_empty() {
            *last_subagent_output = None;
            return Some(result_text);
        }

        last_subagent_output
            .take()
            .filter(|text| !text.trim().is_empty())
    }

    pub fn new(
        agent: Agent<C>,
        workdir: PathBuf,
        model_name: String,
        subagent_depth: usize,
        session_label: String,
        session_id: usize,
    ) -> Self {
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
            flush_before_compaction: false,
            tag_filter: TagFilter::new(),
            viewport_height_percent: DEFAULT_VIEWPORT_HEIGHT_PERCENT,
            current_shelf_height: DEFAULT_SHELF_HEIGHT,
            tool_monitor: ToolMonitorState::new(Self::tool_monitor_line_count()),
            monitor_navigation_prefix: false,
            key_chord_steps: Vec::new(),
            last_subagent_output: None,
            last_result_text: None,
            last_context_sync: Instant::now(),
            subagent_depth,
            session_label,
            session_id,
            pending_approvals: VecDeque::new(),
            last_error: None,
            parent_session_id: None,
            parent_request_id: None,
        }
    }

    pub fn set_mesh_mode(&mut self, addr: &str) {
        self.mesh_supervisor_addr = Some(addr.to_string());
        self.footer.set_mesh(true);
    }

    #[allow(dead_code)]
    pub async fn run(&mut self) -> Result<()> {
        enable_raw_mode()?;
        let mut stdout = std::io::stdout();
        let _ = execute!(
            stdout,
            PushKeyboardEnhancementFlags(
                KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
                    | KeyboardEnhancementFlags::REPORT_ALL_KEYS_AS_ESCAPE_CODES
                    | KeyboardEnhancementFlags::REPORT_EVENT_TYPES
            )
        );

        let res = self.run_loop().await;

        let _ = execute!(std::io::stdout(), PopKeyboardEnhancementFlags);
        disable_raw_mode()?;
        // Use a temporary terminal to show cursor if needed
        let backend = CrosstermBackend::new(std::io::stdout());
        let mut terminal = Terminal::new(backend)?;
        terminal.show_cursor()?;

        res
    }

    fn handle_tool_progress_timeout(&mut self) {
        if self.stream_rx.is_some() && self.tool_monitor.has_running_tools() {
            self.loading_indicator
                .set_streaming_state(StreamingState::Responding);
        }
    }

    fn find_flushable_index(&self, text: &str) -> Option<usize> {
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

    fn max_shelf_height_for_terminal(&self, terminal_height: u16) -> u16 {
        let input_max = 12;
        let available = terminal_height.saturating_sub(input_max).saturating_sub(2);
        let live_max = ((available.saturating_mul(self.viewport_height_percent)) / 100).max(3);
        (input_max + live_max).min(terminal_height.max(4))
    }

    #[allow(dead_code)]
    async fn run_loop(&mut self) -> Result<()> {
        let terminal_size = crossterm::terminal::size()?;
        self.current_shelf_height = self.max_shelf_height_for_terminal(terminal_size.1);

        let stdout = std::io::stdout();
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::with_options(
            backend,
            TerminalOptions {
                viewport: Viewport::Inline(self.current_shelf_height),
            },
        )?;

        let mut events = EventHandler::new(Duration::from_millis(100));
        loop {
            if self.should_quit {
                break;
            }

            self.sync_inline_viewport(&mut terminal)?;

            if self.messages.should_render_dashboard_to_history() {
                let width = terminal.size()?.width;
                let dashboard_lines = self.messages.render_dashboard_lines(width);
                self.insert_lines_before(&mut terminal, dashboard_lines)?;
                self.messages.mark_dashboard_rendered();
            }

            self.flush_unrendered_history(&mut terminal)?;

            terminal.draw(|f| self.render(f))?;

            // Periodic flush for responsiveness
            if !self.streaming_buffer.is_empty() && self.streaming_buffer.should_flush() {
                self.flush_streaming_buffer(&mut terminal)?;
            }

            tokio::select! {
                event = events.next() => {
                    self.handle_event(event?, &mut terminal).await?;
                    self.flush_unrendered_history(&mut terminal)?;

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
                        if matches!(
                            self.handle_agent_event(event, &mut terminal, true)?,
                            SessionAction::Done
                        ) {
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
                            if matches!(
                                self.handle_agent_event(event, &mut terminal, true)?,
                                SessionAction::Done
                            ) {
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

                        // Force immediate redraw to show streaming progress
                        self.flush_unrendered_history(&mut terminal)?;
                        self.sync_inline_viewport(&mut terminal)?;
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
            self.messages.clear_streaming_text();
            return Ok(());
        }

        while let Some(flush_idx) = self.find_flushable_index(&self.streaming_buffer.text) {
            let flushed = self.streaming_buffer.text[..flush_idx].to_string();
            let remaining = self.streaming_buffer.text[flush_idx..].to_string();

            if flushed.is_empty() {
                break;
            }

            self.insert_assistant_chunk_before(terminal, &flushed)?;
            self.streaming_buffer.text = remaining;
            self.messages
                .update_streaming_text(&self.streaming_buffer.text);
            self.sync_inline_viewport(terminal)?;
        }

        self.streaming_buffer.last_flush = Instant::now();
        Ok(())
    }

    fn finalize_streaming_state(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    ) -> Result<()> {
        let remainder = self.tag_filter.finalize();
        if !remainder.is_empty() {
            self.streaming_buffer.push(&remainder);
            self.current_text.push_str(&remainder);
        }

        if !self.streaming_buffer.is_empty() {
            let tail = self.streaming_buffer.text.clone();
            self.insert_assistant_chunk_before(terminal, &tail)?;
        }

        self.streaming_buffer.clear();
        self.messages.clear_streaming_text();
        self.sync_inline_viewport(terminal)?;
        Ok(())
    }

    fn finalize_streaming_state_background(&mut self) {
        let remainder = self.tag_filter.finalize();
        if !remainder.is_empty() {
            self.streaming_buffer.push(&remainder);
            self.current_text.push_str(&remainder);
        }
        self.streaming_buffer.clear();
        self.messages.clear_streaming_text();
    }

    fn live_viewport_height(&self, width: u16, total_height: u16) -> u16 {
        if width < MIN_LIVE_VIEWPORT_WIDTH {
            return 0;
        }

        let max_height = ((total_height.saturating_mul(self.viewport_height_percent)) / 100)
            .max(MIN_LIVE_VIEWPORT_HEIGHT);
        let is_streaming = self.stream_rx.is_some();
        if self.tool_monitor.has_running_tools() {
            return 5.min(max_height).max(MIN_LIVE_VIEWPORT_HEIGHT);
        }

        if self.messages.is_compression_pending() {
            return 5.min(max_height).max(MIN_LIVE_VIEWPORT_HEIGHT);
        }

        if self.streaming_buffer.is_empty() && !is_streaming {
            return 0;
        }

        let inner_width = width.saturating_sub(4) as usize;
        let lines = render_markdown(&self.streaming_buffer.text, inner_width).len() as u16;
        let content_height = lines.max(1);
        (content_height + 2).min(max_height)
    }

    fn handle_monitor_navigation(&mut self, key: crossterm::event::KeyEvent) -> bool {
        if !self.tool_monitor.has_content() {
            self.monitor_navigation_prefix = false;
            return false;
        }

        if self.monitor_navigation_prefix {
            self.monitor_navigation_prefix = false;
            return match key.code {
                KeyCode::Char('i' | 'I') => {
                    self.tool_monitor.select_previous();
                    true
                }
                KeyCode::Char('k' | 'K') => {
                    self.tool_monitor.select_next();
                    true
                }
                KeyCode::Char('l' | 'L') => self.tool_monitor.enter_selected(),
                KeyCode::Char('j' | 'J') => self.tool_monitor.exit_parent(),
                _ => false,
            };
        }

        match (key.modifiers, key.code) {
            (KeyModifiers::CONTROL, KeyCode::Char('x' | 'X')) => {
                self.monitor_navigation_prefix = true;
                self.key_chord_steps = vec!["ctrl+x".to_string()];
                self.refresh_key_chord_hint();
                self.footer
                    .set_status_message(format!("Monitor nav armed: {MONITOR_NAV_HINT}"));
                true
            }
            _ => false,
        }
    }

    fn clear_tool_monitor(&mut self) {
        self.tool_monitor.clear();
        self.monitor_navigation_prefix = false;
        self.clear_key_chord_hint();
        self.sync_activity_chrome();
    }

    fn restore_input_focus(&mut self) {
        self.mode = AppMode::Input;
        self.monitor_navigation_prefix = false;
        self.clear_key_chord_hint();
    }

    fn maybe_clear_tool_monitor(&mut self) {
        if self.tool_monitor.clear_if_idle() {
            self.monitor_navigation_prefix = false;
            self.sync_activity_chrome();
        }
    }

    fn sync_activity_chrome(&mut self) {
        if let Some(snapshot) = self.tool_monitor.active_snapshot() {
            self.loading_indicator.set_activity_context(
                Some(snapshot.tool_name),
                snapshot.progress_message,
                snapshot.progress_percent,
                snapshot.running_count,
            );
        } else {
            self.loading_indicator.clear_activity_context();
        }

        self.sync_prompt_status_hint();
    }

    fn live_title(&self) -> Line<'static> {
        let colors = crate::ui::get_colors();
        Line::from(vec![Span::styled(
            " Live ",
            Style::default()
                .fg(colors.text.accent)
                .add_modifier(Modifier::BOLD),
        )])
    }

    fn monitor_title(&self) -> Line<'static> {
        let colors = crate::ui::get_colors();
        Line::from(vec![Span::styled(
            " Monitor ",
            Style::default()
                .fg(colors.text.accent)
                .add_modifier(Modifier::BOLD),
        )])
    }

    fn render_tool_activity_preview(&self, max_width: usize) -> Vec<Line<'static>> {
        let colors = crate::ui::get_colors();
        let summary = self
            .tool_monitor
            .active_snapshot()
            .map(|snapshot| {
                let mut text = format!(
                    "{} {}",
                    snapshot.tool_name,
                    self.loading_indicator.prompt_hint().unwrap_or_default()
                );
                if let Some(progress) = snapshot.progress_percent {
                    text.push_str(&format!(" • {progress}%"));
                }
                if let Some(message) = snapshot.progress_message {
                    text.push_str(&format!(" • {message}"));
                }
                if snapshot.running_count > 1 {
                    text.push_str(&format!(" • {} active", snapshot.running_count));
                }
                text
            })
            .unwrap_or_else(|| "working".to_string());

        let truncated = if summary.chars().count() > max_width {
            let keep = max_width.saturating_sub(1);
            let trimmed: String = summary.chars().take(keep).collect();
            format!("{trimmed}…")
        } else {
            summary
        };

        let mut lines = vec![Line::from(vec![Span::styled(
            truncated,
            Style::default().fg(colors.text.secondary),
        )])];

        if self.tool_monitor.has_content() && max_width > 20 {
            lines.push(Line::from(vec![Span::styled(
                "ctrl+x then i/k/j/l to navigate tool activity",
                Style::default().fg(colors.ui.comment),
            )]));
        }

        lines
    }

    fn sync_inline_viewport(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    ) -> Result<()> {
        let size = terminal.size()?;
        self.current_shelf_height = self.max_shelf_height_for_terminal(size.height);
        Ok(())
    }

    fn sync_context_percent(&mut self) {
        if self.last_context_sync.elapsed() < Duration::from_secs(1) {
            return;
        }
        self.last_context_sync = Instant::now();

        let config = self.agent.config();
        let context_window_size = config.context_window_size;
        if context_window_size == 0 {
            return;
        }

        let history = self.agent.history();
        let guard = match history.try_read() {
            Ok(g) => g,
            Err(_) => return,
        };

        let system_tokens = config.system_prompt(false).len().div_ceil(4);

        let registry = self.agent.registry();
        let mut tools_tokens: usize = 0;
        for name in registry.names() {
            let schema_bytes = registry
                .get(name)
                .map(|tool| serde_json::to_string(tool.schema()).unwrap_or_default().len())
                .unwrap_or(0);
            tools_tokens += schema_bytes.div_ceil(4);
        }

        let conv_tokens: usize = guard
            .iter()
            .map(|msg| {
                msg.content
                    .iter()
                    .map(|block| match block {
                        crate::agent::messages::ContentBlock::Text { text } => text.len(),
                        crate::agent::messages::ContentBlock::ToolUse { name, input, .. } => {
                            name.len() + input.to_string().len()
                        }
                        crate::agent::messages::ContentBlock::ToolResult { content, .. } => {
                            content.len()
                        }
                    })
                    .sum::<usize>()
            })
            .sum::<usize>()
            .div_ceil(4);

        let total = system_tokens + tools_tokens + conv_tokens;
        let percent = ((total as f32 / context_window_size as f32) * 100.0).min(100.0) as u8;
        self.footer.set_context_percent(percent);
    }

    fn sync_prompt_status_hint(&mut self) {
        if self.is_background {
            self.input
                .set_status_hint(Some("background task running".to_string()));
            return;
        }

        self.input
            .set_status_hint(self.loading_indicator.prompt_hint());
    }

    fn flush_unrendered_history(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    ) -> Result<()> {
        let width = terminal.size()?.width;
        let lines = self.messages.take_unrendered_lines(width);
        self.insert_lines_before(terminal, lines)
    }

    fn flush_completed_tool_group(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
        render_to_terminal: bool,
    ) -> Result<()> {
        if self.messages.flush_completed_pending_tool_group() && render_to_terminal {
            self.flush_unrendered_history(terminal)?;
        }
        Ok(())
    }

    fn insert_lines_before(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
        lines: Vec<Line<'static>>,
    ) -> Result<()> {
        if lines.is_empty() {
            return Ok(());
        }

        let height = lines.len() as u16;
        terminal.insert_before(height, move |buf| {
            Paragraph::new(lines).render(buf.area, buf);
        })?;
        Ok(())
    }

    fn insert_assistant_chunk_before(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
        chunk: &str,
    ) -> Result<()> {
        if chunk.is_empty() {
            return Ok(());
        }

        let width = terminal.size()?.width;
        let include_prefix = self.messages.should_prefix_current_turn();
        let lines = MessagesComponent::render_assistant_chunk(
            chunk,
            self.messages.current_turn(),
            width,
            include_prefix,
        );
        let mut lines = lines;
        lines.push(Line::from(""));
        self.insert_lines_before(terminal, lines)?;
        self.messages.note_stream_chunk_rendered();
        Ok(())
    }

    fn render_live_viewport(&mut self, frame: &mut ratatui::Frame, area: Rect) {
        if area.width < MIN_LIVE_VIEWPORT_WIDTH || area.height < MIN_LIVE_VIEWPORT_HEIGHT {
            return;
        }

        let colors = crate::ui::get_colors();
        let is_streaming = self.stream_rx.is_some();
        let has_stream_text = !self.streaming_buffer.is_empty();
        let has_tool_activity = self.tool_monitor.has_running_tools();
        let has_pending_compaction = self.messages.is_compression_pending();
        if !has_stream_text && !has_pending_compaction && !has_tool_activity && !is_streaming {
            return;
        }

        let block = Block::default()
            .title(if has_tool_activity {
                self.monitor_title()
            } else {
                self.live_title()
            })
            .borders(Borders::ALL)
            .border_style(Style::default().fg(colors.border.focused));

        let inner = block.inner(area);
        frame.render_widget(block, area);

        if inner.width < 1 || inner.height < 1 {
            return;
        }

        if has_tool_activity && !has_stream_text && !has_pending_compaction {
            let visible_lines: Vec<Line> = self
                .render_tool_activity_preview(inner.width as usize)
                .into_iter()
                .take(inner.height as usize)
                .collect();
            frame.render_widget(
                Paragraph::new(visible_lines).style(Style::default().bg(colors.background.primary)),
                inner,
            );
            return;
        }

        if has_pending_compaction && !has_stream_text {
            let preview_lines = self
                .messages
                .render_pending_compaction_preview(inner.width as usize)
                .unwrap_or_default();
            let visible_lines: Vec<Line> = preview_lines
                .into_iter()
                .take(inner.height as usize)
                .collect();
            frame.render_widget(
                Paragraph::new(visible_lines).style(Style::default().bg(colors.background.primary)),
                inner,
            );
            return;
        }

        if !has_stream_text {
            let hint = self
                .loading_indicator
                .prompt_hint()
                .unwrap_or_else(|| "responding".to_string());
            let line = Line::from(vec![Span::styled(
                hint,
                Style::default().fg(colors.text.secondary),
            )]);
            frame.render_widget(
                Paragraph::new(vec![line]).style(Style::default().bg(colors.background.primary)),
                inner,
            );
            return;
        }

        let rendered_lines = render_markdown(&self.streaming_buffer.text, inner.width as usize);
        let total_lines = rendered_lines.len();
        let max_lines = inner.height as usize;
        let start = total_lines.saturating_sub(max_lines);
        let visible_lines: Vec<Line> = rendered_lines.into_iter().skip(start).collect();

        frame.render_widget(
            Paragraph::new(visible_lines).style(Style::default().bg(colors.background.primary)),
            inner,
        );
    }

    async fn handle_event(
        &mut self,
        event: AppEvent,
        terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    ) -> Result<()> {
        match event {
            AppEvent::Key(key) => self.handle_key(key, terminal).await?,
            AppEvent::Mouse(mouse) => self.handle_mouse(mouse),
            AppEvent::Resize(_, _) => {}
            AppEvent::Tick => {
                self.footer.tick();
                self.loading_indicator.tick();
                self.sync_prompt_status_hint();
                self.messages.tick();
                self.status_bar.tick();
                self.handle_tool_progress_timeout();
                self.sync_context_percent();
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

    fn handle_agent_event(
        &mut self,
        event: AgentEvent,
        terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
        render_to_terminal: bool,
    ) -> Result<SessionAction> {
        match event {
            AgentEvent::TextDelta { delta } => {
                self.flush_completed_tool_group(terminal, render_to_terminal)?;
                debug!(delta_len = delta.len(), delta = %delta, "Received TextDelta");

                // Filter out internal tags
                let filtered_delta = self.tag_filter.process(&delta);
                if filtered_delta.is_empty() {
                    return Ok(SessionAction::None);
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
                self.messages
                    .update_streaming_text(&self.streaming_buffer.text);
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

            AgentEvent::ToolStart {
                id,
                name,
                parent_id,
            } => {
                if render_to_terminal {
                    self.flush_streaming_buffer(terminal)?;
                }
                self.status_bar.set_thinking(true);
                let tool_name = name.clone();
                self.tool_monitor
                    .start_tool(id.clone(), name.clone(), parent_id.clone());
                self.loading_indicator.set_tool_activity_phrase(&tool_name);
                self.sync_activity_chrome();
                if parent_id.is_none() {
                    self.messages.start_tool(id, name, None);
                }
            }

            AgentEvent::ToolComplete {
                id,
                name,
                input,
                output,
                is_error,
                parent_id,
            } => {
                if name == SUB_AGNET_TOOL_NAME && parent_id.is_none() && !output.trim().is_empty() {
                    self.last_subagent_output = Some(output.clone());
                }
                self.tool_monitor.complete(&id, output.clone(), is_error);
                self.maybe_clear_tool_monitor();
                self.sync_activity_chrome();
                let command = if name == "bash" {
                    input
                        .get("command")
                        .and_then(|v: &serde_json::Value| v.as_str())
                        .map(String::from)
                } else {
                    None
                };
                if parent_id.is_none() {
                    self.messages.complete_tool(&id, output, is_error, command);
                    self.flush_completed_tool_group(terminal, render_to_terminal)?;
                }
            }

            AgentEvent::Done { result } => {
                debug!(text_len = result.text.len(), "Agent stream completed");
                let done_text =
                    Self::resolve_done_text(result.text, &mut self.last_subagent_output);
                let done_text_for_record = done_text.clone();

                self.flush_completed_tool_group(terminal, render_to_terminal)?;
                if render_to_terminal {
                    self.finalize_streaming_state(terminal)?;
                } else {
                    self.finalize_streaming_state_background();
                }
                self.clear_tool_monitor();
                if let Some(text) = done_text {
                    self.messages.finalize_assistant(text);
                }
                self.messages.mark_last_item_rendered();
                self.loading_indicator
                    .set_streaming_state(StreamingState::Idle);
                self.current_text.clear();
                self.status_bar.stop();
                self.input.set_status_hint(None);
                self.restore_input_focus();
                if self.is_background {
                    self.is_background = false;
                    self.footer.set_background(false);
                    self.footer.set_status_message("Background task completed");
                }

                self.last_error = None;
                self.last_result_text = done_text_for_record;
                return Ok(SessionAction::Done);
            }

            AgentEvent::Error { message } => {
                self.flush_completed_tool_group(terminal, render_to_terminal)?;
                if render_to_terminal {
                    self.finalize_streaming_state(terminal)?;
                } else {
                    self.finalize_streaming_state_background();
                }
                self.clear_tool_monitor();
                if self.current_text.is_empty() {
                    self.messages.add_assistant(format!("Error: {}", message));
                } else {
                    self.messages
                        .finalize_assistant(format!("{}\n\nError: {}", self.current_text, message));
                }
                self.messages.mark_last_item_rendered();
                self.loading_indicator
                    .set_streaming_state(StreamingState::Idle);
                self.current_text.clear();
                self.status_bar.stop();
                self.input.set_status_hint(None);
                self.restore_input_focus();
                if self.is_background {
                    self.is_background = false;
                    self.footer.set_background(false);
                    self.footer.set_status_message("Background task failed");
                }
                self.last_error = Some(message.clone());
                self.last_result_text = Some(format!("Error: {}", message));
                return Ok(SessionAction::Done);
            }

            AgentEvent::SessionSaved { path } => {
                info!(path = %path, "Session log saved to disk");
            }

            AgentEvent::ToolInputDelta { id, delta, .. } => {
                self.tool_monitor.append_input(&id, &delta);
                self.status_bar.set_thinking(true);
                self.sync_activity_chrome();
            }

            AgentEvent::ToolOutputDelta { id, delta, .. } => {
                self.tool_monitor.append_output(&id, &delta);
                self.status_bar.set_thinking(true);
                self.sync_activity_chrome();
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
                parent_id,
            } => {
                info!(id = %id, message = %message, percent = ?percent, "Tool progress");
                self.tool_monitor
                    .update_progress(&id, message.clone(), percent);
                self.sync_activity_chrome();
                if parent_id.is_none() {
                    self.messages.update_tool_progress(&id, message, percent);
                }
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
            AgentEvent::SubAgentRequested { id, prompt, depth } => {
                self.footer.set_status_message("Spawning sub-agent session");
                self.messages.add_subagent_prompt(prompt.clone(), depth);
                return Ok(SessionAction::SpawnSubAgent {
                    request_id: id,
                    prompt,
                    depth,
                });
            }
        }

        Ok(SessionAction::None)
    }

    async fn handle_key(
        &mut self,
        key: crossterm::event::KeyEvent,
        terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    ) -> Result<()> {
        self.update_key_chord_hint(key);
        match self.mode {
            AppMode::Normal => self.handle_normal_key(key).await,
            AppMode::Input => self.handle_input_key(key, terminal).await,
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
            self.maybe_show_next_approval();
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
        self.maybe_show_next_approval();
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

    fn enqueue_approval_request(&mut self, request: ApprovalRequest, is_active: bool) {
        if self.approval_dialog.is_none() && is_active {
            self.show_approval_dialog(request);
            return;
        }

        self.pending_approvals.push_back(request);
    }

    fn maybe_show_next_approval(&mut self) {
        if self.approval_dialog.is_some() {
            return;
        }
        if let Some(request) = self.pending_approvals.pop_front() {
            self.show_approval_dialog(request);
        }
    }

    fn pending_approval_count(&self) -> usize {
        let mut count = self.pending_approvals.len();
        if self.approval_dialog.is_some() {
            count += 1;
        }
        count
    }

    async fn handle_normal_key(&mut self, key: crossterm::event::KeyEvent) -> Result<()> {
        if self.handle_monitor_navigation(key) {
            return Ok(());
        }

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
            (KeyModifiers::NONE | KeyModifiers::SHIFT, KeyCode::Char(c)) => {
                if self.stream_rx.is_none() {
                    self.restore_input_focus();
                    self.input.handle_char(c);
                }
            }
            (KeyModifiers::NONE, KeyCode::Backspace) => {
                if self.stream_rx.is_none() {
                    self.restore_input_focus();
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

        // Get current history state without blocking the runtime thread.
        let (current_count, is_short_history) = match history.try_read() {
            Ok(guard) => {
                let count = guard.len();
                (count, count <= preserve_recent)
            }
            Err(_) => (0, false),
        };

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

    async fn handle_input_key(
        &mut self,
        key: crossterm::event::KeyEvent,
        terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    ) -> Result<()> {
        if self.handle_monitor_navigation(key) {
            return Ok(());
        }

        match (key.modifiers, key.code) {
            (KeyModifiers::NONE, KeyCode::Enter) => {
                self.submit_input().await?;
            }
            (KeyModifiers::CONTROL, KeyCode::Enter) => {
                self.input.insert_newline();
            }
            (KeyModifiers::NONE, KeyCode::Esc) => {
                if self.stream_rx.is_some() {
                    self.cancel_stream(terminal)?;
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
            (KeyModifiers::CONTROL, KeyCode::Char('o')) => {
                self.messages.toggle_tool_expansion();
            }
            (KeyModifiers::CONTROL | KeyModifiers::SUPER, KeyCode::Char('b' | 'B')) => {
                let mods = key.modifiers;
                if mods.contains(KeyModifiers::ALT) {
                    if self.stream_rx.is_some() {
                        self.run_in_background();
                    }
                } else if self.stream_rx.is_none() {
                    self.input.move_cursor_left();
                }
            }
            (KeyModifiers::CONTROL | KeyModifiers::SUPER, KeyCode::Char('f' | 'F')) => {
                if self.stream_rx.is_none() {
                    self.input.move_cursor_right();
                }
            }
            (KeyModifiers::CONTROL | KeyModifiers::SUPER, KeyCode::Char('p' | 'P')) => {
                if self.stream_rx.is_none() {
                    self.input.history_up();
                }
            }
            (KeyModifiers::CONTROL | KeyModifiers::SUPER, KeyCode::Char('n' | 'N')) => {
                if self.stream_rx.is_none() {
                    self.input.history_down();
                }
            }
            (KeyModifiers::CONTROL, KeyCode::Char('a' | 'A')) => {
                if self.stream_rx.is_none() {
                    self.input.move_cursor_line_start();
                }
            }
            (KeyModifiers::CONTROL, KeyCode::Char('e' | 'E')) => {
                if self.stream_rx.is_none() {
                    self.input.move_cursor_line_end();
                }
            }
            (KeyModifiers::CONTROL, KeyCode::Char('k' | 'K')) => {
                if self.stream_rx.is_none() {
                    self.input.delete_line_by_end();
                }
            }
            (KeyModifiers::CONTROL, KeyCode::Char('u' | 'U')) => {
                if self.stream_rx.is_none() {
                    self.input.delete_line_by_head();
                }
            }
            (KeyModifiers::CONTROL, KeyCode::Char('d' | 'D')) => {
                if self.stream_rx.is_none() {
                    self.input.handle_delete();
                }
            }
            (KeyModifiers::CONTROL, KeyCode::Char('h' | 'H')) => {
                if self.stream_rx.is_none() {
                    self.input.handle_backspace();
                }
            }
            (KeyModifiers::ALT, KeyCode::Char('b' | 'B')) => {
                if self.stream_rx.is_none() {
                    self.input.move_cursor_word_back();
                }
            }
            (KeyModifiers::ALT, KeyCode::Char('f' | 'F')) => {
                if self.stream_rx.is_none() {
                    self.input.move_cursor_word_forward();
                }
            }
            (KeyModifiers::ALT, KeyCode::Char('d' | 'D')) => {
                if self.stream_rx.is_none() {
                    self.input.delete_next_word();
                }
            }
            (KeyModifiers::ALT, KeyCode::Backspace) => {
                if self.stream_rx.is_none() {
                    self.input.delete_word();
                }
            }
            (KeyModifiers::ALT, KeyCode::Char('s')) => {
                self.toggle_sidebar(SidebarKind::Skills);
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

    fn cancel_stream(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    ) -> Result<()> {
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

        self.finalize_streaming_state(terminal)?;
        self.clear_tool_monitor();
        self.messages.mark_last_item_rendered();
        self.current_text.clear();
        self.loading_indicator
            .set_streaming_state(StreamingState::Idle);
        self.status_bar.stop();
        self.input.set_status_hint(None);
        self.restore_input_focus();
        Ok(())
    }

    /// Move the current stream to background mode, allowing user to continue interacting
    fn run_in_background(&mut self) {
        if self.stream_rx.is_some() {
            self.is_background = true;
            self.mode = AppMode::Normal;
            self.footer.set_background(true);
            self.input
                .set_status_hint(Some("background task running".to_string()));
        }
    }

    fn toggle_sidebar(&mut self, kind: SidebarKind) {
        self.sidebar = match (&self.sidebar, kind) {
            (Some(Sidebar::Files(_)), SidebarKind::Files) => None,
            (Some(Sidebar::Help(_)), SidebarKind::Help) => None,
            (Some(Sidebar::Skills(_)), SidebarKind::Skills) => None,
            (Some(Sidebar::Context(_)), SidebarKind::Context) => None,
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
            (_, SidebarKind::Context) => {
                let info = self.build_context_info();
                Some(Sidebar::Context(ContextSidebar::new(info)))
            }
        };
    }

    fn build_context_info(&self) -> ContextInfo {
        let config = self.agent.config();
        let context_window_size = config.context_window_size;
        let model_name = config.model.clone();

        // Estimate system prompt tokens (~4 chars per token)
        let system_prompt = config.system_prompt(false);
        let system_prompt_tokens = system_prompt.len().div_ceil(4);

        // Estimate tool schema tokens
        let registry = self.agent.registry();
        let mut tools_tokens: usize = 0;
        let mut tool_details: Vec<(String, usize)> = Vec::new();
        for name in registry.names() {
            let schema_size = registry
                .get(name)
                .map(|tool| serde_json::to_string(tool.schema()).unwrap_or_default().len())
                .unwrap_or(0);
            let tokens = schema_size.div_ceil(4);
            tool_details.push((name.to_string(), tokens));
            tools_tokens += tokens;
        }
        tool_details.sort_by(|a, b| b.1.cmp(&a.1));

        // Estimate conversation history tokens
        let history = self.agent.history();
        let history_guard = match history.try_read() {
            Ok(guard) => guard,
            Err(_) => return ContextInfo {
                model_name,
                context_window_size,
                total_tokens: system_prompt_tokens + tools_tokens,
                system_prompt_tokens,
                tools_tokens,
                conversation_tokens: 0,
                tool_details,
                message_details: Vec::new(),
            },
        };
        let mut conversation_tokens: usize = 0;
        let mut message_details: Vec<(String, usize)> = Vec::new();
        for msg in history_guard.iter() {
            let msg_chars: usize = msg.content.iter().map(|block| match block {
                crate::agent::messages::ContentBlock::Text { text } => text.len(),
                crate::agent::messages::ContentBlock::ToolUse { name, input, .. } => {
                    name.len() + input.to_string().len()
                }
                crate::agent::messages::ContentBlock::ToolResult { content, .. } => content.len(),
            }).sum();
            let msg_tokens = msg_chars.div_ceil(4);
            conversation_tokens += msg_tokens;
            message_details.push((msg.role.clone(), msg_tokens));
        }
        message_details.sort_by(|a, b| b.1.cmp(&a.1));

        let total_tokens = system_prompt_tokens + tools_tokens + conversation_tokens;

        ContextInfo {
            model_name,
            context_window_size,
            total_tokens,
            system_prompt_tokens,
            tools_tokens,
            conversation_tokens,
            tool_details,
            message_details,
        }
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
            if command == "/context" {
                self.input.clear();
                self.toggle_sidebar(SidebarKind::Context);
                return Ok(());
            }
            // Unknown command: send as user message
        }

        self.messages.add_user(trimmed.to_string());

        self.input.clear();
        self.current_text.clear();
        self.last_error = None;
        self.last_result_text = None;
        self.streaming_buffer.clear();
        self.clear_tool_monitor();
        self.messages.clear_streaming_text();
        self.loading_indicator
            .set_streaming_state(StreamingState::Responding);
        self.sync_activity_chrome();

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

        let input_height = self.input.height();
        let status_height = u16::from(self.status_bar.is_active());
        let footer_height = 2;

        // If a sidebar is open, reserve space on the right.
        // Clamp sidebar width so the main area never collapses below a usable minimum.
        let sidebar_min_width = 30u16;
        let sidebar_max_width = 40u16;
        let sidebar_width = if self.sidebar.is_some() {
            size.width.saturating_sub(sidebar_min_width).min(sidebar_max_width)
        } else {
            0
        };

        let main_width = size.width.saturating_sub(sidebar_width);
        let live_height = self.live_viewport_height(
            main_width,
            size.height
                .saturating_sub(input_height)
                .saturating_sub(status_height)
                .saturating_sub(footer_height),
        );
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(live_height),
                Constraint::Length(input_height),
                Constraint::Length(status_height),
                Constraint::Length(footer_height),
            ])
            .split(Rect {
                width: main_width,
                ..size
            });

        let live_area = layout[0];
        let input_area = layout[1];
        let status_area = layout[2];
        let footer_area = layout[3];

        self.messages_area = Rect::default();

        self.render_live_viewport(frame, live_area);
        self.input.render(frame, input_area);
        self.status_bar.render(frame, status_area);
        self.footer.render(frame, footer_area);

        if let Some(ref dialog) = self.approval_dialog {
            dialog.render(frame, size);
        }

        // Render sidebar if open
        if let Some(ref sidebar) = self.sidebar {
            let sidebar_area = Rect {
                x: size.x + main_width,
                y: size.y,
                width: sidebar_width,
                height: size.height,
            };
            self.sidebar_area = sidebar_area;
            match sidebar {
                Sidebar::Files(s) => s.render(frame, sidebar_area),
                Sidebar::Help(s) => s.render(frame, sidebar_area),
                Sidebar::Skills(s) => s.render(frame, sidebar_area),
                Sidebar::Context(s) => s.render(frame, sidebar_area),
            }
        } else {
            self.sidebar_area = Rect::default();
        }
    }
}

pub struct App<C: LLMClient> {
    sessions: Vec<Session<C>>,
    active_idx: usize,
    should_quit: bool,
    workdir: PathBuf,
    mesh_supervisor_addr: Option<String>,
    model_name: String,
    next_session_id: usize,
    next_sub_label: usize,
    pending_close: Option<(usize, Instant)>,
    #[cfg(feature = "test-utils")]
    recorder: Option<crate::test_utils::SessionRecorder>,
    #[cfg(feature = "test-utils")]
    next_frame_id: u64,
}

impl<C: LLMClient + Clone + 'static> App<C> {
    pub fn new(agent: Agent<C>, workdir: PathBuf, model_name: String) -> Self {
        let mut agent = agent;
        agent.enable_subagent_delegate();
        let session = Session::new(
            agent,
            workdir.clone(),
            model_name.clone(),
            0,
            "root".to_string(),
            0,
        );
        Self {
            sessions: vec![session],
            active_idx: 0,
            should_quit: false,
            workdir,
            mesh_supervisor_addr: None,
            model_name,
            next_session_id: 1,
            next_sub_label: 1,
            pending_close: None,
            #[cfg(feature = "test-utils")]
            recorder: None,
            #[cfg(feature = "test-utils")]
            next_frame_id: 0,
        }
    }

    #[cfg(feature = "test-utils")]
    pub fn set_recorder(&mut self, recorder: crate::test_utils::SessionRecorder) {
        self.recorder = Some(recorder);
    }

    #[cfg(feature = "test-utils")]
    fn record_keyboard(&self, key: crossterm::event::KeyEvent, context: &str) {
        if let Some(ref recorder) = self.recorder {
            let key_str = match key.code {
                KeyCode::Char(c) => c.to_string(),
                KeyCode::Enter => "enter".to_string(),
                KeyCode::Backspace => "backspace".to_string(),
                KeyCode::Esc => "esc".to_string(),
                KeyCode::Tab => "tab".to_string(),
                KeyCode::Up => "up".to_string(),
                KeyCode::Down => "down".to_string(),
                KeyCode::Left => "left".to_string(),
                KeyCode::Right => "right".to_string(),
                KeyCode::F(n) => format!("f{}", n),
                _ => "unknown".to_string(),
            };
            let mut mods = Vec::new();
            if key.modifiers.contains(KeyModifiers::CONTROL) {
                mods.push("ctrl");
            }
            if key.modifiers.contains(KeyModifiers::ALT) {
                mods.push("alt");
            }
            if key.modifiers.contains(KeyModifiers::SHIFT) {
                mods.push("shift");
            }
            if key.modifiers.contains(KeyModifiers::SUPER) {
                mods.push("super");
            }
            let recorder = recorder.clone();
            let context = context.to_string();
            tokio::spawn(async move {
                recorder
                    .record_keyboard_input(&key_str, &mods, &context)
                    .await;
            });
        }
    }

    #[cfg(feature = "test-utils")]
    fn record_agent_event(&self, event: AgentEvent) {
        if let Some(ref recorder) = self.recorder {
            let recorder = recorder.clone();
            tokio::spawn(async move {
                recorder.record_agent_event(event).await;
            });
        }
    }

    #[cfg(feature = "test-utils")]
    fn color_to_string(color: Option<ratatui::style::Color>) -> String {
        match color {
            Some(value) => format!("{value:?}"),
            None => "None".to_string(),
        }
    }

    #[cfg(feature = "test-utils")]
    fn modifier_to_string(modifier: ratatui::style::Modifier) -> String {
        if modifier.is_empty() {
            return "NONE".to_string();
        }
        format!("{modifier:?}")
    }

    #[cfg(feature = "test-utils")]
    fn record_tui_frame_from_buffer(&mut self, area: Rect, buffer: &ratatui::buffer::Buffer) {
        let Some(recorder) = self.recorder.clone() else {
            return;
        };

        let session_id = recorder.session_id();
        let frame_id = self.next_frame_id;
        self.next_frame_id = self.next_frame_id.saturating_add(1);

        let timestamp_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_millis() as u64)
            .unwrap_or(0);

        let mut cells =
            Vec::with_capacity((area.width as usize).saturating_mul(area.height as usize));
        for y in 0..area.height {
            for x in 0..area.width {
                let cell = &buffer[(x, y)];
                let style = cell.style();
                cells.push(TuiCellSnapshot {
                    x,
                    y,
                    symbol: cell.symbol().to_string(),
                    fg: Self::color_to_string(style.fg),
                    bg: Self::color_to_string(style.bg),
                    underline_color: Self::color_to_string(style.underline_color),
                    add_modifier: Self::modifier_to_string(style.add_modifier),
                    sub_modifier: Self::modifier_to_string(style.sub_modifier),
                });
            }
        }

        let snapshot = TuiFrameSnapshot {
            session_id,
            frame_id,
            timestamp_ms,
            width: area.width,
            height: area.height,
            cursor: None,
            cells,
        };

        tokio::spawn(async move {
            if let Err(e) = recorder.record_tui_frame(snapshot).await {
                warn!(error = %e, "Failed to record TUI frame snapshot");
            }
        });
    }

    #[cfg(not(feature = "test-utils"))]
    fn record_tui_frame_from_buffer(&mut self, _area: Rect, _buffer: &ratatui::buffer::Buffer) {}

    #[cfg(not(feature = "test-utils"))]
    fn record_keyboard(&self, _key: crossterm::event::KeyEvent, _context: &str) {}

    #[cfg(not(feature = "test-utils"))]
    fn record_agent_event(&self, _event: AgentEvent) {}

    pub fn set_mesh_mode(&mut self, addr: &str) {
        self.mesh_supervisor_addr = Some(addr.to_string());
        for session in &mut self.sessions {
            session.set_mesh_mode(addr);
        }
    }

    pub async fn run(&mut self) -> Result<()> {
        #[cfg(feature = "test-utils")]
        if let Some(ref recorder) = self.recorder {
            recorder.record_session_start().await;
        }

        enable_raw_mode()?;
        let mut stdout = std::io::stdout();
        let _ = execute!(
            stdout,
            PushKeyboardEnhancementFlags(
                KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
                    | KeyboardEnhancementFlags::REPORT_ALL_KEYS_AS_ESCAPE_CODES
                    | KeyboardEnhancementFlags::REPORT_EVENT_TYPES
            )
        );

        let res = self.run_loop().await;

        let _ = execute!(std::io::stdout(), PopKeyboardEnhancementFlags);
        disable_raw_mode()?;
        let backend = CrosstermBackend::new(std::io::stdout());
        let mut terminal = Terminal::new(backend)?;
        terminal.show_cursor()?;

        #[cfg(feature = "test-utils")]
        if let Some(ref recorder) = self.recorder {
            let (reason, state) = match &res {
                Ok(()) => (
                    crate::test_utils::testflow::types::SessionEndReason::UserExit,
                    crate::test_utils::testflow::types::SessionState::Completed,
                ),
                Err(e) => {
                    recorder.record_error(&e.to_string(), None).await;
                    (
                        crate::test_utils::testflow::types::SessionEndReason::Error,
                        crate::test_utils::testflow::types::SessionState::Failed,
                    )
                }
            };
            recorder.record_session_end(reason, state).await;
            let _ = recorder.save().await;
        }

        res
    }

    fn active_session(&self) -> &Session<C> {
        &self.sessions[self.active_idx]
    }

    fn active_session_mut(&mut self) -> &mut Session<C> {
        &mut self.sessions[self.active_idx]
    }

    fn build_breadcrumb(&self) -> String {
        let mut parts = Vec::new();
        for (idx, session) in self.sessions.iter().enumerate() {
            let mut label = session.session_label.clone();
            let mut marker = String::new();
            if session.stream_rx.is_some() {
                marker.push('*');
            }
            if session.pending_approval_count() > 0 {
                marker.push('?');
            }
            if session.last_error.is_some() {
                marker.push('!');
            }
            if idx == self.active_idx {
                marker.push('>');
            }
            if !marker.is_empty() {
                label.push_str(&marker);
            }
            parts.push(label);
        }
        parts.join(" ▸ ")
    }

    fn update_background_flags(&mut self) {
        for (idx, session) in self.sessions.iter_mut().enumerate() {
            let should_background = idx != self.active_idx && session.stream_rx.is_some();
            if session.is_background != should_background {
                session.is_background = should_background;
                session.footer.set_background(should_background);
            }
        }
    }

    fn switch_session(&mut self, next_idx: usize) {
        if next_idx >= self.sessions.len() || next_idx == self.active_idx {
            return;
        }
        self.active_idx = next_idx;
        let session = self.active_session_mut();
        session.is_background = false;
        session.footer.set_background(false);
        session.maybe_show_next_approval();
    }

    fn handle_global_key(
        &mut self,
        key: crossterm::event::KeyEvent,
        terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    ) -> Result<bool> {
        match (key.modifiers, key.code) {
            (KeyModifiers::CONTROL, KeyCode::Char(']')) => {
                let next_idx = (self.active_idx + 1) % self.sessions.len();
                self.switch_session(next_idx);
                return Ok(true);
            }
            (KeyModifiers::CONTROL, KeyCode::Char('[')) => {
                let next_idx = if self.active_idx == 0 {
                    self.sessions.len().saturating_sub(1)
                } else {
                    self.active_idx - 1
                };
                self.switch_session(next_idx);
                return Ok(true);
            }
            (KeyModifiers::CONTROL, KeyCode::Backspace) => {
                self.close_active_session(terminal)?;
                return Ok(true);
            }
            _ => {}
        }
        Ok(false)
    }

    fn close_active_session(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    ) -> Result<()> {
        if self.sessions.len() <= 1 {
            self.active_session_mut()
                .footer
                .set_status_message("Cannot close root session");
            return Ok(());
        }

        let idx = self.active_idx;
        let session_id = self.sessions[idx].session_id;
        let is_root = self.sessions[idx].parent_session_id.is_none();
        if is_root {
            self.active_session_mut()
                .footer
                .set_status_message("Cannot close root session");
            return Ok(());
        }

        if self.sessions[idx].stream_rx.is_some() {
            let confirmed = matches!(
                self.pending_close,
                Some((pending_id, when))
                    if pending_id == session_id && when.elapsed() < Duration::from_secs(3)
            );

            if !confirmed {
                self.pending_close = Some((session_id, Instant::now()));
                self.active_session_mut()
                    .footer
                    .set_status_message("Press ctrl+backspace again to close running session");
                return Ok(());
            }
        }

        self.pending_close = None;

        let was_running = self.sessions[idx].stream_rx.is_some();
        if was_running {
            self.sessions[idx].cancel_stream(terminal)?;
        }

        if was_running {
            if let (Some(parent_idx), Some(request_id)) = (
                self.sessions[idx].parent_session_id,
                self.sessions[idx].parent_request_id.clone(),
            ) {
                let parent_agent = self.sessions.get(parent_idx).map(|p| p.agent.clone());
                let summary = "Error: sub-agent session closed".to_string();
                if let Some(agent) = parent_agent {
                    tokio::spawn(async move {
                        let _ = agent
                            .complete_subagent(
                                &request_id,
                                crate::agent::loop_agent::SubAgentResult {
                                    output: summary.clone(),
                                    is_error: true,
                                },
                            )
                            .await;
                    });
                }
                if let Some(parent) = self.sessions.get_mut(parent_idx) {
                    parent
                        .messages
                        .add_assistant("[sub-agent] Error: session closed".to_string());
                }
            }
        }

        let parent_idx = self.sessions[idx].parent_session_id.unwrap_or(0);
        self.sessions.remove(idx);

        for session in &mut self.sessions {
            if let Some(parent_id) = session.parent_session_id {
                if parent_id > idx {
                    session.parent_session_id = Some(parent_id - 1);
                }
            }
        }

        let next_idx = if parent_idx >= self.sessions.len() {
            self.sessions.len().saturating_sub(1)
        } else if idx < parent_idx {
            parent_idx.saturating_sub(1)
        } else {
            parent_idx
        };

        self.switch_session(next_idx);
        Ok(())
    }

    fn handle_session_action(&mut self, action: SessionAction, parent_idx: usize) -> Result<()> {
        match action {
            SessionAction::None => {}
            SessionAction::Done => {
                if let Some(session) = self.sessions.get_mut(parent_idx) {
                    session.stream_rx = None;
                    session.stream_abort = None;
                }
            }
            SessionAction::SpawnSubAgent {
                request_id,
                prompt,
                depth,
            } => {
                self.spawn_sub_session(parent_idx, request_id, prompt, depth)?;
            }
        }
        Ok(())
    }

    fn restore_session_channels(
        &mut self,
        session_id: usize,
        stream_rx: Option<mpsc::Receiver<AgentEvent>>,
        approval_rx: Option<mpsc::Receiver<ApprovalRequest>>,
    ) {
        if let Some(idx) = self
            .sessions
            .iter()
            .position(|session| session.session_id == session_id)
        {
            self.sessions[idx].stream_rx = stream_rx;
            self.sessions[idx].approval_req_rx = approval_rx;
        }
    }

    fn spawn_sub_session(
        &mut self,
        parent_idx: usize,
        request_id: String,
        prompt: String,
        depth: usize,
    ) -> Result<()> {
        let parent_agent = self.sessions[parent_idx].agent.clone();
        let mut child_agent = parent_agent.spawn_child_agent(depth);
        child_agent.enable_subagent_delegate();

        let label = format!("sub{}", self.next_sub_label);
        self.next_sub_label += 1;

        let mut session = Session::new(
            child_agent,
            self.workdir.clone(),
            self.model_name.clone(),
            depth,
            label,
            self.next_session_id,
        );
        self.next_session_id += 1;

        session.parent_session_id = Some(parent_idx);
        session.parent_request_id = Some(request_id);

        if let Some(addr) = self.mesh_supervisor_addr.clone() {
            session.set_mesh_mode(&addr);
        }

        // Seed the child session with the prompt as a user message and start streaming.
        session.messages.add_user(prompt.clone());
        session.input.clear();
        session.current_text.clear();
        session.streaming_buffer.clear();
        session.clear_tool_monitor();
        session.messages.clear_streaming_text();
        session
            .loading_indicator
            .set_streaming_state(StreamingState::Responding);
        session.sync_activity_chrome();

        let agent = session.agent.clone();
        let prompt_clone = prompt.clone();

        let (channels, approval_handle) = create_approval_channels();
        session.approval_dec_tx = Some(approval_handle.decision_tx);
        session.approval_req_rx = Some(approval_handle.request_rx);

        let (tx, rx) = mpsc::channel(64);
        let handle = tokio::spawn(async move {
            {
                let history_arc = agent.history();
                let mut history = history_arc.write().await;
                history.push(crate::agent::messages::Message::user(&prompt_clone));
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

        session.stream_rx = Some(rx);
        session.stream_abort = Some(handle);

        self.sessions.push(session);
        self.switch_session(self.sessions.len() - 1);
        Ok(())
    }

    async fn complete_child_session(&mut self, child_idx: usize) {
        let (parent_idx, request_id, summary, is_error) = {
            let child = &mut self.sessions[child_idx];
            let summary = child
                .last_result_text
                .clone()
                .unwrap_or_else(|| "(no summary)".to_string());
            let is_error = child.last_error.is_some();
            (
                child.parent_session_id,
                child.parent_request_id.clone(),
                summary,
                is_error,
            )
        };

        if let (Some(parent_idx), Some(request_id)) = (parent_idx, request_id) {
            let parent_agent = self
                .sessions
                .get(parent_idx)
                .map(|parent| parent.agent.clone());
            if let Some(agent) = parent_agent {
                let _ = agent
                    .complete_subagent(
                        &request_id,
                        crate::agent::loop_agent::SubAgentResult {
                            output: summary.clone(),
                            is_error,
                        },
                    )
                    .await;
            }

            if let Some(parent) = self.sessions.get_mut(parent_idx) {
                parent
                    .messages
                    .add_assistant(format!("[sub-agent] {}", summary.trim_end()));
            }
        }
    }

    async fn poll_background_sessions(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    ) -> Result<()> {
        let mut completed_children = Vec::new();
        for idx in 0..self.sessions.len() {
            if idx == self.active_idx {
                continue;
            }

            let mut drained = 0;
            let mut stream_rx = self.sessions[idx].stream_rx.take();
            if let Some(ref mut rx) = stream_rx {
                while drained < 100 {
                    match rx.try_recv() {
                        Ok(event) => {
                            drained += 1;
                            let action =
                                self.sessions[idx].handle_agent_event(event, terminal, false)?;
                            if matches!(action, SessionAction::Done) {
                                self.sessions[idx].stream_abort = None;
                                completed_children.push(idx);
                                stream_rx = None;
                                break;
                            }
                            if let SessionAction::SpawnSubAgent {
                                request_id,
                                prompt,
                                depth,
                            } = action
                            {
                                self.spawn_sub_session(idx, request_id, prompt, depth)?;
                            }
                        }
                        Err(mpsc::error::TryRecvError::Empty) => break,
                        Err(mpsc::error::TryRecvError::Disconnected) => {
                            self.sessions[idx].stream_abort = None;
                            completed_children.push(idx);
                            stream_rx = None;
                            break;
                        }
                    }
                }
            }
            self.sessions[idx].stream_rx = stream_rx;

            let mut approval_rx = self.sessions[idx].approval_req_rx.take();
            if let Some(ref mut rx) = approval_rx {
                while let Ok(request) = rx.try_recv() {
                    self.sessions[idx].enqueue_approval_request(request, false);
                }
            }
            self.sessions[idx].approval_req_rx = approval_rx;

            self.sessions[idx].poll_compaction_result();
        }

        for idx in completed_children {
            self.complete_child_session(idx).await;
        }

        Ok(())
    }

    async fn run_loop(&mut self) -> Result<()> {
        let terminal_size = crossterm::terminal::size()?;
        let initial_height = self
            .active_session()
            .max_shelf_height_for_terminal(terminal_size.1);

        let stdout = std::io::stdout();
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::with_options(
            backend,
            TerminalOptions {
                viewport: Viewport::Inline(initial_height),
            },
        )?;

        let mut events = EventHandler::new(Duration::from_millis(100));
        loop {
            if self.should_quit {
                break;
            }

            self.update_background_flags();
            if let Some((_, when)) = self.pending_close {
                if when.elapsed() > Duration::from_secs(3) {
                    self.pending_close = None;
                }
            }

            {
                let session = self.active_session_mut();
                session.poll_compaction_result();
                session.sync_inline_viewport(&mut terminal)?;

                if session.messages.should_render_dashboard_to_history() {
                    let width = terminal.size()?.width;
                    let dashboard_lines = session.messages.render_dashboard_lines(width);
                    session.insert_lines_before(&mut terminal, dashboard_lines)?;
                    session.messages.mark_dashboard_rendered();
                }

                session.flush_unrendered_history(&mut terminal)?;
            }

            let completed = terminal.draw(|f| {
                let breadcrumb = self.build_breadcrumb();
                let session = &mut self.sessions[self.active_idx];
                session.footer.set_session_breadcrumb(Some(breadcrumb));
                session.render(f);
            })?;
            self.record_tui_frame_from_buffer(completed.area, completed.buffer);

            {
                let session = self.active_session_mut();
                if !session.streaming_buffer.is_empty() && session.streaming_buffer.should_flush() {
                    session.flush_streaming_buffer(&mut terminal)?;
                }
            }

            self.poll_background_sessions(&mut terminal).await?;

            let active_idx = self.active_idx;
            let active_session_id = self.sessions[active_idx].session_id;
            let mut stream_rx = self.sessions[active_idx].stream_rx.take();
            let mut approval_rx = self.sessions[active_idx].approval_req_rx.take();

            tokio::select! {
                event = events.next() => {
                    let event = event?;
                    let mut handled_global = false;
                    if let AppEvent::Key(key) = event {
                        self.record_keyboard(key, "global");
                        handled_global = self.handle_global_key(key, &mut terminal)?;
                    }
                    if !handled_global {
                        let (should_quit, session_stream_rx, session_approval_rx) = {
                            let session = &mut self.sessions[active_idx];
                            session.handle_event(event, &mut terminal).await?;
                            session.flush_unrendered_history(&mut terminal)?;
                            if session.flush_before_compaction {
                                session.flush_streaming_buffer(&mut terminal)?;
                                session.flush_before_compaction = false;
                                session.start_compaction();
                            }
                            let session_stream_rx = session.stream_rx.take();
                            let session_approval_rx = session.approval_req_rx.take();
                            (
                                session.should_quit,
                                session_stream_rx,
                                session_approval_rx,
                            )
                        };
                        if session_stream_rx.is_some() {
                            stream_rx = session_stream_rx;
                            events.set_tick_rate(Duration::from_millis(16));
                        }
                        if session_approval_rx.is_some() {
                            approval_rx = session_approval_rx;
                        }
                        if should_quit {
                            self.should_quit = true;
                        }
                    }
                }

                agent_event = async {
                    match &mut stream_rx {
                        Some(rx) => rx.recv().await,
                        None => std::future::pending().await,
                    }
                } => {
                    if let Some(event) = agent_event {
                        self.record_agent_event(event.clone());
                        let action = {
                            let session = &mut self.sessions[active_idx];
                            session.handle_agent_event(event, &mut terminal, true)?
                        };
                        let action_for_check = action.clone();
                        self.handle_session_action(action, active_idx)?;

                        if matches!(action_for_check, SessionAction::Done) {
                            if self.sessions[active_idx].parent_request_id.is_some() {
                                self.complete_child_session(active_idx).await;
                            }
                            self.sessions[active_idx].stream_abort = None;
                            stream_rx = None;
                        }

                        if matches!(action_for_check, SessionAction::Done) {
                            events.set_tick_rate(Duration::from_millis(100));
                        }

                        const MAX_BATCH_SIZE: usize = 100;
                        let mut events_to_process: Vec<AgentEvent> = Vec::new();
                        if let Some(ref mut rx) = stream_rx {
                            let mut batch_count = 0;
                            while batch_count < MAX_BATCH_SIZE {
                                match rx.try_recv() {
                                    Ok(event) => {
                                        events_to_process.push(event);
                                        batch_count += 1;
                                    }
                                    Err(mpsc::error::TryRecvError::Empty) |
                                    Err(mpsc::error::TryRecvError::Disconnected) => break,
                                }
                            }
                            if !events_to_process.is_empty() {
                                debug!(batch_size = events_to_process.len(), "Batch processing events");
                            }
                        }

                        for event in events_to_process {
                            let action = {
                                let session = &mut self.sessions[active_idx];
                                session.handle_agent_event(event, &mut terminal, true)?
                            };
                            let action_for_check = action.clone();
                            self.handle_session_action(action, active_idx)?;
                            if matches!(action_for_check, SessionAction::Done) {
                                if self.sessions[active_idx].parent_request_id.is_some() {
                                    self.complete_child_session(active_idx).await;
                                }
                                self.sessions[active_idx].stream_abort = None;
                                stream_rx = None;
                            }
                            if matches!(action_for_check, SessionAction::Done) {
                                events.set_tick_rate(Duration::from_millis(100));
                                break;
                            }
                        }

                        let session = &mut self.sessions[active_idx];
                        if !session.streaming_buffer.is_empty() && session.streaming_buffer.should_flush() {
                            session.flush_streaming_buffer(&mut terminal)?;
                        }

                        session.flush_unrendered_history(&mut terminal)?;
                        session.sync_inline_viewport(&mut terminal)?;
                        let completed = terminal.draw(|f| {
                            let breadcrumb = self.build_breadcrumb();
                            let session = &mut self.sessions[self.active_idx];
                            session.footer.set_session_breadcrumb(Some(breadcrumb));
                            session.render(f);
                        })?;
                        self.record_tui_frame_from_buffer(completed.area, completed.buffer);
                    } else {
                        let session = &mut self.sessions[active_idx];
                        session.stream_abort = None;
                        events.set_tick_rate(Duration::from_millis(100));
                        stream_rx = None;
                    }
                }

                approval_request = async {
                    match &mut approval_rx {
                        Some(rx) => rx.recv().await,
                        None => std::future::pending().await,
                    }
                } => {
                    if let Some(request) = approval_request {
                        let session = &mut self.sessions[active_idx];
                        session.enqueue_approval_request(request, true);
                    }
                }
            }

            self.restore_session_channels(active_session_id, stream_rx, approval_rx);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{App, MonitorStatus, Session, ToolMonitorState};
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use ratatui::{backend::TestBackend, Terminal};
    use std::path::PathBuf;
    use std::sync::Arc;

    use crate::agent::config::Config;
    use crate::agent::loop_agent::Agent;
    use crate::benchmark::case::MockScript;
    use crate::benchmark::mock::BenchmarkMockClient;
    use crate::ui::components::StreamingState;

    fn test_app() -> App<BenchmarkMockClient> {
        let client = BenchmarkMockClient::new(MockScript { steps: Vec::new() });
        let agent = Agent::builder(client, Arc::new(Config::default())).build();
        App::new(agent, PathBuf::from("."), "test-model".to_string())
    }

    fn active_session_mut(app: &mut App<BenchmarkMockClient>) -> &mut Session<BenchmarkMockClient> {
        let idx = app.active_idx;
        &mut app.sessions[idx]
    }

    #[test]
    fn tool_monitor_clears_when_all_tools_complete() {
        let mut monitor = ToolMonitorState::new(16);
        monitor.start_tool("root".to_string(), "bash".to_string(), None);
        monitor.complete("root", "done".to_string(), false);

        assert!(monitor.clear_if_idle());
        assert!(!monitor.has_content());
    }

    #[test]
    fn tool_monitor_stays_visible_while_nested_tool_runs() {
        let mut monitor = ToolMonitorState::new(16);
        monitor.start_tool("root".to_string(), "sub_agnet".to_string(), None);
        monitor.start_tool(
            "root::child".to_string(),
            "bash".to_string(),
            Some("root".to_string()),
        );
        monitor.complete("root", "waiting".to_string(), false);

        assert!(!monitor.clear_if_idle());
        assert!(monitor.has_content());
        assert_eq!(
            monitor.nodes.get("root::child").map(|node| node.status),
            Some(MonitorStatus::Pending)
        );
    }

    #[test]
    fn active_snapshot_prefers_running_selection_then_running_descendant() {
        let mut monitor = ToolMonitorState::new(16);
        monitor.start_tool("root".to_string(), "sub_agnet".to_string(), None);
        monitor.start_tool(
            "root::child".to_string(),
            "bash".to_string(),
            Some("root".to_string()),
        );
        monitor.complete("root", "delegating".to_string(), false);

        let snapshot = monitor.active_snapshot().expect("expected active snapshot");
        assert_eq!(snapshot.tool_name, "bash");
        assert_eq!(snapshot.running_count, 1);
    }

    #[test]
    fn key_chord_label_formats_modifier_shortcuts() {
        assert_eq!(
            Session::<crate::client::openai::OpenAIClient>::key_chord_label(
                crossterm::event::KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL,)
            ),
            Some("ctrl+x".to_string())
        );
        assert_eq!(
            Session::<crate::client::openai::OpenAIClient>::key_chord_label(
                crossterm::event::KeyEvent::new(KeyCode::Char('i'), KeyModifiers::NONE,)
            ),
            Some("i".to_string())
        );
    }

    #[test]
    fn resolved_done_text_uses_subagent_output_when_result_is_empty() {
        let mut last_subagent_output = Some("delegated answer".to_string());
        assert_eq!(
            Session::<crate::client::openai::OpenAIClient>::resolve_done_text(
                String::new(),
                &mut last_subagent_output,
            ),
            Some("delegated answer".to_string())
        );
    }

    #[tokio::test]
    async fn typing_in_normal_mode_restores_input_focus() {
        let mut app = test_app();
        let session = active_session_mut(&mut app);
        session.mode = super::AppMode::Normal;

        session
            .handle_normal_key(KeyEvent::from(KeyCode::Char('h')))
            .await
            .expect("normal key handling should succeed");

        assert_eq!(session.mode, super::AppMode::Input);
        assert_eq!(session.input.get_input(), "h");
    }

    #[tokio::test]
    async fn quit_shortcut_still_works_in_normal_mode() {
        let mut app = test_app();
        let session = active_session_mut(&mut app);
        session.mode = super::AppMode::Normal;

        session
            .handle_normal_key(KeyEvent::from(KeyCode::Char('q')))
            .await
            .expect("normal key handling should succeed");

        assert!(session.should_quit);
        assert!(session.input.get_input().is_empty());
    }

    #[test]
    fn render_includes_footer_and_status_bar() {
        let backend = TestBackend::new(90, 12);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        let mut app = test_app();
        let session = active_session_mut(&mut app);
        session.footer.set_status_message("footer ok");
        session.status_bar.start();
        session.status_bar.update_input_tokens(128);
        session.status_bar.update_text("streamed output");

        terminal
            .draw(|frame| session.render(frame))
            .expect("render should succeed");

        let buffer = terminal.backend().buffer();
        let rendered = (0..buffer.area.height)
            .flat_map(|y| (0..buffer.area.width).map(move |x| buffer[(x, y)].symbol().to_string()))
            .collect::<String>();

        assert!(rendered.contains("footer ok"));
        assert!(rendered.contains("thinking"));
    }

    #[test]
    fn render_shows_pending_compaction_in_live_viewport() {
        let backend = TestBackend::new(90, 12);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        let mut app = test_app();
        let session = active_session_mut(&mut app);
        session.messages.start_compression();
        session.messages.tick();

        terminal
            .draw(|frame| session.render(frame))
            .expect("render should succeed");

        let buffer = terminal.backend().buffer();
        let rendered = (0..buffer.area.height)
            .flat_map(|y| (0..buffer.area.width).map(move |x| buffer[(x, y)].symbol().to_string()))
            .collect::<String>();

        assert!(rendered.contains("Compacting") || rendered.contains("context"));
    }

    #[test]
    fn render_shows_tool_monitor_preview_in_live_viewport() {
        let backend = TestBackend::new(90, 12);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        let mut app = test_app();
        let session = active_session_mut(&mut app);
        session
            .tool_monitor
            .start_tool("tool-1".to_string(), "bash".to_string(), None);
        session
            .tool_monitor
            .update_progress("tool-1", "counting lines".to_string(), Some(42));
        session
            .loading_indicator
            .set_streaming_state(StreamingState::Responding);
        session.sync_activity_chrome();

        terminal
            .draw(|frame| session.render(frame))
            .expect("render should succeed");

        let buffer = terminal.backend().buffer();
        let rendered = (0..buffer.area.height)
            .flat_map(|y| (0..buffer.area.width).map(move |x| buffer[(x, y)].symbol().to_string()))
            .collect::<String>();

        assert!(rendered.contains("Monitor"));
        assert!(rendered.contains("bash"));
    }

    #[test]
    fn monitor_navigation_uses_ctrl_x_prefix_with_plain_followup_keys() {
        fn apply_navigation_step(
            monitor: &mut ToolMonitorState,
            prefix: &mut bool,
            key: KeyEvent,
        ) -> bool {
            if !monitor.has_content() {
                *prefix = false;
                return false;
            }

            if *prefix {
                *prefix = false;
                return match key.code {
                    KeyCode::Char('i' | 'I') => {
                        monitor.select_previous();
                        true
                    }
                    KeyCode::Char('k' | 'K') => {
                        monitor.select_next();
                        true
                    }
                    KeyCode::Char('l' | 'L') => monitor.enter_selected(),
                    KeyCode::Char('j' | 'J') => monitor.exit_parent(),
                    _ => false,
                };
            }

            match (key.modifiers, key.code) {
                (KeyModifiers::CONTROL, KeyCode::Char('x' | 'X')) => {
                    *prefix = true;
                    true
                }
                _ => false,
            }
        }

        let mut monitor = ToolMonitorState::new(16);
        let mut prefix = false;
        monitor.start_tool("root-1".to_string(), "bash".to_string(), None);
        monitor.start_tool("root-2".to_string(), "read".to_string(), None);

        assert!(apply_navigation_step(
            &mut monitor,
            &mut prefix,
            KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL),
        ));
        assert!(prefix);
        assert!(apply_navigation_step(
            &mut monitor,
            &mut prefix,
            KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE),
        ));
        assert_eq!(monitor.selected_id.as_deref(), Some("root-2"));
        assert!(!prefix);
    }
}
