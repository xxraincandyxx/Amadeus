// @amadeus-header
// summary: Unit tests for TUI input editing, history, completion, paste, and citation behavior.
// layer: test
// status: test-only
// feature_flags:
// - tui
// provides:
// - module: crate::ui::components::input::tests
// uses:
// - type: crate::ui::components::input::InputComponent
// - artifact: filesystem paths and files
// invariants:
// - Assertions preserve current input and completion behavior.
// side_effects:
// - Reads and writes temporary filesystem state.
// tests:
// - cmd: cargo test -p tui --all-features
// @end-amadeus-header

use super::*;
use std::fs;

use tempfile::tempdir;

fn temp_input() -> (tempfile::TempDir, InputComponent) {
    let temp = tempdir().expect("tempdir");
    let input = InputComponent::new_with_workdir(temp.path().to_path_buf());
    (temp, input)
}

#[test]
fn test_input_new() {
    let input = InputComponent::new();
    assert!(input.get_input().is_empty());
    assert!(input.history.is_empty());
}

#[test]
fn test_input_handle_char() {
    let mut input = InputComponent::new();
    input.handle_char('a');
    input.handle_char('b');
    input.handle_char('c');
    assert_eq!(input.get_input(), "abc");
}

#[test]
fn test_input_backspace() {
    let mut input = InputComponent::new();
    input.handle_char('a');
    input.handle_char('b');
    input.handle_backspace();
    assert_eq!(input.get_input(), "a");
}

#[test]
fn test_input_clear() {
    let mut input = InputComponent::new();
    input.handle_char('t');
    input.handle_char('e');
    input.handle_char('s');
    input.handle_char('t');
    input.clear();
    assert!(input.get_input().is_empty());
    assert_eq!(input.history.len(), 1);
}

#[test]
fn test_input_multiline() {
    let mut input = InputComponent::new();
    input.handle_char('a');
    input.insert_newline();
    input.handle_char('b');
    assert_eq!(input.get_input(), "a\nb");
}

#[test]
fn test_get_stats_empty() {
    let input = InputComponent::new();
    let (chars, lines) = input.get_stats();
    assert_eq!(chars, 0);
    assert_eq!(lines, 1); // Empty textarea has 1 line
}

#[test]
fn test_get_stats_single_line() {
    let mut input = InputComponent::new();
    for c in "hello world".chars() {
        input.handle_char(c);
    }
    let (chars, lines) = input.get_stats();
    assert_eq!(chars, 11);
    assert_eq!(lines, 1);
}

#[test]
fn test_get_stats_multiline() {
    let mut input = InputComponent::new();
    input.handle_char('a');
    input.insert_newline();
    input.handle_char('b');
    input.insert_newline();
    input.handle_char('c');
    let (chars, lines) = input.get_stats();
    assert_eq!(chars, 3);
    assert_eq!(lines, 3);
}

#[test]
fn test_get_stats_unicode() {
    let mut input = InputComponent::new();
    for c in "你好世界".chars() {
        input.handle_char(c);
    }
    let (chars, lines) = input.get_stats();
    assert_eq!(chars, 4);
    assert_eq!(lines, 1);
}

#[test]
fn test_height_minimum() {
    let input = InputComponent::new();
    // top + inner(1) + bottom + shortcuts = 4 when no status hint
    assert!(input.height() >= 4);
}

#[test]
fn test_height_grows_with_lines() {
    let mut input = InputComponent::new();
    let initial_height = input.height();

    for _ in 0..10 {
        input.insert_newline();
    }

    assert!(input.height() > initial_height);
}

#[test]
fn test_history_navigation() {
    let mut input = InputComponent::new();

    // Add some history
    input.handle_char('1');
    input.clear();
    input.handle_char('2');
    input.clear();
    input.handle_char('3');
    input.clear();

    assert_eq!(input.history.len(), 3);

    // Navigate up
    input.history_up();
    assert_eq!(input.get_input(), "3");

    input.history_up();
    assert_eq!(input.get_input(), "2");

    input.history_up();
    assert_eq!(input.get_input(), "1");

    // Navigate down
    input.history_down();
    assert_eq!(input.get_input(), "2");

    input.history_down();
    assert_eq!(input.get_input(), "3");

    input.history_down();
    // Should return to draft (empty)
    assert!(input.get_input().is_empty() || input.get_input() == "");
}

#[test]
fn test_cursor_movement() {
    let mut input = InputComponent::new();
    input.handle_char('a');
    input.handle_char('b');
    input.handle_char('c');

    input.move_cursor_left();
    input.handle_char('x');

    // Should have inserted 'x' before 'c'
    assert!(input.get_input().contains('x'));
}

