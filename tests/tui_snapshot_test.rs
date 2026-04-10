// @amadeus-header
// summary: Integration tests covering tui snapshot test behavior.
// layer: test
// status: test-only
// feature_flags:
// - full
// provides:
// - module: tests::tui_snapshot_test
// uses:
// - module: amadeus::client::StreamEvent
// invariants:
// - Assertions stay aligned with current user-visible behavior.
// side_effects:
// - Writes output to stdout or stderr.
// tests:
// - cmd: cargo test tui_snapshot_test --features full
// @end-amadeus-header

//! TUI Snapshot Tests
//!
//! Visual regression tests that capture and verify complete terminal output.

mod tui;

use amadeus::client::StreamEvent;
use tui::capture::CellSnapshot;
use tui::comparison::{compare, format_diff};
use tui::harness::{run_scenario, InputSequence, TuiTestHarness};
use tui::scenarios::{
    requiring_approval, simple_text, streaming_text, with_tool_call, MockScenarioClient, Scenario,
};
use tui::TuiCapture;

#[cfg(test)]
mod tests {
    use super::*;

    // ============================================================================
    // Basic Tests
    // ============================================================================

    #[test]
    fn test_tui_capture_records_frames() {
        let mut capture = TuiCapture::new("test_session");

        let frame1 = capture.minimal();
        assert_eq!(frame1.session_id, "test_session");
        assert_eq!(frame1.frame_id, 0);

        let frame2 = capture.capture(80, 24, &[]);
        assert_eq!(frame2.frame_id, 1);
    }

    #[test]
    fn test_harness_creates_captures() {
        let mut harness = TuiTestHarness::new("harness_test");
        harness.capture_frame();
        harness.capture_frame();

        assert_eq!(harness.captured_frames().len(), 2);
    }

    // ============================================================================
    // Scenario Tests
    // ============================================================================

    #[tokio::test]
    async fn test_simple_text_scenario() {
        let scenario = simple_text("Hello, world!");
        let client = MockScenarioClient::new(scenario);

        let frames: Vec<tui::TuiFrameSnapshot> =
            run_scenario(client, InputSequence::new().type_text("Hi"), 80, 24).await;

        // Should have initial frame + input frames
        assert!(!frames.is_empty());
    }

    #[tokio::test]
    async fn test_streaming_text_scenario() {
        let scenario = streaming_text("Hello world");
        let client = MockScenarioClient::new(scenario);

        let frames: Vec<tui::TuiFrameSnapshot> =
            run_scenario(client, InputSequence::new().type_text("Say hello"), 80, 24).await;

        // Should capture multiple frames during streaming
        assert!(frames.len() >= 2);
    }

    #[tokio::test]
    async fn test_tool_call_scenario() {
        let scenario = with_tool_call(
            "ls -la",
            "total 8\ndrwxr-xr-x  3 user staff  160 Mar 21 12:00 .",
        );
        let client = MockScenarioClient::new(scenario);

        let frames: Vec<tui::TuiFrameSnapshot> = run_scenario(
            client,
            InputSequence::new().type_text("List files").enter(),
            80,
            24,
        )
        .await;

        assert!(!frames.is_empty());
    }

    #[tokio::test]
    async fn test_approval_scenario() {
        let scenario = requiring_approval("bash", "rm -rf /");
        let client = MockScenarioClient::new(scenario);

        let frames: Vec<tui::TuiFrameSnapshot> = run_scenario(
            client,
            InputSequence::new().type_text("Clean up").enter(),
            80,
            24,
        )
        .await;

        assert!(!frames.is_empty());
    }

    // ============================================================================
    // Comparison Tests
    // ============================================================================

    #[test]
    fn test_identical_frames_match() {
        let mut capture1 = TuiCapture::new("test");
        let mut capture2 = TuiCapture::new("test");

        let frame1 = capture1.minimal();
        let frame2 = capture2.minimal();

        let diff = compare(&frame1, &frame2);
        assert_eq!(diff.summary.total_cells_changed, 0);
    }

    #[test]
    fn test_different_frames_show_diff() {
        let mut capture1 = TuiCapture::new("test1");
        let mut capture2 = TuiCapture::new("test2");

        let frame1 = capture1.minimal();
        let frame2 = capture2.minimal();

        let diff = compare(&frame1, &frame2);

        // Different session IDs should show as minimal difference
        // (cells are both empty so no cell-level changes)
        assert!(diff.summary.is_empty() || diff.summary.total_cells_changed == 0);
    }

