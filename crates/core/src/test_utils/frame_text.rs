// @amadeus-header
// summary: Readable text rendering of a captured TUI frame for agent verification.
// layer: test
// status: test-only
// feature_flags:
// - test-utils
// provides:
// - fn: crate::test_utils::frame_text::render_frame_text
// - fn: crate::test_utils::frame_text::render_frames_filmstrip
// uses:
// - type: crate::test_utils::testflow::types::TuiFrameSnapshot
// invariants:
// - Output is deterministic for a given snapshot (no timing/random data).
// side_effects: none
// tests:
// - cmd: cargo test -p core --features test-utils frame_text
// @end-amadeus-header

//! Render captured TUI frames as plain text so a human or agent can read them.

use crate::test_utils::testflow::types::{TuiCellSnapshot, TuiFrameSnapshot};

/// Render a single frame as text: a header line, one line per terminal row
/// (trailing spaces trimmed), and a cursor line. Deterministic.
pub fn render_frame_text(snapshot: &TuiFrameSnapshot) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "# frame {} ({}x{} @{})\n",
        snapshot.frame_id, snapshot.width, snapshot.height, snapshot.timestamp_ms
    ));

    let mut grid: Vec<Vec<char>> = (0..snapshot.height)
        .map(|_| vec![' '; snapshot.width as usize])
        .collect();
    for cell in &snapshot.cells {
        if cell.y < snapshot.height && cell.x < snapshot.width {
            let sym = cell.symbol.chars().next().unwrap_or(' ');
            grid[cell.y as usize][cell.x as usize] = sym;
        }
    }
    for row in grid {
        let line: String = row.iter().collect();
        out.push_str(line.trim_end());
        out.push('\n');
    }

    if let Some(cursor) = &snapshot.cursor {
        if cursor.visible {
            out.push_str(&format!("^ cursor @ ({},{})\n", cursor.x, cursor.y));
        }
    }
    out
}

/// Render many frames as a filmstrip, frame separated by a blank line.
pub fn render_frames_filmstrip<'a, I>(snapshots: I) -> String
where
    I: IntoIterator<Item = &'a TuiFrameSnapshot>,
{
    let mut out = String::new();
    for snap in snapshots {
        out.push_str(&render_frame_text(snap));
        out.push('\n');
    }
    out
}

/// Build a snapshot from a simple text grid (test helper / fixture builder).
pub fn snapshot_from_text(frame_id: u64, rows: &[&str]) -> TuiFrameSnapshot {
    let height = rows.len() as u16;
    let width = rows.iter().map(|r| r.chars().count()).max().unwrap_or(0) as u16;
    let mut cells = Vec::new();
    for (y, row) in rows.iter().enumerate() {
        for (x, ch) in row.chars().enumerate() {
            if ch != ' ' {
                cells.push(TuiCellSnapshot {
                    x: x as u16,
                    y: y as u16,
                    symbol: ch.to_string(),
                    fg: "default".to_string(),
                    bg: "default".to_string(),
                    underline_color: "default".to_string(),
                    add_modifier: String::new(),
                    sub_modifier: String::new(),
                });
            }
        }
    }
    TuiFrameSnapshot {
        session_id: "test".to_string(),
        frame_id,
        timestamp_ms: 0,
        width,
        height,
        cursor: None,
        cells,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_rows_and_trims_trailing_spaces() {
        let snap = snapshot_from_text(3, &["hello", "hi   ", "world"]);
        let text = render_frame_text(&snap);
        assert!(text.contains("# frame 3"));
        assert!(text.contains("hello\n"));
        assert!(text.contains("\nhi\n"), "trailing spaces trimmed: {text:?}");
        assert!(text.ends_with("world\n"));
    }

    #[test]
    fn filmstrip_separates_frames() {
        let a = snapshot_from_text(0, &["a"]);
        let b = snapshot_from_text(1, &["b"]);
        let text = render_frames_filmstrip(&[a, b]);
        assert_eq!(text.matches("# frame").count(), 2);
    }
}