#[test]
fn test_shortcuts_overlay_default_off() {
    let input = InputComponent::new();
    assert!(!input.shortcuts_visible);
}

#[test]
fn test_toggle_shortcuts_overlay() {
    let mut input = InputComponent::new();
    input.show_shortcuts(true);
    assert!(input.shortcuts_visible);
    input.show_shortcuts(false);
    assert!(!input.shortcuts_visible);
}

#[test]
fn test_render_shortcuts_overlay_lines() {
    let lines = InputComponent::shortcuts_lines(80);
    assert!(!lines.is_empty(), "should produce shortcut lines");
    assert!(
        lines.len() >= 3,
        "should have at least 3 rows for 3-column layout"
    );
}

#[test]
fn test_clear_empty_doesnt_add_to_history() {
    let mut input = InputComponent::new();
    input.clear(); // Clear empty input
    assert!(input.history.is_empty());

    input.handle_char(' ');
    input.clear(); // Clear whitespace-only
    assert!(input.history.is_empty());
}

#[test]
fn test_citation_completion_inserts_markdown_and_renders_token() {
    let (temp, mut input) = temp_input();
    fs::write(temp.path().join("reviewer.md"), "review").expect("write reviewer");
    input.citation_candidates =
        scan_workspace_citation_candidates(temp.path()).expect("scan citations");

    input.handle_char('@');
    input.handle_char('r');
    input.handle_char('e');
    input.handle_char('v');

    assert!(input.citation_completion_is_visible());
    assert_eq!(input.get_completion().as_deref(), Some("reviewer.md"));

    input.apply_completion();
    let expected_path = temp
        .path()
        .join("reviewer.md")
        .canonicalize()
        .expect("canonical reviewer");

    assert_eq!(
        input.get_input(),
        format!("[reviewer.md]({})", expected_path.display())
    );
    let rendered = input
        .visible_input_lines()
        .into_iter()
        .map(|line| line.to_string())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(rendered.contains("@reviewer.md"));
}

#[test]
fn test_citation_completion_reports_popup_height() {
    let (temp, mut input) = temp_input();
    fs::write(temp.path().join("reviewer.md"), "review").expect("write reviewer");
    input.citation_candidates =
        scan_workspace_citation_candidates(temp.path()).expect("scan citations");

    input.handle_char('@');
    input.handle_char('r');

    assert!(input.citation_completion_is_visible());
    assert!(input.completion_height() > 0);
}

#[test]
fn test_btw_dropup_reports_popup_height() {
    let (_temp, mut input) = temp_input();
    input.set_btw_dropup("/btw", "Usage: /btw", false);

    assert!(input.btw_dropup_is_visible());
    assert_eq!(input.completion_height(), 2);
}

#[test]
fn test_editing_clears_btw_dropup() {
    let (_temp, mut input) = temp_input();
    input.set_btw_dropup("/btw", "Usage: /btw", false);
    input.handle_char('a');

    assert!(!input.btw_dropup_is_visible());
}

#[test]
fn test_raw_at_query_remains_visible_before_citation_is_accepted() {
    let (_temp, mut input) = temp_input();

    input.handle_char('@');
    input.handle_char('r');
    input.handle_char('e');
    input.handle_char('v');

    let rendered = input
        .visible_input_lines()
        .into_iter()
        .map(|line| line.to_string())
        .collect::<Vec<_>>()
        .join("\n");

    assert!(rendered.contains("@rev"));
}

#[test]
fn test_path_paste_becomes_citation_markdown() {
    let (temp, mut input) = temp_input();
    let path = temp.path().join("reviewer.md");
    fs::write(&path, "review").expect("write reviewer");
    let expected_path = path.canonicalize().expect("canonical reviewer");

    input.handle_paste(&path.display().to_string());

    assert_eq!(
        input.get_input(),
        format!("[reviewer.md]({})", expected_path.display())
    );
}

#[test]
fn test_directory_paste_becomes_citation_markdown() {
    let (temp, mut input) = temp_input();
    let path = temp.path().join("docs");
    fs::create_dir_all(&path).expect("create docs dir");
    let expected_path = path.canonicalize().expect("canonical docs dir");

    input.handle_paste(&path.display().to_string());

    assert_eq!(
        input.get_input(),
        format!("[docs]({})", expected_path.display())
    );
}

#[test]
fn test_multiline_paste_is_inserted_as_text() {
    let (_temp, mut input) = temp_input();

    input.handle_paste("alpha\nbeta");

    assert_eq!(input.get_input(), "alpha\nbeta");
}
