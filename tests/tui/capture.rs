//! TUI Frame Capture
//!
//! Captures complete terminal state including all cells, cursor, and styling.

use serde::{Deserialize, Serialize};
use std::time::Instant;

/// A complete snapshot of the terminal frame
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TuiFrameSnapshot {
    pub version: String,
    pub session_id: String,
    pub frame_id: u64,
    pub timestamp_ms: u64,
    pub width: u16,
    pub height: u16,
    pub cursor: CursorSnapshot,
    pub footer: FooterSnapshot,
    pub header: HeaderSnapshot,
    pub cells: Vec<CellSnapshot>,
    pub regions: RegionsSnapshot,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct CursorSnapshot {
    pub x: u16,
    pub y: u16,
    pub visible: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FooterSnapshot {
    pub cwd: String,
    pub git_branch: Option<String>,
    pub model: String,
    pub context_pct: u8,
    pub agent_name: Option<String>,
    pub is_mesh: bool,
    pub sandbox_status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HeaderSnapshot {
    pub session_label: String,
    pub streaming: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CellSnapshot {
    pub x: u16,
    pub y: u16,
    pub c: char,
    pub fg: String,
    pub bg: String,
    pub bold: bool,
    pub underline: bool,
    pub reverse: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RegionsSnapshot {
    pub input: InputRegionSnapshot,
    pub messages: Vec<MessageRegionSnapshot>,
    pub tool_panel: ToolPanelSnapshot,
    pub sidebar: SidebarSnapshot,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct InputRegionSnapshot {
    pub y: u16,
    pub text: String,
    pub cursor_x: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageRegionSnapshot {
    pub role: String,
    pub content: String,
    pub y_start: u16,
    pub y_end: u16,
    pub streaming: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ToolPanelSnapshot {
    pub active: bool,
    pub tool_name: Option<String>,
    pub status: String,
    pub output_preview: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SidebarSnapshot {
    pub visible: bool,
    pub kind: String,
}

/// Frame capture utility
pub struct TuiCapture {
    session_id: String,
    frame_counter: u64,
    start_time: Instant,
}

impl TuiCapture {
    pub fn new(session_id: impl Into<String>) -> Self {
        Self {
            session_id: session_id.into(),
            frame_counter: 0,
            start_time: Instant::now(),
        }
    }

    /// Capture current frame as structured snapshot
    pub fn capture(
        &mut self,
        width: u16,
        height: u16,
        cells: &[Vec<CellSnapshot>],
    ) -> TuiFrameSnapshot {
        let frame = TuiFrameSnapshot {
            version: "1.0.0".to_string(),
            session_id: self.session_id.clone(),
            frame_id: self.frame_counter,
            timestamp_ms: self.start_time.elapsed().as_millis() as u64,
            width,
            height,
            cursor: CursorSnapshot::default(),
            footer: FooterSnapshot::default(),
            header: HeaderSnapshot::default(),
            cells: cells.iter().flatten().cloned().collect(),
            regions: RegionsSnapshot::default(),
        };

        self.frame_counter += 1;
        frame
    }

    /// Create a minimal snapshot for testing
    pub fn minimal(&mut self) -> TuiFrameSnapshot {
        self.capture(
            120,
            40,
            &[vec![CellSnapshot {
                x: 0,
                y: 0,
                c: ' ',
                fg: "default".to_string(),
                bg: "default".to_string(),
                bold: false,
                underline: false,
                reverse: false,
            }]],
        )
    }
}

impl Default for TuiCapture {
    fn default() -> Self {
        Self::new("test_session")
    }
}

impl TuiFrameSnapshot {
    /// Format as readable terminal-like output
    pub fn to_terminal_view(&self) -> String {
        let mut output = String::new();

        // Header
        output.push_str(&format!(
            "╔══ Frame {} ════════════════════════════════════════════╗\n",
            self.frame_id
        ));
        output.push_str(&format!(
            "║ Session: {} | {}x{} | {:3}ms ║\n",
            self.session_id, self.width, self.height, self.timestamp_ms
        ));
        output.push_str("╠══════════════════════════════════════════════════════════╣\n");

        // Header region
        if !self.header.session_label.is_empty() || self.header.streaming {
            output.push_str(&format!(
                "║ Header: label='{}' streaming={} ║\n",
                self.header.session_label, self.header.streaming
            ));
        }

        // Cells as ASCII art (simplified view)
        output.push_str("║ Cells (first 10x40):                               ║\n");
        for y in 0..10.min(self.height) {
            let mut row = String::from("║ ");
            for x in 0..40.min(self.width) {
                let cell = self.cells.iter().find(|c| c.x == x && c.y == y);
                row.push(cell.map(|c| c.c).unwrap_or(' '));
            }
            row.push_str(" ║");
            output.push_str(&row);
            output.push('\n');
        }

        // Cursor
        output.push_str(&format!(
            "║ Cursor: ({}, {}) visible={}                    ║\n",
            self.cursor.x, self.cursor.y, self.cursor.visible
        ));

        // Footer
        output.push_str("╠══════════════════════════════════════════════════════════╣\n");
        output.push_str(&format!(
            "║ Footer: {} [{}] {}%",
            self.footer.model, self.footer.cwd, self.footer.context_pct
        ));
        if self.footer.is_mesh {
            output.push_str(" MESH");
        }
        if let Some(ref name) = self.footer.agent_name {
            output.push_str(&format!(" [{}]", name));
        }
        output.push_str(" ║\n");
        output.push_str("\n╚");
        for _ in 0..58 {
            output.push('═');
        }
        output.push_str("╝\n");

        output
    }
}

/// Convert raw terminal buffer to cell snapshots
pub fn buffer_to_cells(buf: &[Vec<(char, &str, &str)>]) -> Vec<CellSnapshot> {
    let mut cells = Vec::new();
    for (y, row) in buf.iter().enumerate() {
        for (x, (c, fg, bg)) in row.iter().enumerate() {
            cells.push(CellSnapshot {
                x: x as u16,
                y: y as u16,
                c: *c,
                fg: fg.to_string(),
                bg: bg.to_string(),
                bold: false,
                underline: false,
                reverse: false,
            });
        }
    }
    cells
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capture_basic() {
        let mut capture = TuiCapture::new("test");
        let snapshot = capture.capture(
            80,
            24,
            &[vec![CellSnapshot {
                x: 0,
                y: 0,
                c: 'H',
                fg: "white".to_string(),
                bg: "black".to_string(),
                bold: true,
                underline: false,
                reverse: false,
            }]],
        );

        assert_eq!(snapshot.session_id, "test");
        assert_eq!(snapshot.frame_id, 0);
        assert_eq!(snapshot.width, 80);
        assert_eq!(snapshot.height, 24);
        assert_eq!(snapshot.cells.len(), 1);
        assert_eq!(snapshot.cells[0].c, 'H');
    }

    #[test]
    fn test_frame_counter_increments() {
        let mut capture = TuiCapture::new("test");
        capture.capture(80, 24, &[]);
        capture.capture(80, 24, &[]);
        let third = capture.capture(80, 24, &[]);

        assert_eq!(third.frame_id, 2);
    }
}
