use std::process::Command;
use std::time::{Duration, Instant};

use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::ui::get_colors;
use super::status::AppState;

#[derive(Debug, Clone)]
pub struct FooterInfo {
    pub cwd: String,
    pub git_branch: Option<String>,
    pub sandbox_status: SandboxStatus,
    pub model_name: String,
    pub context_percent: u8,
    pub is_mesh: bool,
    // New fields from StatusBar
    pub state: AppState,
    pub elapsed: Option<Duration>,
    pub token_count: usize,
    /// Temporary status message (shown for a short time)
    pub status_message: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SandboxStatus {
    None,
    Docker,
    Seatbelt(String),
    Other(String),
}

impl Default for SandboxStatus {
    fn default() -> Self {
        Self::None
    }
}

pub struct Footer {
    info: FooterInfo,
    hide_cwd: bool,
    hide_sandbox: bool,
    hide_model: bool,
    hide_context_percent: bool,
    // For timing and animation
    start_time: Option<Instant>,
    spinner_frame: usize,
    // Status message expiry
    status_message_expiry: Option<Instant>,
}

impl Footer {
    pub fn new(model_name: String) -> Self {
        let cwd = std::env::current_dir()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| ".".to_string());

        let git_branch = Self::detect_git_branch();
        let sandbox_status = Self::detect_sandbox();

        Self {
            info: FooterInfo {
                cwd,
                git_branch,
                sandbox_status,
                model_name,
                context_percent: 0,
                is_mesh: false,
                state: AppState::Idle,
                elapsed: None,
                token_count: 0,
                status_message: None,
            },
            hide_cwd: false,
            hide_sandbox: false,
            hide_model: false,
            hide_context_percent: false,
            start_time: None,
            spinner_frame: 0,
            status_message_expiry: None,
        }
    }

    fn detect_git_branch() -> Option<String> {
        let output = Command::new("git")
            .args(["rev-parse", "--abbrev-ref", "HEAD"])
            .output()
            .ok()?;

        if output.status.success() {
            let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !branch.is_empty() && branch != "HEAD" {
                return Some(branch);
            }
        }
        None
    }

    fn detect_sandbox() -> SandboxStatus {
        if let Ok(sandbox) = std::env::var("SANDBOX") {
            match sandbox.as_str() {
                "docker" | "podman" => return SandboxStatus::Docker,
                "sandbox-exec" => {
                    let profile =
                        std::env::var("SEATBELT_PROFILE").unwrap_or_else(|_| "default".to_string());
                    return SandboxStatus::Seatbelt(profile);
                }
                other if !other.is_empty() => {
                    let name = other.strip_prefix("gemini-").unwrap_or(other).to_string();
                    return SandboxStatus::Other(name);
                }
                _ => {}
            }
        }

        if std::env::var("container").is_ok() {
            return SandboxStatus::Docker;
        }

        SandboxStatus::None
    }

    fn shorten_path(path: &str, max_len: usize) -> String {
        if path.len() <= max_len {
            return path.to_string();
        }

        let home = dirs::home_dir()
            .map(|h| h.display().to_string())
            .unwrap_or_default();

        let tilde_path = if path.starts_with(&home) {
            format!("~{}", &path[home.len()..])
        } else {
            path.to_string()
        };

        if tilde_path.len() <= max_len {
            return tilde_path;
        }

        let parts: Vec<&str> = tilde_path.split('/').collect();
        if parts.len() <= 2 {
            return tilde_path;
        }

        let first = parts.first().unwrap_or(&"");
        let last = parts.last().unwrap_or(&"");

        format!("{}/.../{}", first, last)
    }

    pub fn set_context_percent(&mut self, percent: u8) {
        self.info.context_percent = percent.min(100);
    }

    pub fn set_model_name(&mut self, name: String) {
        self.info.model_name = name;
    }

    pub fn set_mesh(&mut self, is_mesh: bool) {
        self.info.is_mesh = is_mesh;
    }

    pub fn set_state(&mut self, state: AppState) {
        if state == AppState::Processing && self.start_time.is_none() {
            self.start_time = Some(Instant::now());
        } else if state != AppState::Processing {
            self.start_time = None;
        }
        self.info.state = state;
    }

    pub fn set_token_count(&mut self, count: usize) {
        self.info.token_count = count;
    }

    /// Set a temporary status message that will be displayed for a few seconds.
    pub fn set_status_message(&mut self, message: impl Into<String>) {
        self.info.status_message = Some(message.into());
        self.status_message_expiry = Some(Instant::now() + Duration::from_secs(3));
    }

    /// Clear the status message immediately.
    pub fn clear_status_message(&mut self) {
        self.info.status_message = None;
        self.status_message_expiry = None;
    }

    pub fn tick(&mut self) {
        self.spinner_frame = (self.spinner_frame + 1) % 10;
        // Update elapsed time if processing
        if let Some(start) = self.start_time {
            self.info.elapsed = Some(start.elapsed());
        }
        // Expire status message if time has passed
        if let Some(expiry) = self.status_message_expiry {
            if Instant::now() >= expiry {
                self.info.status_message = None;
                self.status_message_expiry = None;
            }
        }
    }

