// @amadeus-header
// summary: TUI component implementation for diff.
// layer: ui
// status: active
// feature_flags:
// - tui
// provides:
// - module: crate::ui::components::diff
// - type: crate::ui::components::diff::DiffLine
// - type: crate::ui::components::diff::DiffStatus
// - type: crate::ui::components::diff::DiffView
// uses:
// - runtime: ratatui terminal rendering
// invariants:
// - Listed interfaces stay aligned with the implementation in this file.
// side_effects: none
// tests:
// - tests/tui_snapshot_test.rs
// @end-amadeus-header

//! # Diff View Component
//!
//! Renders diffs for the `edit_file` tool output.

use ratatui::{
    style::{Color, Style},
    text::Span,
};

/// A single line in a diff.
#[derive(Debug, Clone)]
pub struct DiffLine {
    /// The line number in the old file (or None if new file).
    pub old_line_num: Option<usize>,
    /// The line number in the new file (or None if removed).
    pub new_line_num: Option<usize>,
    /// The content of the line.
    pub content: String,
    /// Whether this line was added, removed, or unchanged.
    pub status: DiffStatus,
}

/// Type of diff line change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffStatus {
    /// Line was unchanged.
    Unchanged,
    /// Line was added.
    Added,
    /// Line was removed.
    Removed,
}

/// Renders a diff between old and new content.
#[derive(Debug, Clone)]
pub struct DiffView {
    /// The diff lines.
    pub lines: Vec<DiffLine>,
}

impl DiffView {
    /// Create a simple diff from two strings.
    pub fn diff(old: &str, new: &str) -> Self {
        let old_lines: Vec<&str> = old.lines().collect();
        let new_lines: Vec<&str> = new.lines().collect();

        let mut diff_lines = Vec::new();

        // Simple line-by-line diff
        let max_len = old_lines.len().max(new_lines.len());
        for i in 0..max_len {
            let old_line = old_lines.get(i);
            let new_line = new_lines.get(i);

            match (old_line, new_line) {
                (Some(o), Some(n)) if o == n => {
                    diff_lines.push(DiffLine {
                        old_line_num: Some(i + 1),
                        new_line_num: Some(i + 1),
                        content: o.to_string(),
                        status: DiffStatus::Unchanged,
                    });
                }
                (Some(o), Some(n)) => {
                    diff_lines.push(DiffLine {
                        old_line_num: Some(i + 1),
                        new_line_num: None,
                        content: o.to_string(),
                        status: DiffStatus::Removed,
                    });
                    diff_lines.push(DiffLine {
                        old_line_num: None,
                        new_line_num: Some(i + 1),
                        content: n.to_string(),
                        status: DiffStatus::Added,
                    });
                }
                (Some(o), None) => {
                    diff_lines.push(DiffLine {
                        old_line_num: Some(i + 1),
                        new_line_num: None,
                        content: o.to_string(),
                        status: DiffStatus::Removed,
                    });
                }
                (None, Some(n)) => {
                    diff_lines.push(DiffLine {
                        old_line_num: None,
                        new_line_num: Some(i + 1),
                        content: n.to_string(),
                        status: DiffStatus::Added,
                    });
                }
                (None, None) => {}
            }
        }

        Self { lines: diff_lines }
    }

    /// Render the diff as styled spans.
    pub fn render(&self) -> Vec<Span<'_>> {
        let mut spans = Vec::new();

        for line in &self.lines {
            let styled_span = match line.status {
                DiffStatus::Added => Span::styled(
                    format!("+ {:4}{}\n", line.new_line_num.unwrap_or(0), line.content),
                    Style::default().fg(Color::Green),
                ),
                DiffStatus::Removed => Span::styled(
                    format!("- {:4}{}\n", line.old_line_num.unwrap_or(0), line.content),
                    Style::default().fg(Color::Red),
                ),
                DiffStatus::Unchanged => Span::styled(
                    format!("  {:4}{}\n", line.new_line_num.unwrap_or(0), line.content),
                    Style::default(),
                ),
            };
            spans.push(styled_span);
        }

        spans
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diff_basic() {
        let old = "Hello\nWorld\nFoo";
        let new = "Hello\nRust\nFoo\nBar";

        let diff = DiffView::diff(old, new);

        // Should have: unchanged Hello, removed World, added Rust, unchanged Foo, added Bar
        assert!(diff.lines.iter().any(|l| l.status == DiffStatus::Removed));
        assert!(diff.lines.iter().any(|l| l.status == DiffStatus::Added));
    }
}
