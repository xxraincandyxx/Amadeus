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

#[derive(Debug, Clone)]
pub struct FooterInfo {
    pub cwd: String,
    pub git_branch: Option<String>,
    pub sandbox_status: SandboxStatus,
    pub model_name: String,
    pub context_percent: u8,
    pub is_mesh: bool,
    /// Temporary status message (shown for a short time)
    pub status_message: Option<String>,
    /// Whether a background task is running
    pub is_background: bool,
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
    // Status message expiry
    status_message_expiry: Option<Instant>,
    // Session start time for duration tracking
    session_start: Instant,
    // Background mode indicator
    is_background: bool,
}

// Icons for footer elements (Unicode, no emojis)
const ICON_FOLDER: &str = "📂";
const ICON_GIT: &str = "⎇";
const ICON_SANDBOX: &str = "◫";
const ICON_CLOCK: &str = "◷";
const ICON_MODEL: &str = "◈";

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
                status_message: None,
                is_background: false,
            },
            hide_cwd: false,
            hide_sandbox: false,
            hide_model: false,
            hide_context_percent: false,
            status_message_expiry: None,
            session_start: Instant::now(),
            is_background: false,
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

    /// Set background mode indicator
    pub fn set_background(&mut self, is_background: bool) {
        self.is_background = is_background;
        self.info.is_background = is_background;
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

    /// Format session duration as MM:SS
    fn format_duration(&self) -> String {
        let elapsed = self.session_start.elapsed();
        let total_secs = elapsed.as_secs();
        let mins = total_secs / 60;
        let secs = total_secs % 60;
        format!("{:02}:{:02}", mins, secs)
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        if area.width < 3 {
            return;
        }

        let colors = get_colors();
        let mut spans = Vec::new();

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

        // Background mode indicator
        if self.is_background {
            spans.push(Span::styled(
                "⏳ BG ",
                Style::default()
                    .fg(colors.background.primary)
                    .bg(colors.status.warning)
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

        // CWD and git branch with icons
        let path_len = ((area.width as usize) / 4).max(15).min(40);

        if !self.hide_cwd {
            let display_path = Self::shorten_path(&self.info.cwd, path_len);
            spans.push(Span::styled(
                ICON_FOLDER,
                Style::default().fg(colors.text.secondary),
            ));
            spans.push(Span::raw(" "));
            spans.push(Span::styled(
                display_path,
                Style::default().fg(colors.text.primary),
            ));

            if let Some(ref branch) = self.info.git_branch {
                spans.push(Span::raw(" "));
                spans.push(Span::styled(
                    ICON_GIT,
                    Style::default().fg(colors.text.secondary),
                ));
                spans.push(Span::styled(
                    format!(" {}", branch),
                    Style::default().fg(colors.text.accent),
                ));
            }
        }

        if !self.hide_sandbox {
            spans.push(Span::raw(" "));
            spans.push(Span::styled("│", Style::default().fg(colors.ui.dark)));

            let (sandbox_text, sandbox_color) = match &self.info.sandbox_status {
                SandboxStatus::None => ("no sandbox".to_string(), colors.status.error),
                SandboxStatus::Docker => ("docker".to_string(), colors.status.success),
                SandboxStatus::Seatbelt(profile) => {
                    (format!("seatbelt:{}", profile), colors.status.warning)
                }
                SandboxStatus::Other(name) => (name.clone(), colors.status.success),
            };

            spans.push(Span::raw(" "));
            spans.push(Span::styled(
                ICON_SANDBOX,
                Style::default().fg(sandbox_color),
            ));
            spans.push(Span::raw(" "));
            spans.push(Span::styled(
                sandbox_text,
                Style::default().fg(sandbox_color),
            ));
        }

        // Right side: session duration, model, and context
        let mut right_spans = Vec::new();

        // Session duration
        right_spans.push(Span::raw(" "));
        right_spans.push(Span::styled("│", Style::default().fg(colors.ui.dark)));
        right_spans.push(Span::raw(" "));
        right_spans.push(Span::styled(
            ICON_CLOCK,
            Style::default().fg(colors.text.secondary),
        ));
        right_spans.push(Span::styled(
            format!(" {}", self.format_duration()),
            Style::default().fg(colors.text.secondary),
        ));

        if !self.hide_model {
            right_spans.push(Span::raw(" "));
            right_spans.push(Span::styled("│", Style::default().fg(colors.ui.dark)));
            right_spans.push(Span::raw(" "));
            right_spans.push(Span::styled(
                ICON_MODEL,
                Style::default().fg(colors.text.accent),
            ));
            right_spans.push(Span::raw(" "));
            right_spans.push(Span::styled(
                self.info.model_name.clone(),
                Style::default().fg(colors.text.accent),
            ));

            if !self.hide_context_percent && self.info.context_percent > 0 {
                let (bar_color, percent_color) = if self.info.context_percent >= 90 {
                    (colors.status.error, colors.status.error)
                } else if self.info.context_percent >= 70 {
                    (colors.status.warning, colors.status.warning)
                } else {
                    (colors.status.success, colors.text.secondary)
                };

                // Visual progress bar [████░░░░] 45%
                let bar_width = 8;
                let filled = ((self.info.context_percent as usize) * bar_width) / 100;
                let empty = bar_width - filled;
                let bar = format!("[{}{}]", "█".repeat(filled), "░".repeat(empty));

                right_spans.push(Span::raw(" "));
                right_spans.push(Span::styled(bar, Style::default().fg(bar_color)));
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_footer_new() {
        let footer = Footer::new("test-model".to_string());
        assert_eq!(footer.info.model_name, "test-model");
        assert_eq!(footer.info.context_percent, 0);
        assert!(!footer.info.is_mesh);
    }

    #[test]
    fn test_footer_set_context_percent() {
        let mut footer = Footer::new("test".to_string());

        footer.set_context_percent(50);
        assert_eq!(footer.info.context_percent, 50);

        footer.set_context_percent(100);
        assert_eq!(footer.info.context_percent, 100);

        // Should clamp to 100
        footer.set_context_percent(150);
        assert_eq!(footer.info.context_percent, 100);
    }

    #[test]
    fn test_footer_set_model_name() {
        let mut footer = Footer::new("old-model".to_string());
        footer.set_model_name("new-model".to_string());
        assert_eq!(footer.info.model_name, "new-model");
    }

    #[test]
    fn test_footer_set_mesh() {
        let mut footer = Footer::new("test".to_string());
        assert!(!footer.info.is_mesh);

        footer.set_mesh(true);
        assert!(footer.info.is_mesh);

        footer.set_mesh(false);
        assert!(!footer.info.is_mesh);
    }

    #[test]
    fn test_footer_status_message() {
        let mut footer = Footer::new("test".to_string());

        footer.set_status_message("Test message");
        assert_eq!(footer.info.status_message, Some("Test message".to_string()));
        assert!(footer.status_message_expiry.is_some());

        footer.clear_status_message();
        assert!(footer.info.status_message.is_none());
        assert!(footer.status_message_expiry.is_none());
    }

    #[test]
    fn test_footer_format_duration() {
        let footer = Footer::new("test".to_string());
        let duration = footer.format_duration();

        // Duration should be in MM:SS format
        assert!(duration.contains(':'));
        assert_eq!(duration.len(), 5); // "00:00" format
    }

    #[test]
    fn test_sandbox_status_default() {
        let status = SandboxStatus::default();
        assert_eq!(status, SandboxStatus::None);
    }

    #[test]
    fn test_sandbox_status_variants() {
        let none = SandboxStatus::None;
        let docker = SandboxStatus::Docker;
        let seatbelt = SandboxStatus::Seatbelt("default".to_string());
        let other = SandboxStatus::Other("custom".to_string());

        assert_ne!(none, docker);
        assert_ne!(docker, seatbelt);
        assert_ne!(seatbelt, other);
    }

    #[test]
    fn test_footer_info_clone() {
        let info = FooterInfo {
            cwd: "/test/path".to_string(),
            git_branch: Some("main".to_string()),
            sandbox_status: SandboxStatus::Docker,
            model_name: "test-model".to_string(),
            context_percent: 50,
            is_mesh: true,
            status_message: Some("test".to_string()),
            is_background: false,
        };

        let cloned = info.clone();
        assert_eq!(info.cwd, cloned.cwd);
        assert_eq!(info.git_branch, cloned.git_branch);
        assert_eq!(info.context_percent, cloned.context_percent);
        assert_eq!(info.is_background, cloned.is_background);
    }

    #[test]
    fn test_footer_default() {
        let footer = Footer::default();
        assert_eq!(footer.info.model_name, "claude-3-sonnet");
    }

    #[test]
    fn test_context_percent_clamping() {
        let mut footer = Footer::new("test".to_string());

        // Test boundary values
        footer.set_context_percent(0);
        assert_eq!(footer.info.context_percent, 0);

        footer.set_context_percent(100);
        assert_eq!(footer.info.context_percent, 100);

        // Test over-max clamping
        footer.set_context_percent(255);
        assert_eq!(footer.info.context_percent, 100);
    }
}