    pub fn refresh_git_branch(&mut self) {
        self.info.git_branch = Self::detect_git_branch();
    }

    pub fn info(&self) -> &FooterInfo {
        &self.info
    }

    fn get_spinner(&self) -> &'static str {
        const SPINNER_FRAMES: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
        SPINNER_FRAMES[self.spinner_frame]
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        if area.width < 3 {
            return;
        }

        let colors = get_colors();
        let mut spans = Vec::new();

        // Status indicator (from StatusBar)
        let (status_icon, status_color, status_text) = match self.info.state {
            AppState::Idle => ("●".to_string(), colors.ui.comment, "IDLE"),
            AppState::Processing => (
                self.get_spinner().to_string(),
                colors.text.link,
                "BUSY",
            ),
            AppState::Success => ("✓".to_string(), colors.status.success, "DONE"),
            AppState::Error => ("✗".to_string(), colors.status.error, "ERR"),
        };

        spans.push(Span::styled(
            format!("{} ", status_icon),
            Style::default().fg(status_color).add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::styled(
            format!("{} ", status_text),
            Style::default().fg(status_color),
        ));

        // Elapsed time when processing
        if self.info.state == AppState::Processing {
            if let Some(elapsed) = self.info.elapsed {
                spans.push(Span::styled(
                    format!("{:.1}s ", elapsed.as_secs_f64()),
                    Style::default().fg(colors.ui.comment),
                ));
            }
        }

        // Token count
        if self.info.token_count > 0 {
            spans.push(Span::styled(
                format!("{}t ", self.info.token_count),
                Style::default().fg(colors.status.warning),
            ));
        }

        // MESH indicator
        if self.info.is_mesh {
            spans.push(Span::styled(
                "MESH ",
                Style::default()
                    .fg(colors.background.primary)
                    .bg(colors.text.accent)
                    .add_modifier(Modifier::BOLD),
            ));
        }

        // Status message (temporary notification)
        if let Some(ref message) = self.info.status_message {
            spans.push(Span::styled(
                format!("{} ", message),
                Style::default().fg(colors.text.accent),
            ));
        }

        // Separator
        spans.push(Span::styled("│", Style::default().fg(colors.ui.dark)));

        // CWD and git branch
        let path_len = ((area.width as usize) / 4).max(15).min(40);

        if !self.hide_cwd {
            let display_path = Self::shorten_path(&self.info.cwd, path_len);
            spans.push(Span::raw(" "));
            spans.push(Span::styled(
                display_path,
                Style::default().fg(colors.text.primary),
            ));

            if let Some(ref branch) = self.info.git_branch {
                spans.push(Span::raw(" "));
                spans.push(Span::styled(
                    format!("({}*)", branch),
                    Style::default().fg(colors.text.secondary),
                ));
            }
        }

        if !self.hide_sandbox {
            spans.push(Span::raw(" "));
            spans.push(Span::styled("│", Style::default().fg(colors.ui.dark)));

            let (sandbox_text, sandbox_color) = match &self.info.sandbox_status {
                SandboxStatus::None => ("no sandbox".to_string(), colors.status.error),
                SandboxStatus::Docker => ("docker".to_string(), colors.status.success),
                SandboxStatus::Seatbelt(profile) => (
                    format!("seatbelt:{}", profile),
                    colors.status.warning,
                ),
                SandboxStatus::Other(name) => (name.clone(), colors.status.success),
            };

            spans.push(Span::raw(" "));
            spans.push(Span::styled(sandbox_text, Style::default().fg(sandbox_color)));
        }

        // Right side: model and context
        let mut right_spans = Vec::new();

        if !self.hide_model {
            right_spans.push(Span::raw(" "));
            right_spans.push(Span::styled("│", Style::default().fg(colors.ui.dark)));
            right_spans.push(Span::raw(" "));
            right_spans.push(Span::styled(
                self.info.model_name.clone(),
                Style::default().fg(colors.text.accent),
            ));

            if !self.hide_context_percent && self.info.context_percent > 0 {
                let percent_color = if self.info.context_percent >= 90 {
                    colors.status.error
                } else if self.info.context_percent >= 70 {
                    colors.status.warning
                } else {
                    colors.text.secondary
                };

                right_spans.push(Span::styled(
                    format!(" {}%", self.info.context_percent),
                    Style::default().fg(percent_color),
                ));
            }
        }

        let left_width: usize = spans.iter().map(|s| s.content.chars().count()).sum();
        let right_width: usize = right_spans.iter().map(|s| s.content.chars().count()).sum();
        let available = (area.width as usize).saturating_sub(left_width + right_width);

        if available > 0 {
            spans.push(Span::raw(" ".repeat(available)));
        }

        spans.extend(right_spans);

        let line = Line::from(spans);
        let paragraph = Paragraph::new(line).style(Style::default().bg(colors.background.primary));

        frame.render_widget(paragraph, area);
    }
}

impl Default for Footer {
    fn default() -> Self {
        Self::new("claude-3-sonnet".to_string())
    }
}