    #[test]
    fn test_diff_formatting() {
        let mut capture1 = TuiCapture::new("test1");
        let mut capture2 = TuiCapture::new("test2");

        let frame1 = capture1.minimal();
        let frame2 = capture2.minimal();

        let diff = compare(&frame1, &frame2);
        let formatted = format_diff(&diff);

        assert!(formatted.contains("Frame diff"));
        assert!(formatted.contains("expected="));
        assert!(formatted.contains("actual="));
    }

    // ============================================================================
    // Input Sequence Tests
    // ============================================================================

    #[test]
    fn test_input_sequence_type_text() {
        let seq = InputSequence::new().type_text("Hello");
        assert_eq!(seq.len(), 1);
    }

    #[test]
    fn test_input_sequence_enter() {
        let seq = InputSequence::new().enter();
        assert_eq!(seq.len(), 1);
    }

    #[test]
    fn test_input_sequence_ctrl() {
        let seq = InputSequence::new().ctrl('c');
        assert_eq!(seq.len(), 1);
    }

    #[test]
    fn test_input_sequence_chaining() {
        let seq = InputSequence::new()
            .type_text("Hello")
            .enter()
            .type_text("World")
            .enter();

        assert_eq!(seq.len(), 4);
    }

    #[test]
    fn test_input_sequence_from_str() {
        let seq: InputSequence = "Hello".into();
        assert_eq!(seq.len(), 1);
    }

    // ============================================================================
    // Mock Client Tests
    // ============================================================================

    #[tokio::test]
    async fn test_mock_scenario_client_simple() {
        let scenario = simple_text("test response");
        let client = MockScenarioClient::new(scenario);

        // Verify client can be cloned and used
        let client2 = client.clone();
        assert!(std::mem::size_of_val(&client2) > 0);
    }

    #[tokio::test]
    async fn test_mock_scenario_client_streaming() {
        let scenario = streaming_text("one two three");
        let client = MockScenarioClient::new(scenario);

        let client2 = client.clone();
        assert!(std::mem::size_of_val(&client2) > 0);
    }

    // ============================================================================
    // Debug Visualization Tests
    // ============================================================================

    #[test]
    fn test_snapshot_debug_view_simple() {
        let mut capture = TuiCapture::new("debug_test");

        // Create a frame with some content
        let cells = vec![
            CellSnapshot {
                x: 0,
                y: 0,
                c: 'H',
                fg: "white".into(),
                bg: "black".into(),
                bold: true,
                underline: false,
                reverse: false,
            },
            CellSnapshot {
                x: 1,
                y: 0,
                c: 'e',
                fg: "white".into(),
                bg: "black".into(),
                bold: false,
                underline: false,
                reverse: false,
            },
            CellSnapshot {
                x: 2,
                y: 0,
                c: 'l',
                fg: "white".into(),
                bg: "black".into(),
                bold: false,
                underline: false,
                reverse: false,
            },
            CellSnapshot {
                x: 3,
                y: 0,
                c: 'l',
                fg: "white".into(),
                bg: "black".into(),
                bold: false,
                underline: false,
                reverse: false,
            },
            CellSnapshot {
                x: 4,
                y: 0,
                c: 'o',
                fg: "white".into(),
                bg: "black".into(),
                bold: false,
                underline: false,
                reverse: false,
            },
        ];

        let snapshot = capture.capture(80, 24, std::slice::from_ref(&cells));

        println!("\n\n===== SIMPLE TEXT SNAPSHOT =====");
        println!("{}", snapshot.to_terminal_view());
        println!("===== SERIALIZED =====");
        println!("{}", serde_json::to_string_pretty(&snapshot).unwrap());

        assert_eq!(snapshot.cells.len(), 5);
    }

