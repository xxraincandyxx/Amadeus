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
// - format: JSON tool input
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
use serde_json::Value;
use similar::{ChangeTag, TextDiff};

/// A single line in a diff.
#[derive(Debug, Clone, PartialEq, Eq)]
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffView {
    /// The diff lines.
    pub lines: Vec<DiffLine>,
}

impl DiffView {
    /// Create a line diff from two strings.
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

    /// Create a display diff from supported file-tool input.
    pub fn from_tool_input(tool_name: &str, input: &Value) -> Option<Self> {
        let (old, new) = match tool_name {
            "write_file" => ("", input.get("content")?.as_str()?),
            "edit_file" => (
                input.get("old_text")?.as_str()?,
                input.get("new_text")?.as_str()?,
            ),
            _ => return None,
        };

        Some(Self::diff(old, new))
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

    #[test]
    fn write_file_input_is_all_additions() {
        let input = serde_json::json!({
            "path": "notes.txt",
            "content": "first\nsecond\n"
        });

        let diff = DiffView::from_tool_input("write_file", &input).expect("write diff");

        assert_eq!(diff.lines.len(), 2);
        assert!(diff
            .lines
            .iter()
            .all(|line| line.status == DiffStatus::Added));
    }

    #[test]
    fn edit_file_input_contains_removed_and_added_lines() {
        let input = serde_json::json!({
            "path": "notes.txt",
            "old_text": "before\n",
            "new_text": "after\n"
        });

        let diff = DiffView::from_tool_input("edit_file", &input).expect("edit diff");

        assert_eq!(diff.lines.len(), 2);
        assert_eq!(diff.lines[0].status, DiffStatus::Removed);
        assert_eq!(diff.lines[1].status, DiffStatus::Added);
    }

    #[test]
    fn unrelated_or_incomplete_tool_input_has_no_diff() {
        assert!(DiffView::from_tool_input("bash", &serde_json::json!({})).is_none());
        assert!(DiffView::from_tool_input("write_file", &serde_json::json!({})).is_none());
    }
}
