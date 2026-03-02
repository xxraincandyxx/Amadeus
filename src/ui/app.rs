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
use tracing::info;

use crate::agent::events::AgentEvent;
use crate::agent::loop_agent::Agent;
use crate::client::LLMClient;
use crate::error::Result;
use crate::ui::{get_colors, get_theme, next_theme};
use crate::ui::components::{
    AppState, FileSidebar, Footer, HelpSidebar, InputComponent, LoadingIndicator,
    MessagesComponent, Sidebar, SidebarKind, StatusBar, StreamingState,
};
use crate::ui::event::{AppEvent, EventHandler};

const MARGIN: u16 = 1;
const MIN_CONTENT_WIDTH: u16 = 60;
const SIDEBAR_PERCENTAGE: u16 = 20;
const MIN_SIDEBAR_WIDTH: u16 = 15;

pub enum AppMode {
    Normal,
    Input,
}

pub struct App<C: LLMClient> {
    agent: Agent<C>,
    mode: AppMode,
    messages: MessagesComponent,
    input: InputComponent,
    status: StatusBar,
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
}

impl<C: LLMClient + Clone + 'static> App<C> {
    pub fn new(agent: Agent<C>, workdir: PathBuf, model_name: String) -> Self {
        let status = StatusBar::new(model_name.clone());
        let footer = Footer::new(model_name);
        let loading_indicator = LoadingIndicator::new();

        Self {
            agent,
            mode: AppMode::Input,
            messages: MessagesComponent::new(),
            input: InputComponent::new(),
            status,
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
        }
    }

    pub fn set_mesh_mode(&mut self, addr: &str) {
        self.mesh_supervisor_addr = Some(addr.to_string());
        self.status.is_mesh = true;
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
                self.status.tick();
                self.loading_indicator.tick();
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
                self.status.set_state(AppState::Processing);
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
                self.status.set_state(AppState::Success);
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
                self.status.set_state(AppState::Error);
                self.loading_indicator.set_streaming_state(StreamingState::Idle);
                self.current_text.clear();
                return true;
            }

            AgentEvent::SessionSaved { path } => {
                info!(path = %path, "Session log saved to disk");
            }

            AgentEvent::ToolInputDelta { .. } => {}

            AgentEvent::ApprovalRequired { tool, reason, .. } => {
                info!(tool = %tool, reason = %reason, "Approval required for tool execution");
                // TODO: Show approval dialog and collect user response
            }
        }

        false
    }

    async fn handle_key(&mut self, key: crossterm::event::KeyEvent) -> Result<()> {
        match self.mode {
            AppMode::Normal => self.handle_normal_key(key).await,
            AppMode::Input => self.handle_input_key(key).await,
        }
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
            (KeyModifiers::CONTROL, KeyCode::Char('d')) => {
                self.should_quit = true;
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
        self.status.set_state(AppState::Idle);
        self.loading_indicator.set_streaming_state(StreamingState::Idle);
    }

    fn can_show_sidebar(&self, area: Rect) -> bool {
        area.width >= MIN_CONTENT_WIDTH
    }

    fn toggle_sidebar(&mut self, kind: SidebarKind) {
        self.sidebar = match (&self.sidebar, kind) {
            (Some(Sidebar::Files(_)), SidebarKind::Files) => None,
            (Some(Sidebar::Help(_)), SidebarKind::Help) => None,
            (_, SidebarKind::Files) => Some(Sidebar::Files(FileSidebar::new(self.workdir.clone()))),
            (_, SidebarKind::Help) => Some(Sidebar::Help(HelpSidebar::new())),
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

        self.messages.add_user(trimmed.to_string());
        self.input.clear();
        self.current_text.clear();
        self.status.set_state(AppState::Processing);
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

        let (tx, rx) = mpsc::channel(64);
        let handle = tokio::spawn(async move {
            // Add to history first
            {
                let history_arc = agent.history();
                let mut history = history_arc.write().await;
                history.push(crate::agent::messages::Message::user(&prompt));
            }

            let mut stream = agent.run_stream();
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
                }
            }
        }

        let input_height = self.input.height();
        let status_height = 1u16;
        let footer_height = 1u16;
        let loading_height = if self.loading_indicator.is_active() { 1u16 } else { 0u16 };

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(footer_height),
                Constraint::Length(status_height),
                Constraint::Min(0),
                Constraint::Length(loading_height),
                Constraint::Length(input_height),
            ])
            .split(main_area);

        self.footer.render(frame, chunks[0]);
        self.status.render(frame, chunks[1]);
        self.messages_area = chunks[2];
        self.messages.render(frame, chunks[2]);

        if loading_height > 0 {
            self.loading_indicator.render(frame, chunks[3]);
        }

        self.input.render(frame, chunks[4]);
    }
}
