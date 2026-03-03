use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::ui::get_colors;

/// Connection status indicator
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ConnectionStatus {
    Connected,
    Disconnected,
    Reconnecting,
}

/// Header component showing session info and status
pub struct Header {
    /// Session name/title
    session_name: Option<String>,
    /// Connection status
    connection_status: ConnectionStatus,
    /// Current mode indicator
    mode: String,
    /// Pending operations count
    pending_operations: usize,
}

impl Header {
    pub fn new() -> Self {
        Self {
            session_name: None,
            connection_status: ConnectionStatus::Connected,
            mode: "Input".to_string(),
            pending_operations: 0,
        }
    }

    /// Set the session name
    pub fn set_session_name(&mut self, name: impl Into<String>) {
        self.session_name = Some(name.into());
    }

    /// Set the connection status
    pub fn set_connection_status(&mut self, status: ConnectionStatus) {
        self.connection_status = status;
    }

    /// Set the current mode
    pub fn set_mode(&mut self, mode: impl Into<String>) {
        self.mode = mode.into();
    }

    /// Set pending operations count
    pub fn set_pending_operations(&mut self, count: usize) {
        self.pending_operations = count;
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        if area.width < 10 {
            return;
        }

        let colors = get_colors();
        let mut spans = Vec::new();

        // Title: Amadeus
        spans.push(Span::styled(
            " Amadeus ",
            Style::default()
                .fg(colors.background.primary)
                .bg(colors.text.accent)
                .add_modifier(Modifier::BOLD),
        ));

        // Separator
        spans.push(Span::styled(" ", Style::default().bg(colors.background.primary)));

        // Session name (if set)
        if let Some(ref name) = self.session_name {
            spans.push(Span::styled(
                format!("{} ", name),
                Style::default()
                    .fg(colors.text.primary)
                    .bg(colors.background.primary),
            ));
            spans.push(Span::styled("│", Style::default().fg(colors.ui.dark)));
            spans.push(Span::raw(" "));
        }

        // Mode indicator
        let mode_color = match self.mode.as_str() {
            "Normal" => colors.text.secondary,
            "Input" => colors.text.accent,
            "Approval" => colors.status.warning,
            _ => colors.text.primary,
        };
        spans.push(Span::styled(
            &self.mode,
            Style::default().fg(mode_color).add_modifier(Modifier::BOLD),
        ));

        // Connection status
        let (status_icon, status_color) = match self.connection_status {
            ConnectionStatus::Connected => ("●", colors.status.success),
            ConnectionStatus::Disconnected => ("○", colors.status.error),
            ConnectionStatus::Reconnecting => ("◐", colors.status.warning),
        };

        // Right side: pending ops and connection status
        let mut right_spans = Vec::new();

        // Pending operations indicator
        if self.pending_operations > 0 {
            right_spans.push(Span::styled(
                format!("{} pending", self.pending_operations),
                Style::default().fg(colors.status.warning),
            ));
            right_spans.push(Span::raw(" "));
            right_spans.push(Span::styled("│", Style::default().fg(colors.ui.dark)));
            right_spans.push(Span::raw(" "));
        }

        // Connection status
        right_spans.push(Span::styled(status_icon, Style::default().fg(status_color)));

        // Calculate widths for left/right balance
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

impl Default for Header {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_header_new() {
        let header = Header::new();
        assert!(header.session_name.is_none());
        assert_eq!(header.connection_status, ConnectionStatus::Connected);
        assert_eq!(header.mode, "Input");
        assert_eq!(header.pending_operations, 0);
    }

    #[test]
    fn test_header_set_session_name() {
        let mut header = Header::new();
        header.set_session_name("Test Session");
        assert_eq!(header.session_name, Some("Test Session".to_string()));
    }

    #[test]
    fn test_header_set_mode() {
        let mut header = Header::new();
        header.set_mode("Normal");
        assert_eq!(header.mode, "Normal");

        header.set_mode("Approval");
        assert_eq!(header.mode, "Approval");
    }

    #[test]
    fn test_header_set_connection_status() {
        let mut header = Header::new();

        header.set_connection_status(ConnectionStatus::Disconnected);
        assert_eq!(header.connection_status, ConnectionStatus::Disconnected);

        header.set_connection_status(ConnectionStatus::Reconnecting);
        assert_eq!(header.connection_status, ConnectionStatus::Reconnecting);

        header.set_connection_status(ConnectionStatus::Connected);
        assert_eq!(header.connection_status, ConnectionStatus::Connected);
    }

    #[test]
    fn test_header_set_pending_operations() {
        let mut header = Header::new();
        header.set_pending_operations(5);
        assert_eq!(header.pending_operations, 5);

        header.set_pending_operations(0);
        assert_eq!(header.pending_operations, 0);
    }

    #[test]
    fn test_connection_status_variants() {
        assert_eq!(ConnectionStatus::Connected, ConnectionStatus::Connected);
        assert_ne!(ConnectionStatus::Connected, ConnectionStatus::Disconnected);
        assert_ne!(ConnectionStatus::Connected, ConnectionStatus::Reconnecting);
    }

    #[test]
    fn test_header_default() {
        let header = Header::default();
        assert!(header.session_name.is_none());
    }
}
