use std::path::PathBuf;
use std::time::Duration;

use crossterm::{
    event::{KeyCode, KeyModifiers, MouseButton, MouseEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use futures::StreamExt;
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    Terminal,
};
use tokio::sync::mpsc;
use tracing::{info, warn};

use crate::agent::events::{AgentEvent, ApprovalDecision, ApprovalRequest};
use crate::agent::loop_agent::{create_approval_channels, Agent};
use crate::client::LLMClient;
use crate::error::Result;
use crate::ui::{get_colors, get_theme, next_theme};
use crate::ui::components::{
    ApprovalDialog, ApprovalResponse, FileSidebar, Footer, HelpSidebar,
    InputComponent, LoadingIndicator, MessagesComponent, Sidebar, SidebarKind, StreamingState,
};
use crate::ui::event::{AppEvent, EventHandler};

const MARGIN: u16 = 1;
const MIN_CONTENT_WIDTH: u16 = 60;
const SIDEBAR_PERCENTAGE: u16 = 20;
const MIN_SIDEBAR_WIDTH: u16 = 15;

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
    compaction_result_rx: Option<mpsc::Receiver<std::result::Result<crate::agent::compaction::CompactionResult, crate::error::AgentError>>>,
}

impl<C: LLMClient + Clone + 'static> App<C> {
    pub fn new(agent: Agent<C>, workdir: PathBuf, model_name: String) -> Self {
        let footer = Footer::new(model_name);
        let loading_indicator = LoadingIndicator::new();

        Self {
            agent,
            mode: AppMode::Input,
            messages: MessagesComponent::new(),
            input: InputComponent::new(),
            footer,
            loading_indicator,
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
        }
    }

    pub fn set_mesh_mode(&mut self, addr: &str) {
        self.mesh_supervisor_addr = Some(addr.to_string());
        self.footer.set_mesh(true);
    }

    pub async fn run(&mut self) -> Result<()> {
        enable_raw_mode()?;
        let mut stdout = std::io::stdout();
        execute!(stdout, EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        let mut events = EventHandler::new(Duration::from_millis(100));

        let res = self.run_loop(&mut terminal, &mut events).await;

        disable_raw_mode()?;
        execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
        terminal.show_cursor()?;

        res
    }

    async fn run_loop(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
        events: &mut EventHandler,
    ) -> Result<()> {
        loop {
            if self.should_quit {
                break;
            }

            terminal.draw(|f| self.render(f))?;

            tokio::select! {
                event = events.next() => {
                    self.handle_event(event?).await?;
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
                        }
                    } else {
                        self.stream_rx = None;
                        self.stream_abort = None;
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
                        self.show_approval_dialog(request);
                    }
                }
            }
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
                self.current_text.push_str(&delta);
                self.messages.update_streaming_text(&self.current_text);
            }

            AgentEvent::ToolStart { id, name } => {
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
                self.messages.finalize_assistant(result.text);
                self.loading_indicator.set_streaming_state(StreamingState::Idle);
                self.current_text.clear();
                return true;
            }

            AgentEvent::Error { message } => {
                if self.current_text.is_empty() {
                    self.messages.add_assistant(format!("Error: {}", message));
                } else {
                    self.messages
                        .finalize_assistant(format!("{}\n\nError: {}", self.current_text, message));
                }
                self.loading_indicator.set_streaming_state(StreamingState::Idle);
                self.current_text.clear();
                return true;
            }

            AgentEvent::SessionSaved { path } => {
                info!(path = %path, "Session log saved to disk");
            }

            AgentEvent::ToolInputDelta { .. } => {}

            AgentEvent::ApprovalRequired { request } => {
                // This event is just for logging/notification.
                // The actual approval dialog is triggered via the approval channel
                // which is polled in the main select! loop.
                info!(tool = %request.tool, reason = %request.reason, "Approval required for tool execution");
            }

            AgentEvent::TokenUsage { input_tokens, output_tokens, total_tokens } => {
                // Calculate context window percentage
                let context_size = self.agent.config().context_window_size;
                if context_size > 0 {
                    let percent = ((total_tokens as f32 / context_size as f32) * 100.0).min(100.0) as u8;
                    self.footer.set_context_percent(percent);
                }

                info!(input = input_tokens, output = output_tokens, total = total_tokens, "Token usage");
            }

            AgentEvent::ToolProgress { id, message, percent } => {
                info!(id = %id, message = %message, percent = ?percent, "Tool progress");
                // Update tool progress in messages component
                self.messages.update_tool_progress(&id, message, percent);
            }

            AgentEvent::Compaction { original_count, compacted_count, tokens_saved, messages_summarized } => {
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

        info!(
            messages = current_count,
            "Manual compaction triggered"
        );

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
            let result = compactor.compact(&mut history_guard, &client, context_window_size).await;

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
                        original = result.original_count,
                        compacted = result.compacted_count,
                        tokens_saved = result.tokens_saved,
                        "Manual compaction complete"
                    );

                    // Estimate token counts from message counts
                    let original_tokens = result.original_count * 200;
                    let final_tokens = result.compacted_count * 200;

                    // Check if compaction was beneficial
                    if result.tokens_saved > 0 {
                        self.messages.complete_compression(original_tokens, final_tokens);
                    } else {
                        self.messages.complete_compression_not_beneficial(original_tokens);
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
                    self.messages.complete_compression_failed("Compaction task crashed".to_string());
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
            (KeyModifiers::CONTROL, KeyCode::Char('b')) => {
                self.toggle_sidebar(SidebarKind::Files);
            }
            (KeyModifiers::SUPER, KeyCode::Char('b')) => {
                self.toggle_sidebar(SidebarKind::Files);
            }
            (KeyModifiers::ALT, KeyCode::Char('b')) => {
                self.toggle_sidebar(SidebarKind::Help);
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
        self.loading_indicator.set_streaming_state(StreamingState::Idle);
    }

    fn can_show_sidebar(&self, area: Rect) -> bool {
        area.width >= MIN_CONTENT_WIDTH
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
                    &self.workdir.join(".amadeus/skills")
                ).unwrap_or_default();
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
                self.start_compaction();
                return Ok(());
            }
            // Add other slash commands here in the future
        }

        self.messages.add_user(trimmed.to_string());
        self.input.clear();
        self.current_text.clear();
        self.loading_indicator.set_streaming_state(StreamingState::Responding);

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
                                let _ = tx.send(AgentEvent::Done { 
                                    result: crate::agent::events::RunResult {
                                        text,
                                        tool_calls: Vec::new(),
                                    }
                                }).await;
                            }
                        } else {
                            let error_body = resp.text().await.unwrap_or_else(|_| "Unknown error".to_string());
                            let _ = tx.send(AgentEvent::Error { 
                                message: format!("Supervisor error ({}): {}", addr, error_body) 
                            }).await;
                        }
                    }
                    Err(e) => {
                        let _ = tx.send(AgentEvent::Error { message: format!("Connection failed to {}: {}", addr, e) }).await;
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
        let colors = get_colors();

        let bg =
            ratatui::widgets::Block::default().style(ratatui::style::Style::default().bg(colors.background.primary));
        frame.render_widget(bg, size);

        let margin_area = Rect::new(
            size.x + MARGIN,
            size.y + MARGIN,
            size.width.saturating_sub(MARGIN * 2),
            size.height.saturating_sub(MARGIN * 2),
        );

        if margin_area.width < 10 || margin_area.height < 5 {
            return;
        }

        let (main_area, sidebar_area) =
            if self.sidebar.is_some() && self.can_show_sidebar(margin_area) {
                let sidebar_width =
                    (margin_area.width * SIDEBAR_PERCENTAGE / 100).max(MIN_SIDEBAR_WIDTH);
                let main_width = margin_area.width.saturating_sub(sidebar_width);

                if main_width < 20 {
                    (margin_area, None)
                } else {
                    let main = Rect::new(
                        margin_area.x + sidebar_width,
                        margin_area.y,
                        main_width,
                        margin_area.height,
                    );
                    let sidebar = Rect::new(
                        margin_area.x,
                        margin_area.y,
                        sidebar_width,
                        margin_area.height,
                    );
                    (main, Some(sidebar))
                }
            } else {
                (margin_area, None)
            };

        if let Some(sidebar_area) = sidebar_area {
            self.sidebar_area = sidebar_area;
            if let Some(ref sidebar) = self.sidebar {
                match sidebar {
                    Sidebar::Files(fs) => fs.render(frame, sidebar_area),
                    Sidebar::Help(hs) => hs.render(frame, sidebar_area),
                    Sidebar::Skills(ss) => ss.render(frame, sidebar_area),
                }
            }
        }

        let input_height = self.input.height();
        let footer_height = 1u16;
        let loading_height = if self.loading_indicator.is_active() { 1u16 } else { 0u16 };

        // Layout: Messages → Loading → Input → Footer (at bottom)
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(0),  // Messages (maximized)
                Constraint::Length(loading_height),
                Constraint::Length(input_height),
                Constraint::Length(footer_height),
            ])
            .split(main_area);

        self.messages_area = chunks[0];
        self.messages.render(frame, chunks[0]);

        if loading_height > 0 {
            self.loading_indicator.render(frame, chunks[1]);
        }

        self.input.render(frame, chunks[2]);
        self.footer.render(frame, chunks[3]);

        // Render approval dialog on top if active
        if let Some(ref dialog) = self.approval_dialog {
            dialog.render(frame, size);
        }
    }
}
