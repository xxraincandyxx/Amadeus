// @amadeus-header
// summary: Main TUI application state, rendering, slash-command inspection, and event orchestration.
// layer: ui
// status: active
// feature_flags:
// - test-utils
// - tui
// provides:
// - module: crate::ui::app
// - type: crate::ui::app::AppMode
// - type: crate::ui::app::App
// uses:
// - module: crate::test_utils::testflow::types
// - module: crate::agent::events
// - module: crate::agent::loop_agent
// - module: crate::client::LLMClient
// - module: crate::error::Result
// - module: crate::ui::event
// - module: crate::ui::i18n
// - module: crate::ui
// - cmd: git apply
// - cmd: git diff
// - runtime: tokio async runtime
// invariants:
// - Listed interfaces stay aligned with the implementation in this file.
// - The thought hit area matches the most recently rendered thought panel header.
// side_effects:
// - Configures terminal keyboard and mouse input modes.
// - Runs external commands or subprocesses.
// - Spawns asynchronous tasks.
// - Sends or receives messages across async channels.
// tests:
// - cmd: cargo test -p tui clicking_thought_header_reveals_hidden_content
// - cmd: cargo test -p tui rewind --features test-utils
// - tests/tool_approval_test.rs
// @end-amadeus-header

use std::collections::{HashMap, VecDeque};
use std::io::IsTerminal;
use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, Instant};

use crossterm::{
    event::{
        DisableMouseCapture, EnableMouseCapture, KeyCode, KeyModifiers, KeyboardEnhancementFlags,
        MouseButton, MouseEventKind, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode},
};
use futures::StreamExt;
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Paragraph, Widget},
    Terminal, TerminalOptions, Viewport,
};
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

#[cfg(feature = "test-utils")]
use crate::test_utils::testflow::types::{TuiCellSnapshot, TuiFrameSnapshot};

use crate::agent::events::{AgentEvent, ApprovalDecision, ApprovalRequest};
use crate::agent::loop_agent::{create_approval_channels, Agent, SessionCheckpoint};
use crate::client::LLMClient;
use crate::commands::{
    answer_side_question, build_context_report, SideQuestionOptions, SlashCommand,
};
use crate::error::Result;
use crate::hooks::{HookDescriptor, HookEvent, HookRegistry};
use crate::ui::components::{
    ApprovalDialog, ApprovalResponse, ContextInfo, FileSidebar, Footer, HelpSidebar,
    InputComponent, LoadingIndicator, MessagesComponent, Sidebar, SlashDialog, SlashDialogItem,
    StatusBar, StreamingState,
};
use crate::ui::event::{AppEvent, EventHandler};
use crate::ui::export::{
    build_export as build_export_artifact, default_export_path, write_export, SessionHeader,
};
use crate::ui::{get_theme, next_theme, SidebarKind};
use crate::Language;

const STREAM_FLUSH_INTERVAL_MS: u64 = 150;
const STREAM_FLUSH_CHAR_THRESHOLD: usize = 32;
const DEFAULT_SHELF_HEIGHT: u16 = 6;
const TOOL_MONITOR_LINES_ENV: &str = "AMADEUS_TOOL_MONITOR_LINES";
const DEFAULT_TOOL_MONITOR_LINES: u16 = 16;
const MIN_TOOL_MONITOR_LINES: u16 = 6;
const MONITOR_NAV_HINT: &str = "^X i prev  ^X k next  ^X j back  ^X l enter";
const KEY_CHORD_SEPARATOR: &str = ", ";
const SUB_AGENT_TOOL_NAME: &str = "sub_agent";

