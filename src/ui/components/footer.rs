use std::process::Command;

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
            },
            hide_cwd: false,
            hide_sandbox: false,
            hide_model: false,
            hide_context_percent: false,
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

    pub fn refresh_git_branch(&mut self) {
        self.info.git_branch = Self::detect_git_branch();
    }

    pub fn info(&self) -> &FooterInfo {
        &self.info
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        if area.width < 3 {
            return;
        }

        let colors = get_colors();
        let mut spans = Vec::new();

        let path_len = ((area.width as usize) / 4).max(20).min(50);

        if !self.hide_cwd {
            let display_path = Self::shorten_path(&self.info.cwd, path_len);
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
            spans.push(Span::raw(" │ "));

            let (status_text, status_color) = match &self.info.sandbox_status {
                SandboxStatus::None => ("no sandbox".to_string(), colors.status.error),
                SandboxStatus::Docker => ("docker".to_string(), colors.status.success),
                SandboxStatus::Seatbelt(profile) => (
                    format!("macOS Seatbelt ({})", profile),
                    colors.status.warning,
                ),
                SandboxStatus::Other(name) => (name.clone(), colors.status.success),
            };

            spans.push(Span::styled(status_text, Style::default().fg(status_color)));
        }

        let mut right_spans = Vec::new();

        if !self.hide_model {
            if self.info.is_mesh {
                right_spans.push(Span::styled(
                    " MESH ",
                    Style::default()
                        .fg(colors.background.primary)
                        .bg(colors.text.accent)
                        .add_modifier(Modifier::BOLD),
                ));
                right_spans.push(Span::raw(" "));
            }

            right_spans.push(Span::styled(
                format!("/model {}", self.info.model_name),
                Style::default().fg(colors.text.secondary),
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