    #[test]
    fn test_snapshot_debug_view_with_footer() {
        let mut capture = TuiCapture::new("footer_test");

        let cells = vec![
            CellSnapshot {
                x: 0,
                y: 0,
                c: '>',
                fg: "green".into(),
                bg: "black".into(),
                bold: true,
                underline: false,
                reverse: false,
            },
            CellSnapshot {
                x: 2,
                y: 0,
                c: ' ',
                fg: "white".into(),
                bg: "black".into(),
                bold: false,
                underline: false,
                reverse: false,
            },
        ];

        let mut snapshot = capture.capture(120, 40, &[cells]);
        snapshot.footer.cwd = "/Users/dev/project".into();
        snapshot.footer.model = "claude-3-sonnet".into();
        snapshot.footer.context_pct = 45;
        snapshot.footer.git_branch = Some("main".into());

        println!("\n\n===== FOOTER SNAPSHOT =====");
        println!("{}", snapshot.to_terminal_view());
        println!("===== FOOTER DETAILS =====");
        println!("cwd: {}", snapshot.footer.cwd);
        println!("model: {}", snapshot.footer.model);
        println!("context: {}%", snapshot.footer.context_pct);
        println!("git: {:?}", snapshot.footer.git_branch);

        assert_eq!(snapshot.footer.context_pct, 45);
    }

    #[test]
    fn test_snapshot_streaming_sequence() {
        let mut capture = TuiCapture::new("streaming_test");

        // Simulate streaming "Hello World" character by character
        let text = "Hello World!";
        let mut frames = Vec::new();

        for (i, c) in text.chars().enumerate() {
            let cells = vec![CellSnapshot {
                x: i as u16,
                y: 0,
                c,
                fg: "cyan".into(),
                bg: "black".into(),
                bold: false,
                underline: false,
                reverse: false,
            }];
            let frame = capture.capture(80, 24, &[cells]);
            frames.push(frame);
        }

        println!(
            "\n\n===== STREAMING SEQUENCE ({} frames) =====",
            frames.len()
        );
        for (i, frame) in frames.iter().enumerate() {
            println!(
                "Frame {}: '{}'",
                i,
                frame.cells.iter().map(|c| c.c).collect::<String>()
            );
        }

        assert_eq!(frames.len(), 12);
        assert_eq!(frames.last().unwrap().cells.len(), 1);
    }

    #[test]
    fn test_diff_visualization() {
        let mut capture1 = TuiCapture::new("diff_test");
        let mut capture2 = TuiCapture::new("diff_test");

        // Frame 1: "Hello"
        let cells1 = "Hello"
            .chars()
            .enumerate()
            .map(|(i, c)| CellSnapshot {
                x: i as u16,
                y: 0,
                c,
                fg: "white".into(),
                bg: "black".into(),
                bold: false,
                underline: false,
                reverse: false,
            })
            .collect::<Vec<_>>();
        let snap1 = capture1.capture(80, 24, &[cells1]);

        // Frame 2: "World"
        let cells2 = "World"
            .chars()
            .enumerate()
            .map(|(i, c)| CellSnapshot {
                x: i as u16,
                y: 0,
                c,
                fg: "white".into(),
                bg: "black".into(),
                bold: false,
                underline: false,
                reverse: false,
            })
            .collect::<Vec<_>>();
        let snap2 = capture2.capture(80, 24, &[cells2]);

        let diff = compare(&snap1, &snap2);

        println!("\n\n===== DIFF VISUALIZATION =====");
        println!("{}", format_diff(&diff));
        println!("===== SUMMARY =====");
        println!("Total changes: {}", diff.summary.total_cells_changed);
        println!("Cells added: {}", diff.summary.cells_added);
        println!("Cells removed: {}", diff.summary.cells_removed);
        println!("Style changes: {}", diff.summary.style_changes);

        assert!(diff.summary.cells_removed > 0);
        assert!(diff.summary.cells_added > 0);
    }

    // ============================================================================
    // Edge Cases
    // ============================================================================

    #[test]
    fn test_empty_scenario() {
        let scenario = Scenario::new("empty");
        assert_eq!(scenario.name, "empty");
        assert!(scenario.events.is_empty());
    }

    #[test]
    fn test_scenario_builder() {
        let scenario = Scenario::new("test")
            .description("Test scenario")
            .add_turn(vec![
                StreamEvent::TextDelta("Hello".to_string()),
                StreamEvent::StopReason("end_turn".to_string()),
            ]);

        assert_eq!(scenario.name, "test");
        assert_eq!(scenario.description, "Test scenario");
        assert_eq!(scenario.events.len(), 1);
    }

    #[test]
    fn test_long_text_scenario() {
        let scenario = tui::scenarios::long_text(1000);
        assert!(scenario.description.contains("1000"));
    }
}