/// Convert a character with SHIFT modifier to its shifted counterpart.
/// Handles letters (a-z -> A-Z) and US keyboard shifted punctuation/symbols.
fn apply_shift_modifier(c: char) -> char {
    match c {
        // Letters - use ASCII uppercase
        'a'..='z' => c.to_ascii_uppercase(),
        // Numbers with shift
        '1' => '!',
        '2' => '@',
        '3' => '#',
        '4' => '$',
        '5' => '%',
        '6' => '^',
        '7' => '&',
        '8' => '*',
        '9' => '(',
        '0' => ')',
        // Punctuation
        '`' => '~',
        '-' => '_',
        '=' => '+',
        '[' => '{',
        ']' => '}',
        '\\' => '|',
        ';' => ':',
        '\'' => '"',
        ',' => '<',
        '.' => '>',
        '/' => '?',
        // Already shifted or non-shiftable characters pass through
        _ => c,
    }
}

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
    fn new(
        _id: String,
        name: String,
        parent_id: Option<String>,
        progress_message: Option<String>,
    ) -> Self {
        Self {
            name,
            parent_id,
            input: String::new(),
            output: String::new(),
            status: MonitorStatus::Pending,
            progress_message,
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

    fn start_tool(
        &mut self,
        id: String,
        name: String,
        parent_id: Option<String>,
        progress_message: Option<String>,
    ) {
        let parent_for_node = parent_id.clone();
        self.nodes.entry(id.clone()).or_insert_with(|| {
            ToolMonitorNode::new(id.clone(), name, parent_for_node, progress_message)
        });

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

    fn refresh_bash_command_preview(&mut self, id: &str) {
        let Some(node) = self.nodes.get_mut(id) else {
            return;
        };

        if node.name != "bash" {
            return;
        }

        let Ok(input) = serde_json::from_str::<serde_json::Value>(&node.input) else {
            return;
        };

        if let Some(command) = input.get("command").and_then(|value| value.as_str()) {
            node.progress_message = Some(command.to_string());
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
    SlashDialog,
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

#[derive(Debug, Clone)]
struct RewindCheckpointRecord {
    label: String,
    detail: String,
    checkpoint: SessionCheckpoint,
    code: Option<CodeSnapshot>,
}

#[derive(Debug, Clone)]
struct CodeSnapshot {
    diff: String,
    summary: CodeSnapshotSummary,
}

#[derive(Debug, Clone, Default)]
struct CodeSnapshotSummary {
    files: Vec<String>,
    additions: usize,
    deletions: usize,
}

#[derive(Debug, Clone)]
struct HooksDialogState {
    dialog: SlashDialog,
    events: Vec<HookEvent>,
    descriptors: Vec<HookDescriptor>,
}

#[derive(Debug, Clone)]
struct RewindDialogState {
    dialog: SlashDialog,
    entries: Vec<Option<RewindCheckpointRecord>>,
}

#[derive(Debug, Clone)]
struct RewindConfirmState {
    dialog: SlashDialog,
    entry: RewindCheckpointRecord,
}

#[derive(Debug, Clone)]
struct AgentsDialogState {
    dialog: SlashDialog,
    session_ids: Vec<usize>,
}

#[derive(Debug, Clone)]
enum SlashDialogState {
    Hooks(HooksDialogState),
    Rewind(RewindDialogState),
    RewindConfirm(RewindConfirmState),
    Agents(AgentsDialogState),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AgentDialogAction {
    Open,
    SwitchTo(usize),
}

#[derive(Debug, Clone)]
struct TransientSlashResponse {
    command: String,
    response: String,
}

pub(crate) struct Session<C: LLMClient> {
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
    thought_area: Rect,
    sidebar_area: Rect,
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
    current_shelf_height: u16,
    tool_monitor: ToolMonitorState,
    monitor_navigation_prefix: bool,
    key_chord_steps: Vec<String>,
    last_subagent_output: Option<String>,
    last_result_text: Option<String>,
    last_context_sync: Instant,
    hooks: HookRegistry,
    slash_dialog: Option<SlashDialogState>,
    rewind_checkpoints: Vec<RewindCheckpointRecord>,
    subagent_depth: usize,
    session_label: String,
    session_id: usize,
    pending_new_agent: bool,
    pending_agent_dialog: Option<AgentDialogAction>,
    pending_transcript_reset: bool,
    pending_approvals: VecDeque<ApprovalRequest>,
    last_error: Option<String>,
    parent_session_id: Option<usize>,
    parent_request_id: Option<String>,
    transient_slash_response: Option<TransientSlashResponse>,
    btw_response_rx: Option<mpsc::UnboundedReceiver<Result<(String, String)>>>,
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
        let hooks = HookRegistry::load_for_config(agent.config().as_ref()).unwrap_or_default();
        let mut footer = Footer::new(model_name.clone());
        // Set default agent name for multi-agent indicator
        footer.set_agent_name(Some("main".to_string()));
        let loading_indicator = LoadingIndicator::new();
        let status_bar = StatusBar::new();

        Self {
            agent,
            mode: AppMode::Input,
            messages: MessagesComponent::new(),
            input: InputComponent::new_with_workdir(workdir.clone()),
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
            thought_area: Rect::default(),
            sidebar_area: Rect::default(),
            approval_dialog: None,
            approval_dec_tx: None,
            approval_req_rx: None,
            compaction_result_rx: None,
            is_background: false,
            streaming_buffer: StreamingBuffer::new(),
            flush_before_compaction: false,
            tag_filter: TagFilter::new(),
            current_shelf_height: DEFAULT_SHELF_HEIGHT,
            tool_monitor: ToolMonitorState::new(Self::tool_monitor_line_count()),
            monitor_navigation_prefix: false,
            key_chord_steps: Vec::new(),
            last_subagent_output: None,
            last_result_text: None,
            last_context_sync: Instant::now(),
            hooks,
            slash_dialog: None,
            rewind_checkpoints: Vec::new(),
            subagent_depth,
            session_label,
            session_id,
            pending_new_agent: false,
            pending_agent_dialog: None,
            pending_transcript_reset: false,
            pending_approvals: VecDeque::new(),
            last_error: None,
            parent_session_id: None,
            parent_request_id: None,
            transient_slash_response: None,
            btw_response_rx: None,
        }
    }

    fn poll_btw_response(&mut self) {
        let Some(mut rx) = self.btw_response_rx.take() else {
            return;
        };

        match rx.try_recv() {
            Ok(Ok((command, response))) => {
                self.input.set_btw_dropup(command, response, false);
            }
            Ok(Err(error)) => {
                self.input
                    .set_btw_dropup("/btw", format!("Error: {error}"), true);
            }
            Err(mpsc::error::TryRecvError::Empty) => {
                self.btw_response_rx = Some(rx);
            }
            Err(mpsc::error::TryRecvError::Disconnected) => {
                self.input
                    .set_btw_dropup("/btw", "Error: /btw request ended unexpectedly.", true);
            }
        }
    }

    fn start_btw_request(&mut self, command: String, question: String) {
        let agent = self.agent.clone();
        let in_flight_assistant_text = if self.current_text.trim().is_empty() {
            None
        } else {
            Some(self.current_text.clone())
        };
        let (tx, rx) = mpsc::unbounded_channel();
        self.btw_response_rx = Some(rx);
        self.input
            .set_btw_dropup(command.clone(), "Thinking…", false);

        tokio::spawn(async move {
            let result = answer_side_question(
                &agent,
                &question,
                SideQuestionOptions {
                    in_flight_assistant_text,
                },
            )
            .await
            .map(|response| (command, response));
            let _ = tx.send(result);
        });
    }

    fn clear_transient_slash_response(&mut self) {
        self.transient_slash_response = None;
        self.input.set_placeholder_visible(true);
    }

    fn transient_slash_response_height(&self) -> u16 {
        if self.transient_slash_response.is_some() {
            2
        } else {
            0
        }
    }

    fn render_transient_slash_response(&mut self, frame: &mut ratatui::Frame, area: Rect) {
        let Some(response) = self.transient_slash_response.as_ref() else {
            return;
        };
        if area.width == 0 || area.height == 0 {
            return;
        }

        let colors = crate::ui::get_colors();
        let command = format!("❯ {}", response.command);
        let result = format!("  ⎿  {}", response.response);
        let lines = vec![
            Line::from(Span::styled(
                command,
                Style::default().fg(colors.text.primary),
            )),
            Line::from(Span::styled(
                result,
                Style::default().fg(colors.text.secondary),
            )),
        ];
        frame.render_widget(Paragraph::new(lines), area);
    }

    fn checkpoint_preview(input: &str) -> String {
        let trimmed = input.trim();
        if trimmed.chars().count() <= 48 {
            trimmed.to_string()
        } else {
            let prefix: String = trimmed.chars().take(47).collect();
            format!("{prefix}…")
        }
    }

    fn checkpoint_detail(checkpoint: &SessionCheckpoint) -> String {
        format!(
            "{} messages · {} todos",
            checkpoint.history.len(),
            checkpoint.todos.len()
        )
    }

    fn summarize_code_snapshot(diff: &str) -> CodeSnapshotSummary {
        let mut summary = CodeSnapshotSummary::default();

        for line in diff.lines() {
            if let Some(path) = line.strip_prefix("+++ b/") {
                if path != "/dev/null" {
                    summary.files.push(path.to_string());
                }
            } else if line.starts_with('+') && !line.starts_with("+++") {
                summary.additions += 1;
            } else if line.starts_with('-') && !line.starts_with("---") {
                summary.deletions += 1;
            }
        }

        summary.files.sort();
        summary.files.dedup();
        summary
    }

    fn code_snapshot_detail(snapshot: &Option<CodeSnapshot>) -> Option<String> {
        let snapshot = snapshot.as_ref()?;
        if snapshot.diff.trim().is_empty() {
            return Some("code clean".to_string());
        }

        let file_label = match snapshot.summary.files.as_slice() {
            [] => "workspace".to_string(),
            [file] => file.clone(),
            files => format!("{} files", files.len()),
        };

        Some(format!(
            "code +{} -{} in {}",
            snapshot.summary.additions, snapshot.summary.deletions, file_label
        ))
    }

    fn checkpoint_detail_with_code(
        checkpoint: &SessionCheckpoint,
        code: &Option<CodeSnapshot>,
    ) -> String {
        match Self::code_snapshot_detail(code) {
            Some(code_detail) => {
                format!("{} · {}", Self::checkpoint_detail(checkpoint), code_detail)
            }
            None => Self::checkpoint_detail(checkpoint),
        }
    }

    fn capture_code_snapshot(&self) -> Option<CodeSnapshot> {
        let output = Command::new("git")
            .args(["diff", "--binary", "--no-ext-diff", "--"])
            .current_dir(&self.workdir)
            .output()
            .ok()?;

        if !output.status.success() {
            return None;
        }

        let diff = String::from_utf8(output.stdout).ok()?;
        let summary = Self::summarize_code_snapshot(&diff);
        Some(CodeSnapshot { diff, summary })
    }

    async fn capture_rewind_checkpoint(&mut self, label: String) -> Result<()> {
        let checkpoint = self.agent.checkpoint().await?;
        let code = self.capture_code_snapshot();
        let detail = Self::checkpoint_detail_with_code(&checkpoint, &code);
        self.rewind_checkpoints.push(RewindCheckpointRecord {
            label,
            detail,
            checkpoint,
            code,
        });
        Ok(())
    }

    fn dismiss_slash_dialog(&mut self) {
        self.slash_dialog = None;
        self.mode = AppMode::Input;
    }

    fn show_hooks_dialog(&mut self) {
        self.hooks =
            HookRegistry::load_for_config(self.agent.config().as_ref()).unwrap_or_default();
        let events = vec![
            HookEvent::PreToolUse,
            HookEvent::PostToolUse,
            HookEvent::PostToolUseFailure,
        ];
        let items = events
            .iter()
            .map(|event| {
                let count = self.hooks.descriptors_for_event(*event).len();
                SlashDialogItem::new(
                    format!("{} - {}", event.title(), event.summary()),
                    Some(if count == 1 {
                        "1 hook".to_string()
                    } else {
                        format!("{count} hooks")
                    }),
                )
            })
            .collect::<Vec<_>>();

        self.slash_dialog = Some(SlashDialogState::Hooks(HooksDialogState {
            dialog: SlashDialog::new(
                "Hooks",
                Some(format!("{} hooks", self.hooks.descriptors().len())),
                Vec::new(),
                "Enter to confirm · Esc to cancel",
                items,
            ),
            events,
            descriptors: self.hooks.descriptors().to_vec(),
        }));
        self.mode = AppMode::SlashDialog;
    }

    async fn show_rewind_dialog(&mut self) -> Result<()> {
        let current_checkpoint = self.agent.checkpoint().await?;
        let current_code = self.capture_code_snapshot();
        let mut entries = self
            .rewind_checkpoints
            .iter()
            .cloned()
            .map(Some)
            .collect::<Vec<_>>();
        entries.push(None);

        let items = self
            .rewind_checkpoints
            .iter()
            .map(|entry| SlashDialogItem::new(entry.label.clone(), Some(entry.detail.clone())))
            .chain(std::iter::once(SlashDialogItem::new(
                "(current)",
                Some(Self::checkpoint_detail_with_code(
                    &current_checkpoint,
                    &current_code,
                )),
            )))
            .collect::<Vec<_>>();

        self.slash_dialog = Some(SlashDialogState::Rewind(RewindDialogState {
            dialog: SlashDialog::new(
                "Rewind",
                None,
                vec![
                    "Restore the session to the point before the selected command or prompt."
                        .to_string(),
                ],
                "Enter to continue · Esc to exit",
                items,
            ),
            entries,
        }));
        self.mode = AppMode::SlashDialog;
        Ok(())
    }

    fn show_rewind_confirm_dialog(&mut self, entry: RewindCheckpointRecord) {
        let code_detail = Self::code_snapshot_detail(&entry.code)
            .unwrap_or_else(|| "code snapshot unavailable".to_string());
        let items = vec![
            SlashDialogItem::new("Restore code and conversation", Some(code_detail.clone())),
            SlashDialogItem::new(
                "Restore conversation",
                Some(Self::checkpoint_detail(&entry.checkpoint)),
            ),
            SlashDialogItem::new("Restore code", Some(code_detail)),
            SlashDialogItem::new(
                "Summarize from here",
                Some("not implemented yet".to_string()),
            ),
            SlashDialogItem::new("Never mind", None),
        ];

        self.slash_dialog = Some(SlashDialogState::RewindConfirm(RewindConfirmState {
            dialog: SlashDialog::new(
                "Rewind",
                Some(
                    "Confirm you want to restore to the point before you sent this message:"
                        .to_string(),
                ),
                vec![
                    format!("| {}", entry.label),
                    "The conversation will be forked.".to_string(),
                    "Code restore uses the unstaged git diff snapshot for tracked files."
                        .to_string(),
                ],
                "Enter to confirm · Esc to exit",
                items,
            ),
            entry,
        }));
        self.mode = AppMode::SlashDialog;
    }

    fn render_hook_details(&mut self, event: HookEvent, descriptors: &[HookDescriptor]) {
        let matching = descriptors
            .iter()
            .filter(|descriptor| descriptor.event == event)
            .collect::<Vec<_>>();

        if matching.is_empty() {
            self.messages.add_local_command_result(format!(
                "**{}**\n\nNo hooks configured for this phase.",
                event.title()
            ));
            return;
        }

        let mut content = format!("**{}**\n\n", event.title());
        for descriptor in matching {
            let tools = if descriptor.tools.is_empty() {
                "all tools".to_string()
            } else {
                descriptor.tools.join(", ")
            };
            let source = match descriptor.source {
                crate::hooks::HookSource::Global => "global",
                crate::hooks::HookSource::Workspace => "workspace",
                crate::hooks::HookSource::Local => "local",
                crate::hooks::HookSource::Runtime => "runtime",
            };
            content.push_str(&format!(
                "- `{}` from `{}` for `{}`\n  `{}`\n",
                descriptor.name, source, tools, descriptor.command
            ));
        }
        self.messages.add_local_command_result(content);
    }

    async fn restore_rewind_conversation(&mut self, entry: &RewindCheckpointRecord) -> Result<()> {
        self.agent.restore_checkpoint(&entry.checkpoint).await?;
        self.messages
            .replace_history_from_messages(&entry.checkpoint.history);
        self.messages.reset_scrollback_cursor_for_session_switch();
        self.current_text.clear();
        self.last_result_text = None;
        self.last_error = None;
        self.streaming_buffer.clear();
        self.messages.clear_streaming_text();
        self.clear_tool_monitor();
        self.pending_transcript_reset = true;
        self.footer
            .set_status_message(format!("Rewound to {}", entry.label));
        Ok(())
    }

    fn run_git_apply(&self, args: &[&str], patch: &str) -> Result<()> {
        let mut child = Command::new("git")
            .args(args)
            .current_dir(&self.workdir)
            .stdin(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()?;

        if let Some(stdin) = child.stdin.as_mut() {
            use std::io::Write;
            stdin.write_all(patch.as_bytes())?;
        }

        let output = child.wait_with_output()?;
        if output.status.success() {
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            Err(crate::error::AgentError::Command(if stderr.is_empty() {
                "git apply failed".to_string()
            } else {
                stderr
            }))
        }
    }

    fn restore_rewind_code(&mut self, entry: &RewindCheckpointRecord) -> Result<()> {
        let Some(target) = entry.code.as_ref() else {
            self.footer
                .set_status_message("No code snapshot available for that checkpoint");
            return Ok(());
        };

        let current = self.capture_code_snapshot();
        if let Some(current) = current {
            if !current.diff.trim().is_empty() {
                self.run_git_apply(&["apply", "-R", "--whitespace=nowarn"], &current.diff)?;
            }
        }

        if !target.diff.trim().is_empty() {
            self.run_git_apply(&["apply", "--whitespace=nowarn"], &target.diff)?;
        }

        self.footer
            .set_status_message(format!("Restored code to {}", entry.label));
        Ok(())
    }

    fn apply_pending_transcript_reset(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    ) -> Result<()> {
        if !self.pending_transcript_reset {
            return Ok(());
        }

        let height = terminal.size()?.height as usize;
        self.pending_transcript_reset = false;
        self.messages.reset_scrollback_cursor_for_session_switch();
        if height > 0 {
            self.insert_lines_before(terminal, vec![Line::from(""); height])?;
        }
        Ok(())
    }

    async fn submit_slash_dialog(&mut self) -> Result<()> {
        let dialog = self.slash_dialog.clone();
        match dialog {
            Some(SlashDialogState::Hooks(state)) => {
                if let Some(selected) = state.dialog.selected() {
                    self.render_hook_details(state.events[selected], &state.descriptors);
                }
                self.dismiss_slash_dialog();
            }
            Some(SlashDialogState::Rewind(state)) => {
                if let Some(selected) = state.dialog.selected() {
                    if let Some(Some(entry)) = state.entries.get(selected) {
                        self.show_rewind_confirm_dialog(entry.clone());
                        return Ok(());
                    }
                }
                self.dismiss_slash_dialog();
            }
            Some(SlashDialogState::RewindConfirm(state)) => {
                if let Some(selected) = state.dialog.selected() {
                    match selected {
                        0 => {
                            self.restore_rewind_code(&state.entry)?;
                            self.restore_rewind_conversation(&state.entry).await?;
                        }
                        1 => {
                            self.restore_rewind_conversation(&state.entry).await?;
                        }
                        2 => {
                            self.restore_rewind_code(&state.entry)?;
                        }
                        3 => {
                            self.messages.add_local_command_result(
                                "Summarize-from-here is not implemented yet.".to_string(),
                            );
                        }
                        _ => {}
                    }
                }
                self.dismiss_slash_dialog();
            }
            Some(SlashDialogState::Agents(state)) => {
                if let Some(selected) = state.dialog.selected() {
                    if let Some(session_id) = state.session_ids.get(selected) {
                        self.pending_agent_dialog = Some(AgentDialogAction::SwitchTo(*session_id));
                    }
                }
                self.dismiss_slash_dialog();
            }
            None => {}
        }
        Ok(())
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
        let fixed_chrome = 2;
        (input_max + fixed_chrome).min(terminal_height.max(4))
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

        if !self.streaming_buffer.is_empty() {
            let flushed = std::mem::take(&mut self.streaming_buffer.text);
            self.insert_assistant_chunk_before(terminal, &flushed)?;
            self.messages.update_streaming_text("");
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
                .get(&name)
                .map(|tool| {
                    serde_json::to_string(&tool.provider_definition())
                        .unwrap_or_default()
                        .len()
                })
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

        if self.input.is_shell_mode() {
            return;
        }

        self.input
            .set_status_hint(self.loading_indicator.input_chrome_hint());
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
        if let Err(error) = terminal.insert_before(height, move |buf| {
            Paragraph::new(lines).render(buf.area, buf);
        }) {
            if Self::is_recoverable_terminal_insert_error(&error) {
                warn!(
                    error = %error,
                    "Terminal scrollback insert failed; continuing with live redraw"
                );
                self.footer
                    .set_status_message("Terminal scrollback update skipped; redraw continued");
                return Ok(());
            }
            return Err(error.into());
        }
        Ok(())
    }

    fn is_recoverable_terminal_insert_error(error: &std::io::Error) -> bool {
        let text = error.to_string();
        text.contains("cursor position") && text.contains("could not be read")
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

    async fn handle_event(
        &mut self,
        event: AppEvent,
        terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    ) -> Result<()> {
        match event {
            AppEvent::Key(key) => self.handle_key(key, terminal).await?,
            AppEvent::Paste(text) => self.handle_paste(text).await?,
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
                self.poll_btw_response();
            }
        }
        Ok(())
    }

    async fn handle_paste(&mut self, text: String) -> Result<()> {
        if self.stream_rx.is_some() {
            return Ok(());
        }

        self.restore_input_focus();
        self.input.handle_paste(&text);
        Ok(())
    }

    fn handle_mouse(&mut self, event: crossterm::event::MouseEvent) {
        match event.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                if event.column >= self.thought_area.x
                    && event.column < self.thought_area.x + self.thought_area.width
                    && event.row >= self.thought_area.y
                    && event.row < self.thought_area.y + self.thought_area.height.min(2)
                {
                    self.messages.toggle_active_thinking();
                    return;
                }

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
            MouseEventKind::Drag(MouseButton::Left) => {
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
            MouseEventKind::ScrollDown
                if self.is_mouse_in_messages_area(event.column, event.row) =>
            {
                self.messages.scroll_down(3);
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
                command,
                parent_id,
            } => {
                if render_to_terminal {
                    self.flush_streaming_buffer(terminal)?;
                }
                self.status_bar.set_thinking(true);
                let tool_name = name.clone();
                self.tool_monitor.start_tool(
                    id.clone(),
                    name.clone(),
                    parent_id.clone(),
                    command.clone(),
                );
                self.loading_indicator.set_tool_activity_phrase(&tool_name);
                self.sync_activity_chrome();
                if parent_id.is_none() {
                    self.messages.start_tool(id, name, command);
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
                if name == SUB_AGENT_TOOL_NAME && parent_id.is_none() && !output.trim().is_empty() {
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
                    self.messages
                        .complete_tool_with_input(&id, &name, &input, output, is_error, command);
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
                self.tool_monitor.refresh_bash_command_preview(&id);
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
                status: _,
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
            AppMode::SlashDialog => self.handle_slash_dialog_key(key).await,
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
            (KeyModifiers::NONE, KeyCode::Esc)
            | (KeyModifiers::CONTROL, KeyCode::Char('c' | 'C')) => {
                // Cancel/deny on escape or Ctrl+C
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

    async fn handle_slash_dialog_key(&mut self, key: crossterm::event::KeyEvent) -> Result<()> {
        match (key.modifiers, key.code) {
            (KeyModifiers::NONE, KeyCode::Up) => {
                if let Some(dialog) = &mut self.slash_dialog {
                    match dialog {
                        SlashDialogState::Hooks(state) => state.dialog.select_previous(),
                        SlashDialogState::Rewind(state) => state.dialog.select_previous(),
                        SlashDialogState::RewindConfirm(state) => state.dialog.select_previous(),
                        SlashDialogState::Agents(state) => state.dialog.select_previous(),
                    }
                }
            }
            (KeyModifiers::NONE, KeyCode::Down) => {
                if let Some(dialog) = &mut self.slash_dialog {
                    match dialog {
                        SlashDialogState::Hooks(state) => state.dialog.select_next(),
                        SlashDialogState::Rewind(state) => state.dialog.select_next(),
                        SlashDialogState::RewindConfirm(state) => state.dialog.select_next(),
                        SlashDialogState::Agents(state) => state.dialog.select_next(),
                    }
                }
            }
            (KeyModifiers::NONE, KeyCode::Enter) => {
                self.submit_slash_dialog().await?;
            }
            (KeyModifiers::NONE, KeyCode::Esc)
            | (KeyModifiers::CONTROL, KeyCode::Char('c' | 'C')) => {
                if matches!(self.slash_dialog, Some(SlashDialogState::Hooks(_))) {
                    self.messages
                        .add_local_command_result("Hooks dialog dismissed".to_string());
                }
                self.dismiss_slash_dialog();
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
            (KeyModifiers::NONE, KeyCode::Char('q'))
            | (KeyModifiers::CONTROL, KeyCode::Char('c' | 'C')) => {
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
                    let c = if key.modifiers.contains(KeyModifiers::SHIFT) {
                        apply_shift_modifier(c)
                    } else {
                        c
                    };
                    self.input.handle_char(c);
                }
            }
            (KeyModifiers::NONE, KeyCode::Backspace) if self.stream_rx.is_none() => {
                self.restore_input_focus();
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
        let compactor = ContextCompactor::from_config(compaction_config);
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

        if self.input.is_shortcuts_visible() {
            self.input.show_shortcuts(false);
            return Ok(());
        }

        if self.stream_rx.is_some() {
            return self.handle_streaming_input_key(key, terminal).await;
        }

        match (key.modifiers, key.code) {
            (KeyModifiers::NONE, KeyCode::Enter) => {
                self.submit_input().await?;
            }
            (KeyModifiers::CONTROL, KeyCode::Enter) => {
                self.input.insert_newline();
            }
            (KeyModifiers::CONTROL, KeyCode::Char('c' | 'C')) => {
                if self.stream_rx.is_some() {
                    self.cancel_stream(terminal)?;
                } else if self.input.get_input().trim().is_empty() {
                    self.should_quit = true;
                } else {
                    self.input.clear();
                }
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
            (KeyModifiers::NONE, KeyCode::Tab) => {
                if self.stream_rx.is_none() {
                    if self.input.completion_is_visible() {
                        self.input.apply_completion();
                    } else if self.input.get_input().starts_with('/') {
                        self.input.force_show_completion();
                    }
                }
            }
            // Shift+Tab: Move up in completion list
            (KeyModifiers::SHIFT, KeyCode::Tab) => {
                if self.stream_rx.is_none() && self.input.completion_is_visible() {
                    self.input.completion_select_up();
                }
            }
            // Ctrl+Down: Move down in completion list
            (KeyModifiers::CONTROL, KeyCode::Down) => {
                if self.stream_rx.is_none() && self.input.completion_is_visible() {
                    self.input.completion_select_down();
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
            (KeyModifiers::CONTROL, KeyCode::Char('b' | 'B'))
            | (KeyModifiers::SUPER, KeyCode::Char('b' | 'B')) => {
                if self.stream_rx.is_none() {
                    self.input.move_cursor_left();
                }
            }
            (KeyModifiers::CONTROL, KeyCode::Char('f' | 'F'))
            | (KeyModifiers::SUPER, KeyCode::Char('f' | 'F')) => {
                if self.stream_rx.is_none() {
                    self.input.move_cursor_right();
                }
            }
            (KeyModifiers::CONTROL, KeyCode::Char('p' | 'P'))
            | (KeyModifiers::SUPER, KeyCode::Char('p' | 'P')) => {
                if self.stream_rx.is_none() {
                    self.input.move_cursor_up();
                }
            }
            (KeyModifiers::CONTROL, KeyCode::Char('n' | 'N'))
            | (KeyModifiers::SUPER, KeyCode::Char('n' | 'N')) => {
                if self.stream_rx.is_none() {
                    self.input.move_cursor_down();
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
            (KeyModifiers::NONE | KeyModifiers::SHIFT, KeyCode::Char('?')) => {
                if self.stream_rx.is_none() && self.input.get_input().trim().is_empty() {
                    self.input.show_shortcuts(true);
                } else if self.stream_rx.is_none() {
                    self.input.handle_char('?');
                }
            }
            (KeyModifiers::SHIFT, KeyCode::Char('/')) => {
                if self.stream_rx.is_none() && self.input.get_input().trim().is_empty() {
                    self.input.show_shortcuts(true);
                } else if self.stream_rx.is_none() {
                    self.input.handle_char('?');
                }
            }
            (KeyModifiers::NONE | KeyModifiers::SHIFT, KeyCode::Char(c))
                if self.stream_rx.is_none() =>
            {
                let c = if key.modifiers.contains(KeyModifiers::SHIFT) {
                    apply_shift_modifier(c)
                } else {
                    c
                };
                self.input.handle_char(c);
            }
            _ => {}
        }
        Ok(())
    }

    async fn handle_streaming_input_key(
        &mut self,
        key: crossterm::event::KeyEvent,
        terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    ) -> Result<()> {
        match (key.modifiers, key.code) {
            (KeyModifiers::CONTROL, KeyCode::Char('c' | 'C')) => {
                if self.input.get_input().trim().is_empty() {
                    self.cancel_stream(terminal)?;
                } else {
                    self.input.clear();
                }
            }
            (KeyModifiers::NONE, KeyCode::Esc) => {
                if self.input.get_input().trim().is_empty() {
                    self.cancel_stream(terminal)?;
                } else {
                    self.input.clear();
                }
            }
            (KeyModifiers::NONE, KeyCode::Enter) => {
                if matches!(
                    SlashCommand::parse(self.input.get_input().trim()),
                    Some(SlashCommand::Btw { .. })
                ) {
                    self.submit_input().await?;
                }
            }
            (KeyModifiers::NONE, KeyCode::Tab) => {
                if self.input.completion_is_visible() {
                    self.input.apply_completion();
                } else if self.input.get_input().starts_with('/') {
                    self.input.force_show_completion();
                }
            }
            (KeyModifiers::SHIFT, KeyCode::Tab) => {
                if self.input.completion_is_visible() {
                    self.input.completion_select_up();
                }
            }
            (KeyModifiers::CONTROL, KeyCode::Down) => {
                if self.input.completion_is_visible() {
                    self.input.completion_select_down();
                }
            }
            (KeyModifiers::NONE, KeyCode::Backspace) => {
                self.input.handle_backspace();
            }
            (KeyModifiers::NONE, KeyCode::Delete) => {
                self.input.handle_delete();
            }
            (KeyModifiers::NONE, KeyCode::Left) => {
                self.input.move_cursor_left();
            }
            (KeyModifiers::NONE, KeyCode::Right) => {
                self.input.move_cursor_right();
            }
            (KeyModifiers::NONE, KeyCode::Home) => {
                self.input.move_cursor_line_start();
            }
            (KeyModifiers::NONE, KeyCode::End) => {
                self.input.move_cursor_line_end();
            }
            (KeyModifiers::CONTROL, KeyCode::Char('b' | 'B'))
            | (KeyModifiers::SUPER, KeyCode::Char('b' | 'B')) => {
                self.input.move_cursor_left();
            }
            (KeyModifiers::CONTROL, KeyCode::Char('f' | 'F'))
            | (KeyModifiers::SUPER, KeyCode::Char('f' | 'F')) => {
                self.input.move_cursor_right();
            }
            (KeyModifiers::CONTROL, KeyCode::Char('a' | 'A')) => {
                self.input.move_cursor_line_start();
            }
            (KeyModifiers::CONTROL, KeyCode::Char('e' | 'E')) => {
                self.input.move_cursor_line_end();
            }
            (KeyModifiers::CONTROL, KeyCode::Char('k' | 'K')) => {
                self.input.delete_line_by_end();
            }
            (KeyModifiers::CONTROL, KeyCode::Char('u' | 'U')) => {
                self.input.delete_line_by_head();
            }
            (KeyModifiers::ALT, KeyCode::Char('b' | 'B')) => {
                self.input.move_cursor_word_back();
            }
            (KeyModifiers::ALT, KeyCode::Char('f' | 'F')) => {
                self.input.move_cursor_word_forward();
            }
            (KeyModifiers::ALT, KeyCode::Char('d' | 'D')) => {
                self.input.delete_next_word();
            }
            (KeyModifiers::ALT, KeyCode::Backspace) => {
                self.input.delete_word();
            }
            (KeyModifiers::NONE | KeyModifiers::SHIFT, KeyCode::Char('?')) => {
                self.input.handle_char('?');
            }
            (KeyModifiers::SHIFT, KeyCode::Char('/')) => {
                self.input.handle_char('?');
            }
            (KeyModifiers::NONE | KeyModifiers::SHIFT, KeyCode::Char(c)) => {
                let c = if key.modifiers.contains(KeyModifiers::SHIFT) {
                    apply_shift_modifier(c)
                } else {
                    c
                };
                self.input.handle_char(c);
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

    fn build_context_info(&self) -> ContextInfo {
        build_context_report(&self.agent)
    }

    /// Build an export artifact for the current session and write it to `path`.
    /// When `path` is `None`, the conversation is written to
    /// `<workdir>/.amadeus/exports/conversation-<label>-<id>-<stamp>.md`.
    pub fn export_to_path(
        &self,
        path: Option<&std::path::Path>,
    ) -> std::io::Result<std::path::PathBuf> {
        let header = SessionHeader {
            session_id: self.session_id.to_string(),
            label: self.session_label.clone(),
            parent_session_id: self.parent_session_id.map(|id| id.to_string()),
            workdir: self.workdir.display().to_string(),
            model: self.agent.config().model.clone(),
            subagent_depth: self.subagent_depth,
        };
        let artifact = build_export_artifact(&self.messages, &self.agent, &header);
        let target: std::path::PathBuf = match path {
            Some(p) => p.to_path_buf(),
            None => default_export_path(&self.workdir, &self.session_label, self.session_id),
        };
        write_export(&artifact, &target)
    }

    fn build_tools_info(&self) -> String {
        let registry = self.agent.registry();
        let mut lines = vec![format!(
            "**Active Tool Profile:** `{}`",
            registry.profile().name
        )];
        lines.push(String::new());
        lines.push("| Tool | Pack | Source | Permission | Aliases | Override |".to_string());
        lines.push("| --- | --- | --- | --- | --- | --- |".to_string());
        for tool in registry.inventory() {
            lines.push(format!(
                "| `{}` | `{}` | `{}` | `{}` | {} | {} |",
                tool.name,
                tool.pack,
                tool.source.as_str(),
                tool.required_permission.as_str(),
                if tool.aliases.is_empty() {
                    "-".to_string()
                } else {
                    tool.aliases
                        .iter()
                        .map(|alias| format!("`{alias}`"))
                        .collect::<Vec<_>>()
                        .join(", ")
                },
                if tool.overridden { "yes" } else { "no" }
            ));
        }
        lines.join("\n")
    }

    fn build_prompt_info(&self) -> String {
        let config = self.agent.config();
        let mut lines = vec![format!(
            "**Active Prompt Profile:** `{}`",
            config.prompt_profile_name()
        )];
        if let Some(profile) = config.prompt_profile() {
            lines.push(format!("**Mode:** `{:?}`", profile.mode));
            lines.push(format!(
                "**Project context:** {}",
                if profile.include_project_context {
                    "included"
                } else {
                    "disabled"
                }
            ));
            lines.push(String::new());
            lines.push("**Sections**".to_string());
            if profile.sections.is_empty() {
                lines.push("- none".to_string());
            } else {
                for section in &profile.sections {
                    lines.push(format!(
                        "- `{}`{}",
                        section.id,
                        section
                            .title
                            .as_ref()
                            .map(|title| format!(": {title}"))
                            .unwrap_or_default()
                    ));
                }
            }
            lines.push(String::new());
            lines.push("**Files**".to_string());
            if profile.files.is_empty() {
                lines.push("- none".to_string());
            } else {
                for file in &profile.files {
                    lines.push(format!("- `{}`", file.display()));
                }
            }
        } else {
            lines.push(
                "No custom prompt profile is configured; using the built-in prompt.".to_string(),
            );
        }
        lines.join("\n")
    }

    async fn submit_input(&mut self) -> Result<()> {
        let input = self.input.get_input();
        let trimmed = input.trim();
        let shell_command = self.input.shell_command();
        let parsed_command = SlashCommand::parse(trimmed);

        if self.stream_rx.is_some() && !matches!(parsed_command, Some(SlashCommand::Btw { .. })) {
            return Ok(());
        }

        if trimmed.is_empty() || trimmed == "q" || trimmed == "exit" {
            if trimmed == "q" || trimmed == "exit" {
                self.should_quit = true;
            }
            self.input.clear();
            return Ok(());
        }

        self.footer.clear_status_message();
        self.clear_transient_slash_response();
        self.input.clear_btw_dropup();

        if let Some(command) = parsed_command {
            match command {
                SlashCommand::Btw { question } => {
                    self.capture_rewind_checkpoint(Self::checkpoint_preview(trimmed))
                        .await?;
                    self.input.clear();
                    if let Some(question) = question
                        .map(|value| value.trim().to_string())
                        .filter(|value| !value.is_empty())
                    {
                        self.start_btw_request(trimmed.to_string(), question);
                    } else {
                        self.input.set_btw_dropup("/btw", "Usage: /btw", false);
                    }
                    return Ok(());
                }
                SlashCommand::Compact => {
                    self.capture_rewind_checkpoint(Self::checkpoint_preview(trimmed))
                        .await?;
                    self.input.clear();
                    self.flush_before_compaction = true;
                    return Ok(());
                }
                SlashCommand::Context => {
                    self.capture_rewind_checkpoint(Self::checkpoint_preview(trimmed))
                        .await?;
                    self.input.clear();
                    let info = self.build_context_info();
                    let turn = self.messages.current_turn();
                    self.messages.add_context_report(info, turn);
                    return Ok(());
                }
                SlashCommand::Tools => {
                    self.capture_rewind_checkpoint(Self::checkpoint_preview(trimmed))
                        .await?;
                    self.input.clear();
                    self.messages
                        .add_local_command_result(self.build_tools_info());
                    return Ok(());
                }
                SlashCommand::Prompt => {
                    self.capture_rewind_checkpoint(Self::checkpoint_preview(trimmed))
                        .await?;
                    self.input.clear();
                    self.messages
                        .add_local_command_result(self.build_prompt_info());
                    return Ok(());
                }
                SlashCommand::NewAgent => {
                    self.capture_rewind_checkpoint(Self::checkpoint_preview(trimmed))
                        .await?;
                    self.input.clear();
                    self.pending_new_agent = true;
                    return Ok(());
                }
                SlashCommand::Agents => {
                    self.capture_rewind_checkpoint(Self::checkpoint_preview(trimmed))
                        .await?;
                    self.input.clear();
                    self.pending_agent_dialog = Some(AgentDialogAction::Open);
                    return Ok(());
                }
                SlashCommand::Language { language } => {
                    self.capture_rewind_checkpoint(Self::checkpoint_preview(trimmed))
                        .await?;
                    self.input.clear();
                    let note = if let Some(value) = language {
                        if let Some(language) = Language::parse_str(&value) {
                            crate::ui::i18n::set_language(language);
                            format!(
                                "{} {} ({})",
                                crate::ui::i18n::text("language.changed"),
                                crate::ui::i18n::language_name(language),
                                language.as_str()
                            )
                        } else {
                            crate::ui::i18n::text("language.invalid").to_string()
                        }
                    } else {
                        let language = crate::ui::i18n::language();
                        format!(
                            "{}: {} ({})",
                            crate::ui::i18n::text("language.current"),
                            crate::ui::i18n::language_name(language),
                            language.as_str()
                        )
                    };
                    self.messages.add_local_command_result(note);
                    return Ok(());
                }
                SlashCommand::Help => {
                    self.capture_rewind_checkpoint(Self::checkpoint_preview(trimmed))
                        .await?;
                    self.input.clear();
                    self.messages
                        .add_local_command_result(crate::ui::i18n::help_text());
                    return Ok(());
                }
                SlashCommand::Hooks => {
                    self.capture_rewind_checkpoint(Self::checkpoint_preview(trimmed))
                        .await?;
                    self.input.clear();
                    self.show_hooks_dialog();
                    return Ok(());
                }
                SlashCommand::Rewind { steps } => {
                    self.input.clear();
                    if let Some(steps) = steps {
                        if steps > 0 && steps <= self.rewind_checkpoints.len() {
                            let index = self.rewind_checkpoints.len() - steps;
                            let entry = self.rewind_checkpoints[index].clone();
                            self.restore_rewind_code(&entry)?;
                            self.restore_rewind_conversation(&entry).await?;
                        } else {
                            self.messages.add_local_command_result(format!(
                                "No rewind checkpoint available for {steps} step(s)."
                            ));
                        }
                    } else {
                        self.show_rewind_dialog().await?;
                    }
                    return Ok(());
                }
                SlashCommand::Export { path } => {
                    self.capture_rewind_checkpoint(Self::checkpoint_preview(trimmed))
                        .await?;
                    self.input.clear();
                    let target = path.as_deref().map(std::path::Path::new);
                    match self.export_to_path(target) {
                        Ok(written) => self.messages.add_local_command_result(format!(
                            "Exported conversation → {}",
                            written.display()
                        )),
                        Err(e) => self
                            .messages
                            .add_local_command_result(format!("Export failed: {e}")),
                    }
                    return Ok(());
                }
                SlashCommand::Exit => {
                    self.input.clear();
                    self.should_quit = true;
                    return Ok(());
                }
                SlashCommand::Unknown(name) => {
                    self.capture_rewind_checkpoint(Self::checkpoint_preview(trimmed))
                        .await?;
                    self.input.clear();
                    self.messages
                        .add_local_command_result(format!("Unknown skill: {name}"));
                    return Ok(());
                }
            }
        }

        if let Some(cmd) = shell_command {
            self.capture_rewind_checkpoint(Self::checkpoint_preview(trimmed))
                .await?;
            self.input.clear();

            let input_json = serde_json::json!({"command": cmd.clone()});
            let result = self.agent.registry().execute("bash", input_json).await;
            let output = match result {
                Ok(output) => output,
                Err(e) => format!("error: {e}"),
            };

            let display = format!("$ {cmd}\n{output}");
            self.messages.add_local_command_result(display);

            let prompt_text = format!("$ {cmd}");
            let result_text = output.clone();
            let agent = self.agent.clone();
            tokio::spawn(async move {
                let history = agent.history();
                let mut history = history.write().await;
                history.push(crate::agent::messages::Message::user(&prompt_text));
                history.push(crate::agent::messages::Message::assistant(vec![
                    crate::agent::messages::ContentBlock::Text { text: result_text },
                ]));
            });

            return Ok(());
        }

        self.capture_rewind_checkpoint(Self::checkpoint_preview(trimmed))
            .await?;
        self.messages.clear_active_thinking();
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

        let input_height = self
            .input
            .height()
            .saturating_add(self.input.completion_height());
        let status_height = u16::from(self.status_bar.is_active());
        let footer_height = 2;
        let transient_slash_response_height = self.transient_slash_response_height();

        // If a sidebar is open, reserve space on the right.
        // Clamp sidebar width so the main area never collapses below a usable minimum.
        let sidebar_min_width = 30u16;
        let sidebar_max_width = 40u16;
        let sidebar_width = if self.sidebar.is_some() {
            size.width
                .saturating_sub(sidebar_min_width)
                .min(sidebar_max_width)
        } else {
            0
        };

        let main_width = size.width.saturating_sub(sidebar_width);
        let fixed_chrome_height = input_height
            .saturating_add(transient_slash_response_height)
            .saturating_add(status_height)
            .saturating_add(footer_height);
        let thought_max_height = size.height.saturating_sub(fixed_chrome_height);
        let thought_height = self
            .messages
            .thought_panel_height(main_width, thought_max_height);
        let layout_area = Rect {
            width: main_width,
            ..size
        };
        let layout = if transient_slash_response_height > 0 {
            Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(thought_height),
                    Constraint::Length(transient_slash_response_height),
                    Constraint::Length(input_height),
                    Constraint::Length(status_height),
                    Constraint::Length(footer_height),
                ])
                .split(layout_area)
        } else {
            Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(thought_height),
                    Constraint::Length(input_height),
                    Constraint::Length(status_height),
                    Constraint::Length(footer_height),
                ])
                .split(layout_area)
        };

        let thought_area = layout[0];
        let (transient_slash_response_area, input_area, status_area, footer_area) =
            if transient_slash_response_height > 0 {
                (layout[1], layout[2], layout[3], layout[4])
            } else {
                (Rect::default(), layout[1], layout[2], layout[3])
            };

        self.messages_area = Rect::default();

        self.thought_area = thought_area;
        self.messages.render_thought_panel(frame, thought_area);
        self.render_transient_slash_response(frame, transient_slash_response_area);
        self.input.render(frame, input_area);
        self.status_bar.render(frame, status_area);
        self.footer.render(frame, footer_area);

        if let Some(ref dialog) = self.approval_dialog {
            dialog.render(frame, size);
        }

        if let Some(ref dialog) = self.slash_dialog {
            match dialog {
                SlashDialogState::Hooks(state) => state.dialog.render(frame, size),
                SlashDialogState::Rewind(state) => state.dialog.render(frame, size),
                SlashDialogState::RewindConfirm(state) => state.dialog.render(frame, size),
                SlashDialogState::Agents(state) => state.dialog.render(frame, size),
            }
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
    model_name: String,
    next_session_id: usize,
    next_sub_label: usize,
    /// When true, next run-loop tick replays the active session's committed history into the
    /// current inline terminal.
    pending_terminal_reset: bool,
    pending_reset_from_history: bool,
    pending_close: Option<(usize, Instant)>,
    /// When set, the active session is exported to this path on graceful exit.
    pending_export: Option<PathBuf>,
    #[cfg(feature = "test-utils")]
    recorder: Option<crate::test_utils::SessionRecorder>,
    #[cfg(feature = "test-utils")]
    next_frame_id: u64,
}

impl<C: LLMClient + Clone + 'static> App<C> {
    pub fn new(agent: Agent<C>, workdir: PathBuf, model_name: String) -> Self {
        let mut agent = agent;
        crate::ui::i18n::set_language(agent.config().tui.language);
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
            model_name,
            next_session_id: 1,
            next_sub_label: 1,
            pending_terminal_reset: false,
            pending_reset_from_history: false,
            pending_close: None,
            pending_export: None,
            #[cfg(feature = "test-utils")]
            recorder: None,
            #[cfg(feature = "test-utils")]
            next_frame_id: 0,
        }
    }

    /// Schedule an export of the active session to `path` on graceful exit.
    /// When `path` is `None`, the session is written to the default export
    /// directory (`<workdir>/.amadeus/exports/`).
    pub fn set_export_on_exit(&mut self, path: Option<PathBuf>) {
        self.pending_export = path;
    }

    /// Take the configured export path, clearing the pending flag.
    pub fn take_pending_export(&mut self) -> Option<PathBuf> {
        self.pending_export.take()
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

    pub async fn run(&mut self) -> Result<()> {
        #[cfg(feature = "test-utils")]
        if let Some(ref recorder) = self.recorder {
            recorder.record_session_start().await;
        }

        enable_raw_mode()?;
        let mut stdout = std::io::stdout();
        let _ = execute!(
            stdout,
            EnableMouseCapture,
            PushKeyboardEnhancementFlags(
                KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
                    | KeyboardEnhancementFlags::REPORT_ALL_KEYS_AS_ESCAPE_CODES
                    | KeyboardEnhancementFlags::REPORT_EVENT_TYPES
            )
        );

        let res = self.run_loop().await;

        let _ = execute!(
            std::io::stdout(),
            DisableMouseCapture,
            PopKeyboardEnhancementFlags
        );
        disable_raw_mode()?;
        let backend = CrosstermBackend::new(std::io::stdout());
        let mut terminal = Terminal::new(backend)?;
        terminal.show_cursor()?;

        if let Some(path) = self.take_pending_export() {
            if res.is_ok() {
                let active_idx = self.active_idx.min(self.sessions.len().saturating_sub(1));
                match self.sessions[active_idx].export_to_path(Some(&path)) {
                    Ok(written) => tracing::info!(
                        path = %written.display(),
                        "Exported conversation on exit"
                    ),
                    Err(e) => tracing::warn!(
                        error = %e,
                        path = %path.display(),
                        "Failed to export conversation on exit"
                    ),
                }
            }
        }

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

    fn build_session_tabs(&self) -> String {
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
            if !marker.is_empty() {
                label.push_str(&marker);
            }
            if idx == self.active_idx {
                label = format!("[{label}]");
            }
            parts.push(label);
        }
        parts.join(" ")
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

    fn open_agents_dialog(&mut self) {
        let active_idx = self.active_idx;
        let ordered_indices = std::iter::once(active_idx)
            .chain((0..self.sessions.len()).filter(|idx| *idx != active_idx));
        let mut session_ids = Vec::with_capacity(self.sessions.len());
        let mut items = Vec::with_capacity(self.sessions.len());

        for idx in ordered_indices {
            let session = &self.sessions[idx];
            session_ids.push(session.session_id);
            let detail = if idx == active_idx {
                crate::ui::i18n::text("agents.current")
            } else if session.stream_rx.is_some() {
                crate::ui::i18n::text("agents.working")
            } else {
                crate::ui::i18n::text("agents.idle")
            };
            items.push(SlashDialogItem::new(
                session.session_label.clone(),
                Some(detail.to_string()),
            ));
        }

        let session = self.active_session_mut();
        session.slash_dialog = Some(SlashDialogState::Agents(AgentsDialogState {
            dialog: SlashDialog::new(
                crate::ui::i18n::text("agents.title"),
                Some(crate::ui::i18n::text("agents.subtitle").to_string()),
                Vec::new(),
                crate::ui::i18n::text("agents.footer"),
                items,
            ),
            session_ids,
        }));
        session.mode = AppMode::SlashDialog;
    }

    fn process_pending_agent_dialog(&mut self) -> bool {
        let Some(action) = self.active_session_mut().pending_agent_dialog.take() else {
            return false;
        };

        match action {
            AgentDialogAction::Open => {
                self.open_agents_dialog();
                false
            }
            AgentDialogAction::SwitchTo(session_id) => {
                let Some(next_idx) = self
                    .sessions
                    .iter()
                    .position(|session| session.session_id == session_id)
                else {
                    return false;
                };
                if next_idx == self.active_idx {
                    return false;
                }
                self.switch_session(next_idx);
                true
            }
        }
    }

    fn switch_session(&mut self, next_idx: usize) {
        if next_idx >= self.sessions.len() || next_idx == self.active_idx {
            return;
        }
        let switching_to_empty = self.sessions[next_idx].messages.is_empty();
        self.pending_reset_from_history = !self.sessions[self.active_idx].messages.is_empty();
        self.active_idx = next_idx;
        self.pending_terminal_reset = true;
        let session = self.active_session_mut();
        session.is_background = false;
        session.footer.set_background(false);
        session.maybe_show_next_approval();
        if switching_to_empty {
            self.pending_reset_from_history = false;
        }
    }

    fn switch_to_next_session(&mut self) -> bool {
        if self.sessions.len() <= 1 {
            return false;
        }
        let next_idx = (self.active_idx + 1) % self.sessions.len();
        self.switch_session(next_idx);
        true
    }

    fn switch_to_previous_session(&mut self) -> bool {
        if self.sessions.len() <= 1 {
            return false;
        }
        let next_idx = if self.active_idx == 0 {
            self.sessions.len().saturating_sub(1)
        } else {
            self.active_idx - 1
        };
        self.switch_session(next_idx);
        true
    }

    fn switch_to_parent_session(&mut self) -> bool {
        let Some(parent_idx) = self.sessions[self.active_idx].parent_session_id else {
            return false;
        };
        self.switch_session(parent_idx);
        true
    }

    fn switch_to_child_session(&mut self) -> bool {
        let child_idx = self.sessions.iter().enumerate().find_map(|(idx, session)| {
            (session.parent_session_id == Some(self.active_idx)).then_some(idx)
        });

        let Some(child_idx) = child_idx else {
            return false;
        };
        self.switch_session(child_idx);
        true
    }

    fn new_inline_terminal(&self) -> std::io::Result<Terminal<CrosstermBackend<std::io::Stdout>>> {
        let terminal_size = crossterm::terminal::size()?;
        let initial_height = self
            .active_session()
            .max_shelf_height_for_terminal(terminal_size.1);
        let stdout = std::io::stdout();
        let backend = CrosstermBackend::new(stdout);
        Terminal::with_options(
            backend,
            TerminalOptions {
                viewport: Viewport::Inline(initial_height),
            },
        )
    }

    fn recycle_terminal_after_session_switch(
        &mut self,
        terminal: Terminal<CrosstermBackend<std::io::Stdout>>,
    ) -> Result<Terminal<CrosstermBackend<std::io::Stdout>>> {
        if !self.pending_terminal_reset {
            return Ok(terminal);
        }
        self.pending_terminal_reset = false;
        self.pending_reset_from_history = false;
        self.active_session_mut()
            .messages
            .reset_scrollback_cursor_for_session_switch();
        Ok(terminal)
    }

    fn should_finish_session_switch_immediately(&self) -> bool {
        self.pending_terminal_reset
            && !self.pending_reset_from_history
            && self.active_session().messages.is_empty()
    }

    fn finish_session_switch(
        &mut self,
        _terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    ) -> Result<()> {
        if !self.pending_terminal_reset {
            return Ok(());
        }

        if !std::io::stdout().is_terminal() {
            self.pending_terminal_reset = false;
            self.pending_reset_from_history = false;
            return Ok(());
        }

        if !self.should_finish_session_switch_immediately() {
            return Ok(());
        }

        self.pending_terminal_reset = false;
        self.pending_reset_from_history = false;
        self.active_session_mut()
            .messages
            .reset_scrollback_cursor_for_session_switch();
        Ok(())
    }

    fn handle_global_key(
        &mut self,
        key: crossterm::event::KeyEvent,
        terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    ) -> Result<bool> {
        match (key.modifiers, key.code) {
            (KeyModifiers::NONE, KeyCode::Tab)
            | (KeyModifiers::CONTROL, KeyCode::Char('i' | 'I')) => {
                let session = &self.sessions[self.active_idx];
                let should_defer_to_completion = session.stream_rx.is_none()
                    && (session.input.completion_is_visible()
                        || session.input.get_input().starts_with('/'));
                if !should_defer_to_completion && self.switch_to_next_session() {
                    self.finish_session_switch(terminal)?;
                    return Ok(true);
                }
            }
            (_, KeyCode::BackTab) => {
                let session = &self.sessions[self.active_idx];
                let should_defer_to_completion =
                    session.stream_rx.is_none() && session.input.completion_is_visible();
                if !should_defer_to_completion && self.switch_to_previous_session() {
                    self.finish_session_switch(terminal)?;
                    return Ok(true);
                }
            }
            (KeyModifiers::CONTROL, KeyCode::Char('5' | ']')) => {
                if self.switch_to_child_session() {
                    self.finish_session_switch(terminal)?;
                    return Ok(true);
                }
            }
            (KeyModifiers::CONTROL, KeyCode::Char('[')) => {
                if self.switch_to_parent_session() {
                    self.finish_session_switch(terminal)?;
                    return Ok(true);
                }
            }
            (KeyModifiers::NONE, KeyCode::Esc) => {
                let session = &self.sessions[self.active_idx];
                let is_idle = session.stream_rx.is_none() && session.mode == AppMode::Normal;
                if is_idle && self.switch_to_previous_session() {
                    self.finish_session_switch(terminal)?;
                    return Ok(true);
                }
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

    /// Spawn a new independent session with a fresh agent (empty history).
    fn spawn_new_session(&mut self) -> Result<()> {
        // Get client and config from the active session's agent
        let active_session = self.active_session();
        let client = active_session.agent.client().clone();
        let config = active_session.agent.config();

        // Create a fresh agent with empty history and default configuration
        let new_agent = Agent::builder(client, config).with_default_tools().build();

        let label = format!("session{}", self.next_session_id);

        let mut session = Session::new(
            new_agent,
            self.workdir.clone(),
            self.model_name.clone(),
            0,
            label,
            self.next_session_id,
        );
        self.next_session_id += 1;

        // Set up approval channels
        let (_channels, approval_handle) = create_approval_channels();
        session.approval_dec_tx = Some(approval_handle.decision_tx);
        session.approval_req_rx = Some(approval_handle.request_rx);

        // Don't start streaming - just create the session and switch to it
        session.input.clear();
        session.current_text.clear();
        session.streaming_buffer.clear();
        session.clear_tool_monitor();
        session.messages.clear_streaming_text();
        session.sync_activity_chrome();

        self.sessions.push(session);
        self.switch_session(self.sessions.len() - 1);

        Ok(())
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

            // Automatically switch back to parent session if the completed subagent was active
            if self.active_idx == child_idx {
                self.switch_session(parent_idx);
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
            let mut needs_approval = false;
            if let Some(ref mut rx) = approval_rx {
                while let Ok(request) = rx.try_recv() {
                    self.sessions[idx].enqueue_approval_request(request, false);
                    needs_approval = true;
                }
            }
            self.sessions[idx].approval_req_rx = approval_rx;

            if needs_approval {
                self.switch_session(idx);
                break;
            }

            self.sessions[idx].poll_compaction_result();
        }

        for idx in completed_children {
            self.complete_child_session(idx).await;
        }

        Ok(())
    }

    async fn run_loop(&mut self) -> Result<()> {
        let mut terminal = self.new_inline_terminal()?;

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

            terminal = self.recycle_terminal_after_session_switch(terminal)?;

            {
                let session = self.active_session_mut();
                session.poll_compaction_result();
                session.sync_inline_viewport(&mut terminal)?;

                session.flush_unrendered_history(&mut terminal)?;
            }

            let completed = terminal.draw(|f| {
                let breadcrumb = self.build_session_tabs();
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
                        let mut needs_new_agent = false;
                        let (should_quit, session_stream_rx, session_approval_rx) = {
                            let session = &mut self.sessions[active_idx];
                            session.handle_event(event, &mut terminal).await?;
                            session.apply_pending_transcript_reset(&mut terminal)?;
                            session.flush_unrendered_history(&mut terminal)?;
                            if session.flush_before_compaction {
                                session.flush_streaming_buffer(&mut terminal)?;
                                session.flush_before_compaction = false;
                                session.start_compaction();
                            }
                            if session.pending_new_agent {
                                session.pending_new_agent = false;
                                needs_new_agent = true;
                            }
                            let session_stream_rx = session.stream_rx.take();
                            let session_approval_rx = session.approval_req_rx.take();
                            (
                                session.should_quit,
                                session_stream_rx,
                                session_approval_rx,
                            )
                        };
                        if needs_new_agent {
                            self.spawn_new_session()?;
                            self.finish_session_switch(&mut terminal)?;
                        }
                        if self.process_pending_agent_dialog() {
                            self.finish_session_switch(&mut terminal)?;
                        }
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
                        session.loading_indicator.tick();
                        session.sync_prompt_status_hint();
                        let completed = terminal.draw(|f| {
                            let breadcrumb = self.build_session_tabs();
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

#[cfg(feature = "test-utils")]
impl<C: LLMClient + Clone + 'static> App<C> {
    /// Test-only accessor for the active session (headless driver).
    pub(crate) fn test_session_mut(&mut self) -> &mut Session<C> {
        self.active_session_mut()
    }
}

#[cfg(feature = "test-utils")]
impl<C: LLMClient + Clone + 'static> Session<C> {
    /// Type one character into the input box.
    pub(crate) fn test_type_char(&mut self, c: char) {
        self.input.handle_char(c);
    }

    /// Submit the current input (starts a prompt turn or runs a slash command).
    pub(crate) async fn test_submit(&mut self) -> Result<()> {
        self.submit_input().await
    }

    /// Render the session into a ratatui frame (TestBackend draws into a buffer).
    pub(crate) fn test_render(&mut self, frame: &mut ratatui::Frame) {
        self.render(frame);
    }

    /// Drain the in-flight agent event stream to completion, headlessly (no
    /// terminal output). Commits the assistant message and tool groups via the
    /// `render_to_terminal = false` path that background sub-agent sessions use.
    /// Mirrors the run-loop drain in `run_loop` (see lines ~1503-1566) but with
    /// `render_to_terminal = false`, so the throwaway stdout terminal is never
    /// drawn to.
    pub(crate) async fn test_pump_turn(&mut self) -> Result<()> {
        let mut terminal = Terminal::new(CrosstermBackend::new(std::io::stdout()))?;
        loop {
            let event = match &mut self.stream_rx {
                Some(rx) => rx.recv().await,
                None => return Ok(()),
            };
            match event {
                None => {
                    self.stream_rx = None;
                    self.stream_abort = None;
                    return Ok(());
                }
                Some(ev) => {
                    let action = self.handle_agent_event(ev, &mut terminal, false)?;
                    if matches!(action, SessionAction::Done) {
                        self.stream_rx = None;
                        self.stream_abort = None;
                        return Ok(());
                    }
                }
            }
        }
    }

    /// Drain and return the session's unrendered message lines as joined text.
    ///
    /// Amadeus renders committed conversation messages via terminal scrollback
    /// (`insert_before`), not the live frame buffer, so headless tests assert on
    /// this message store rather than on a captured frame.
    ///
    /// **Draining semantics:** this consumes the queue. Each call returns only
    /// the lines committed since the previous call (or since session start).
    /// To assert on the full transcript across multiple turns, accumulate the
    /// returned strings rather than expecting earlier turns to remain.
    pub(crate) fn test_drain_unrendered_text(&mut self, width: u16) -> String {
        self.messages
            .take_unrendered_lines(width)
            .into_iter()
            .map(|line| line.to_string())
            .collect::<Vec<_>>()
            .join("\n")
    }
}

#[cfg(test)]
#[path = "app_tests.rs"]
mod tests;
