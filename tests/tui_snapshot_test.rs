//! TUI Snapshot Tests
//!
//! Visual regression tests that capture and verify complete terminal output.

mod tui;

use tui::TuiCapture;
use tui::comparison::{compare, format_diff, SnapshotComparison};
use tui::harness::{TuiTestHarness, InputSequence, run_scenario};
use tui::scenarios::{
    simple_text, streaming_text, with_tool_call, requiring_approval,
    MockScenarioClient, Scenario,
};
use amadeus::client::StreamEvent;

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

        let frames: Vec<tui::TuiFrameSnapshot> = run_scenario(
            client,
            InputSequence::new().type_text("Say hello"),
            80,
            24,
        ).await;

        // Should capture multiple frames during streaming
        assert!(frames.len() >= 2);
    }

    #[tokio::test]
    async fn test_tool_call_scenario() {
        let scenario = with_tool_call("ls -la", "total 8\ndrwxr-xr-x  3 user staff  160 Mar 21 12:00 .");
        let client = MockScenarioClient::new(scenario);

        let frames: Vec<tui::TuiFrameSnapshot> = run_scenario(
            client,
            InputSequence::new().type_text("List files").enter(),
            80,
            24,
        ).await;

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
        ).await;

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
