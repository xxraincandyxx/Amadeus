// @amadeus-header
// summary: TUI test support for comparison.
// layer: test
// status: test-only
// feature_flags:
// - full
// provides:
// - module: tests::tui::comparison
// - type: tests::tui::comparison::FrameDiff
// - type: tests::tui::comparison::StyleChange
// - type: tests::tui::comparison::CursorChange
// - type: tests::tui::comparison::FooterChanges
// - type: tests::tui::comparison::HeaderChanges
// - type: tests::tui::comparison::DiffSummary
// - fn: tests::tui::comparison::compare
// uses:
// - protocol: serde serialization
// invariants:
// - Assertions stay aligned with current user-visible behavior.
// side_effects: none
// tests:
// - cmd: cargo test comparison --features full
// @end-amadeus-header

//! Snapshot Comparison
//!
//! Compares TUI frames and produces human-readable diffs.

use super::capture::TuiFrameSnapshot;
use serde::{Deserialize, Serialize};

/// Result of comparing two snapshots
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrameDiff {
    pub expected_frame_id: u64,
    pub actual_frame_id: u64,
    pub added_cells: Vec<String>,
    pub removed_cells: Vec<String>,
    pub style_changes: Vec<StyleChange>,
    pub cursor_changes: Option<CursorChange>,
    pub footer_changes: FooterChanges,
    pub header_changes: HeaderChanges,
    pub summary: DiffSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StyleChange {
    pub x: u16,
    pub y: u16,
    pub field: String,
    pub expected: String,
    pub actual: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CursorChange {
    pub expected_x: u16,
    pub expected_y: u16,
    pub actual_x: u16,
    pub actual_y: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct FooterChanges {
    pub cwd: Option<String>,
    pub git_branch: Option<String>,
    pub model: Option<String>,
    pub context_pct: Option<String>,
    pub agent_name: Option<String>,
    pub is_mesh: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct HeaderChanges {
    pub session_label: Option<String>,
    pub streaming: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffSummary {
    pub total_cells_changed: usize,
    pub cells_added: usize,
    pub cells_removed: usize,
    pub style_changes: usize,
    pub cursor_moved: bool,
    pub footer_changed: bool,
    pub header_changed: bool,
}

impl DiffSummary {
    pub fn is_empty(&self) -> bool {
        self.total_cells_changed == 0
            && !self.cursor_moved
            && !self.footer_changed
            && !self.header_changed
    }
}

/// Compare two snapshots
pub fn compare(expected: &TuiFrameSnapshot, actual: &TuiFrameSnapshot) -> FrameDiff {
    let mut added_cells = Vec::new();
    let mut removed_cells = Vec::new();
    let mut style_changes = Vec::new();

    // Build maps for quick lookup
    let expected_map: std::collections::HashMap<(u16, u16), &_> =
        expected.cells.iter().map(|c| ((c.x, c.y), c)).collect();
    let actual_map: std::collections::HashMap<(u16, u16), &_> =
        actual.cells.iter().map(|c| ((c.x, c.y), c)).collect();

    // Find added and changed cells
    for (pos, cell) in &actual_map {
        if let Some(exp) = expected_map.get(pos) {
            // Check for style changes
            if exp.c != cell.c {
                removed_cells.push(format!("({}:{}): '{}' → '{}'", pos.0, pos.1, exp.c, cell.c));
                added_cells.push(format!("({}:{}): '{}'", pos.0, pos.1, cell.c));
            }
            if exp.fg != cell.fg {
                style_changes.push(StyleChange {
                    x: pos.0,
                    y: pos.1,
                    field: "fg".to_string(),
                    expected: exp.fg.clone(),
                    actual: cell.fg.clone(),
                });
            }
            if exp.bg != cell.bg {
                style_changes.push(StyleChange {
                    x: pos.0,
                    y: pos.1,
                    field: "bg".to_string(),
                    expected: exp.bg.clone(),
                    actual: cell.bg.clone(),
                });
            }
        } else {
            added_cells.push(format!("({}:{}): '{}'", pos.0, pos.1, cell.c));
        }
    }

    // Find removed cells
    for (pos, cell) in &expected_map {
        if !actual_map.contains_key(pos) {
            removed_cells.push(format!("({}:{}): '{}'", pos.0, pos.1, cell.c));
        }
    }

    // Cursor changes
    let cursor_moved = expected.cursor != actual.cursor;
    let cursor_changes = if cursor_moved {
        Some(CursorChange {
            expected_x: expected.cursor.x,
            expected_y: expected.cursor.y,
            actual_x: actual.cursor.x,
            actual_y: actual.cursor.y,
        })
    } else {
        None
    };

    // Footer changes
    let footer_changes = FooterChanges {
        cwd: diff_field(&expected.footer.cwd, &actual.footer.cwd),
        git_branch: diff_option_string(
            expected.footer.git_branch.clone(),
            actual.footer.git_branch.clone(),
        ),
        model: diff_field(&expected.footer.model, &actual.footer.model),
        context_pct: diff_u8(expected.footer.context_pct, actual.footer.context_pct),
        agent_name: diff_option_string(
            expected.footer.agent_name.clone(),
            actual.footer.agent_name.clone(),
        ),
        is_mesh: diff_bool(expected.footer.is_mesh, actual.footer.is_mesh),
    };
    let footer_changed = !is_footer_default(&footer_changes);

    // Header changes
    let header_changes = HeaderChanges {
        session_label: diff_field(&expected.header.session_label, &actual.header.session_label),
        streaming: diff_bool(expected.header.streaming, actual.header.streaming),
    };
    let header_changed = !is_header_default(&header_changes);

    // Compute summary values BEFORE moving vectors
    let total_cells_changed = added_cells.len() + removed_cells.len() + style_changes.len();
    let cells_added = added_cells.len();
    let cells_removed = removed_cells.len();
    let style_changes_count = style_changes.len();

    FrameDiff {
        expected_frame_id: expected.frame_id,
        actual_frame_id: actual.frame_id,
        added_cells,
        removed_cells,
        style_changes,
        cursor_changes,
        footer_changes,
        header_changes,
        summary: DiffSummary {
            total_cells_changed,
            cells_added,
            cells_removed,
            style_changes: style_changes_count,
            cursor_moved,
            footer_changed,
            header_changed,
        },
    }
}

fn diff_field<T: PartialEq + ToString>(expected: &T, actual: &T) -> Option<String> {
    if expected != actual {
        Some(format!("{} → {}", expected.to_string(), actual.to_string()))
    } else {
        None
    }
}

fn diff_u8(expected: u8, actual: u8) -> Option<String> {
    if expected != actual {
        Some(format!("{} → {}", expected, actual))
    } else {
        None
    }
}

fn diff_bool(expected: bool, actual: bool) -> Option<String> {
    if expected != actual {
        Some(format!("{} → {}", expected, actual))
    } else {
        None
    }
}

fn diff_option_string(expected: Option<String>, actual: Option<String>) -> Option<String> {
    if expected != actual {
        Some(format!("{:?} → {:?}", expected, actual))
    } else {
        None
    }
}

fn is_footer_default(f: &FooterChanges) -> bool {
    f.cwd.is_none()
        && f.git_branch.is_none()
        && f.model.is_none()
        && f.context_pct.is_none()
        && f.agent_name.is_none()
        && f.is_mesh.is_none()
}

fn is_header_default(h: &HeaderChanges) -> bool {
    h.session_label.is_none() && h.streaming.is_none()
}

/// Pretty-print a diff for terminal output
pub fn format_diff(diff: &FrameDiff) -> String {
    let mut output = String::new();

    output.push_str(&format!(
        "Frame diff: expected={}, actual={}\n",
        diff.expected_frame_id, diff.actual_frame_id
    ));
    output.push_str(&"=".repeat(60));
    output.push('\n');

    if !diff.added_cells.is_empty() {
        output.push_str(&format!("+ ADDED {} cells:\n", diff.added_cells.len()));
        for cell in &diff.added_cells[..diff.added_cells.len().min(10)] {
            output.push_str(&format!("  + {}\n", cell));
        }
        if diff.added_cells.len() > 10 {
            output.push_str(&format!("  ... and {} more\n", diff.added_cells.len() - 10));
        }
    }

    if !diff.removed_cells.is_empty() {
        output.push_str(&format!("- REMOVED {} cells:\n", diff.removed_cells.len()));
        for cell in &diff.removed_cells[..diff.removed_cells.len().min(10)] {
            output.push_str(&format!("  - {}\n", cell));
        }
        if diff.removed_cells.len() > 10 {
            output.push_str(&format!(
                "  ... and {} more\n",
                diff.removed_cells.len() - 10
            ));
        }
    }

    if !diff.style_changes.is_empty() {
        output.push_str(&format!("~ STYLE {} changes:\n", diff.style_changes.len()));
        for change in &diff.style_changes[..diff.style_changes.len().min(10)] {
            output.push_str(&format!(
                "  ~ ({}:{}) {}: {} → {}\n",
                change.x, change.y, change.field, change.expected, change.actual
            ));
        }
    }

    if let Some(ref cursor) = diff.cursor_changes {
        output.push_str(&format!(
            "→ CURSOR moved: ({}:{}) → ({}:{})\n",
            cursor.expected_x, cursor.expected_y, cursor.actual_x, cursor.actual_y
        ));
    }

    if let Some(ref footer) = diff.footer_changes.cwd {
        output.push_str(&format!("~ FOOTER cwd: {}\n", footer));
    }
    if let Some(ref footer) = diff.footer_changes.context_pct {
        output.push_str(&format!("~ FOOTER context: {}\n", footer));
    }

    output.push_str(&"-".repeat(60));
    output.push('\n');
    output.push_str(&format!(
        "Total changes: {}\n",
        diff.summary.total_cells_changed
    ));

    output
}

/// Snapshot comparison with assertion support
pub struct SnapshotComparison {
    expected: TuiFrameSnapshot,
    actual: TuiFrameSnapshot,
}

impl SnapshotComparison {
    pub fn new(expected: TuiFrameSnapshot, actual: TuiFrameSnapshot) -> Self {
        Self { expected, actual }
    }

    pub fn diff(&self) -> FrameDiff {
        compare(&self.expected, &self.actual)
    }

    pub fn assert_match(&self) -> Result<(), FrameDiff> {
        let diff = self.diff();
        if diff.summary.total_cells_changed == 0
            && !diff.summary.cursor_moved
            && !diff.summary.footer_changed
            && !diff.summary.header_changed
        {
            Ok(())
        } else {
            Err(diff)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::capture::{
        CellSnapshot, CursorSnapshot, FooterSnapshot, HeaderSnapshot, TuiCapture,
    };
    use super::*;

    fn make_snapshot(frame_id: u64, text: &str, cursor_x: u16) -> TuiFrameSnapshot {
        let cells: Vec<CellSnapshot> = text
            .chars()
            .enumerate()
            .map(|(i, c)| CellSnapshot {
                x: i as u16,
                y: 0,
                c,
                fg: "default".to_string(),
                bg: "default".to_string(),
                bold: false,
                underline: false,
                reverse: false,
            })
            .collect();

        TuiFrameSnapshot {
            version: "1.0.0".to_string(),
            session_id: "test".to_string(),
            frame_id,
            timestamp_ms: 0,
            width: 80,
            height: 24,
            cursor: CursorSnapshot {
                x: cursor_x,
                y: 0,
                visible: true,
            },
            footer: FooterSnapshot::default(),
            header: HeaderSnapshot::default(),
            cells,
            regions: Default::default(),
        }
    }

    #[test]
    fn test_identical_snapshots_match() {
        let snap1 = make_snapshot(0, "Hello", 5);
        let snap2 = make_snapshot(0, "Hello", 5);

        let comp = SnapshotComparison::new(snap1, snap2);
        assert!(comp.assert_match().is_ok());
    }

    #[test]
    fn test_different_text_detects_changes() {
        let snap1 = make_snapshot(0, "Hello", 5);
        let snap2 = make_snapshot(0, "World", 5);

        let comp = SnapshotComparison::new(snap1, snap2);
        let diff = comp.diff();

        assert!(!diff.added_cells.is_empty());
        assert!(!diff.removed_cells.is_empty());
    }

    #[test]
    fn test_cursor_change_detected() {
        let snap1 = make_snapshot(0, "Hello", 5);
        let snap2 = make_snapshot(0, "Hello", 3);

        let comp = SnapshotComparison::new(snap1, snap2);
        let diff = comp.diff();

        assert!(diff.cursor_changes.is_some());
    }
}
