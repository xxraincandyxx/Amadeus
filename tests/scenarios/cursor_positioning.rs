// @amadeus-header
// summary: Scenario testing support for cursor positioning.
// layer: test
// status: test-only
// feature_flags:
// - full
// provides:
// - module: tests::scenarios::cursor_positioning
// uses: none
// invariants:
// - Assertions stay aligned with current user-visible behavior.
// side_effects: none
// tests:
// - cmd: cargo test cursor_positioning --features full
// @end-amadeus-header

const VIEWPORT_HEIGHT: u16 = 3;

struct CursorState {
    row: u16,
    col: u16,
    operations: Vec<String>,
}

impl CursorState {
    fn new() -> Self {
        Self {
            row: 0,
            col: 0,
            operations: Vec::new(),
        }
    }

    fn move_up(&mut self, lines: u16) {
        self.row = self.row.saturating_sub(lines);
        self.operations.push(format!("MoveUp({})", lines));
    }

    fn move_to_column(&mut self, col: u16) {
        self.col = col;
        self.operations.push(format!("MoveToColumn({})", col));
    }

    fn clear(&mut self) {
        self.operations.push("Clear".to_string());
    }

    fn verify_moved_up(&self, expected_lines: u16) -> bool {
        self.operations
            .contains(&format!("MoveUp({})", expected_lines))
    }

    fn position(&self) -> (u16, u16) {
        (self.row, self.col)
    }
}

#[test]
fn test_first_flush_positions_cursor_above_viewport() {
    let mut cursor = CursorState::new();

    cursor.row = 10;
    cursor.move_up(VIEWPORT_HEIGHT);

    assert_eq!(
        cursor.row, 7,
        "First flush should move cursor up 3 lines (viewport height)"
    );
    assert!(cursor.verify_moved_up(VIEWPORT_HEIGHT));
}

#[test]
fn test_subsequent_flush_positions_cursor_above_viewport_and_content() {
    let mut cursor = CursorState::new();

    cursor.row = 15;
    cursor.move_up(VIEWPORT_HEIGHT);
    cursor.row = 12;

    let last_printed_height = 5u16;
    cursor.move_up(VIEWPORT_HEIGHT + last_printed_height);

    assert_eq!(
        cursor.row, 4,
        "Subsequent flush should move up viewport height + last printed height"
    );
    assert!(cursor.verify_moved_up(VIEWPORT_HEIGHT + last_printed_height));
}

#[test]
fn test_cursor_at_top_edge() {
    let mut cursor = CursorState::new();

    cursor.row = 0;
    cursor.move_up(VIEWPORT_HEIGHT);

    assert_eq!(cursor.row, 0, "Cursor at top should saturate at 0");
}

#[test]
fn test_cursor_movement_sequence() {
    let mut cursor = CursorState::new();

    cursor.row = 20;
    cursor.col = 50;

    cursor.move_up(VIEWPORT_HEIGHT);
    assert_eq!(cursor.position(), (17, 50));

    cursor.move_to_column(0);
    assert_eq!(cursor.position(), (17, 0));

    cursor.clear();
    assert!(cursor.operations.contains(&"Clear".to_string()));
}

#[test]
fn test_multiple_flushes_accumulate_movements() {
    let mut cursor = CursorState::new();

    cursor.row = 30;

    cursor.move_up(VIEWPORT_HEIGHT);
    let row_after_first = cursor.row;

    cursor.move_up(VIEWPORT_HEIGHT + 10);
    let row_after_second = cursor.row;

    assert!(row_after_second < row_after_first);
    assert_eq!(row_after_second, row_after_first - VIEWPORT_HEIGHT - 10);
}

#[test]
fn test_viewport_height_constant() {
    assert_eq!(VIEWPORT_HEIGHT, 3, "Viewport height should be 3 lines");
}

#[test]
fn test_cursor_saturating_subtraction() {
    let mut cursor = CursorState::new();

    cursor.row = 2;
    cursor.move_up(5);

    assert_eq!(cursor.row, 0, "Should saturate at 0, not underflow");
}

#[test]
fn test_operations_recorded_correctly() {
    let mut cursor = CursorState::new();

    cursor.move_up(3);
    cursor.move_to_column(0);
    cursor.clear();
    cursor.move_up(10);

    assert_eq!(cursor.operations.len(), 4);
    assert_eq!(cursor.operations[0], "MoveUp(3)");
    assert_eq!(cursor.operations[1], "MoveToColumn(0)");
    assert_eq!(cursor.operations[2], "Clear");
    assert_eq!(cursor.operations[3], "MoveUp(10)");
}

#[test]
fn test_first_vs_subsequent_flush_difference() {
    let mut first_flush_cursor = CursorState::new();
    let mut subsequent_cursor = CursorState::new();

    first_flush_cursor.row = 20;
    first_flush_cursor.move_up(VIEWPORT_HEIGHT);

    subsequent_cursor.row = 20;
    subsequent_cursor.move_up(VIEWPORT_HEIGHT + 5);

    assert_ne!(
        first_flush_cursor.row, subsequent_cursor.row,
        "First flush and subsequent flush should position differently"
    );

    assert_eq!(
        subsequent_cursor.row,
        first_flush_cursor.row - 5,
        "Subsequent flush should account for printed height"
    );
}

#[test]
fn test_cursor_returns_to_start_after_print() {
    let mut cursor = CursorState::new();

    cursor.row = 20;
    cursor.move_up(VIEWPORT_HEIGHT);

    let printed_height = 8u16;
    cursor.move_up(printed_height.saturating_sub(1));

    assert_eq!(
        cursor.row,
        20 - VIEWPORT_HEIGHT - printed_height + 1,
        "Cursor should return to start of content area"
    );
}
