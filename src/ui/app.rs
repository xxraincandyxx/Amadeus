use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use crossterm::{
    event::{KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    Terminal,
};
use tokio::sync::RwLock;

use crate::agent::loop_agent::Agent;
use crate::agent::messages::Message;
use crate::client::LLMClient;
use crate::error::Result;
use crate::ui::colors::THEME;
use crate::ui::components::{
    AppState, FileSidebar, HelpSidebar, InputComponent, MessagesComponent, Sidebar, SidebarKind,
    StatusBar, ToolPanel, ToolResult,
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
    history: Arc<RwLock<Vec<Message>>>,
    mode: AppMode,
    messages: MessagesComponent,
    input: InputComponent,
    status: StatusBar,
    tool_panel: ToolPanel,
    sidebar: Option<Sidebar>,
    should_quit: bool,
    workdir: PathBuf,
}

impl<C: LLMClient> App<C> {
    pub fn new(agent: Agent<C>, workdir: PathBuf, model_name: String) -> Self {
        let history = Arc::new(RwLock::new(Vec::new()));
        let status = StatusBar::new(model_name);

        Self {
            agent,
            history,
            mode: AppMode::Input,
            messages: MessagesComponent::new(),
            input: InputComponent::new(),
            status,
            tool_panel: ToolPanel::new(),
            sidebar: None,
            should_quit: false,
            workdir,
        }
    }

    pub async fn run(&mut self) -> Result<()> {
        enable_raw_mode()?;
        let mut stdout = std::io::stdout();
        execute!(stdout, EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        let events = EventHandler::new(Duration::from_millis(100));

        let res = self.run_loop(&mut terminal, events).await;

        disable_raw_mode()?;
        execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
        terminal.show_cursor()?;

        res
    }

    async fn run_loop(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
        events: EventHandler,
    ) -> Result<()> {
        loop {
            if self.should_quit {
                break;
            }

            terminal.draw(|f| self.render(f))?;

            match events.next()? {
                AppEvent::Key(key) => self.handle_key(key).await?,
                AppEvent::Mouse(_) => {}
                AppEvent::Resize(_, _) => {}
                AppEvent::Tick => {
                    self.status.tick();
                }
            }
        }

        Ok(())
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
            (KeyModifiers::NONE, KeyCode::Esc) => {
                self.sidebar = None;
                self.tool_panel.collapse_all();
            }
            (KeyModifiers::NONE, KeyCode::Up) => {
                if let Some(Sidebar::Files(ref mut sidebar)) = self.sidebar {
                    sidebar.select_up();
                }
            }
            (KeyModifiers::NONE, KeyCode::Down) => {
                if let Some(Sidebar::Files(ref mut sidebar)) = self.sidebar {
                    sidebar.select_down();
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
                self.mode = AppMode::Normal;
                self.sidebar = None;
                self.tool_panel.collapse_all();
            }
            (KeyModifiers::NONE, KeyCode::Up) => {
                self.input.history_up();
            }
            (KeyModifiers::NONE, KeyCode::Down) => {
                self.input.history_down();
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
                if self.input.get_input().trim().is_empty() {
                    self.should_quit = true;
                } else {
                    self.input.handle_char('q');
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
            (KeyModifiers::NONE, KeyCode::PageUp) => {
                self.messages.scroll_up(10);
            }
            (KeyModifiers::NONE, KeyCode::PageDown) => {
                self.messages.scroll_down(10);
            }
            (KeyModifiers::NONE | KeyModifiers::SHIFT, KeyCode::Char(c)) => {
                self.input.handle_char(c);
            }
            _ => {}
        }
        Ok(())
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
        self.tool_panel.clear();

        self.status.set_state(AppState::Processing);

        let result = self.agent.run(trimmed, Arc::clone(&self.history)).await;

        self.status.set_state(match &result {
            Ok(_) => AppState::Success,
            Err(_) => AppState::Error,
        });

        match result {
            Ok(run_result) => {
                for tool_call in &run_result.tool_calls {
                    let command = if tool_call.name == "bash" {
                        tool_call
                            .input
                            .get("command")
                            .and_then(|v| v.as_str())
                            .map(String::from)
                    } else {
                        None
                    };

                    self.tool_panel.add_result(ToolResult {
                        tool_name: tool_call.name.clone(),
                        command,
                        output: tool_call.output.clone(),
                        is_error: tool_call.is_error,
                        is_collapsed: false,
                    });
                }

                self.messages.add_assistant(run_result.text);
            }
            Err(e) => {
                self.messages.add_assistant(format!("Error: {}", e));
            }
        }

        Ok(())
    }

    fn render(&mut self, frame: &mut ratatui::Frame) {
        let size = frame.area();

        let bg =
            ratatui::widgets::Block::default().style(ratatui::style::Style::default().bg(THEME.bg));
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
            if let Some(ref sidebar) = self.sidebar {
                match sidebar {
                    Sidebar::Files(fs) => fs.render(frame, sidebar_area),
                    Sidebar::Help(hs) => hs.render(frame, sidebar_area),
                }
            }
        }

        let input_height = self.input.height();
        let status_height = 1u16;
        let tool_height = if self.tool_panel.has_results() {
            8u16
        } else {
            0u16
        };

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(status_height),
                Constraint::Min(0),
                Constraint::Length(tool_height),
                Constraint::Length(input_height),
            ])
            .split(main_area);

        self.status.render(frame, chunks[0]);
        self.messages.render(frame, chunks[1]);

        if tool_height > 0 && chunks[2].height > 0 {
            self.tool_panel.render(frame, chunks[2]);
        }

        self.input.render(frame, chunks[3]);
    }
}
