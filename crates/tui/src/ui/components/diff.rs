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
// - library: similar line diffing
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
use similar::{ChangeTag, TextDiff};

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
        let lines = TextDiff::from_lines(old, new)
            .iter_all_changes()
            .map(|change| {
                let status = match change.tag() {
                    ChangeTag::Equal => DiffStatus::Unchanged,
                    ChangeTag::Insert => DiffStatus::Added,
                    ChangeTag::Delete => DiffStatus::Removed,
                };
                DiffLine {
                    old_line_num: change.old_index().map(|index| index + 1),
                    new_line_num: change.new_index().map(|index| index + 1),
                    content: change.value().trim_end_matches(['\r', '\n']).to_string(),
                    status,
                }
            })
            .collect();

        Self { lines }
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

    #[test]
    fn insertion_preserves_following_line_numbers() {
        let diff = DiffView::diff("alpha\nbeta\n", "alpha\ninserted\nbeta\n");

        assert_eq!(diff.lines.len(), 3);
        assert_eq!(diff.lines[0].status, DiffStatus::Unchanged);
        assert_eq!(diff.lines[1].status, DiffStatus::Added);
        assert_eq!(diff.lines[1].new_line_num, Some(2));
        assert_eq!(diff.lines[2].status, DiffStatus::Unchanged);
        assert_eq!(diff.lines[2].old_line_num, Some(2));
        assert_eq!(diff.lines[2].new_line_num, Some(3));
    }

    #[test]
    fn deletion_preserves_following_line_numbers() {
        let diff = DiffView::diff("alpha\nremoved\nbeta\n", "alpha\nbeta\n");

        assert_eq!(diff.lines.len(), 3);
        assert_eq!(diff.lines[1].status, DiffStatus::Removed);
        assert_eq!(diff.lines[1].old_line_num, Some(2));
        assert_eq!(diff.lines[2].status, DiffStatus::Unchanged);
        assert_eq!(diff.lines[2].old_line_num, Some(3));
        assert_eq!(diff.lines[2].new_line_num, Some(2));
    }
}
