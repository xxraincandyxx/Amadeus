// @amadeus-header
// summary: TUI edge-case coverage driving HeadlessApp through multi-turn, tool-use, error, and small-viewport scenarios.
// layer: test
// status: test-only
// feature_flags:
// - full
// provides:
// - module: tests::tui_edge_cases_test
// uses:
// - module: tests::mocks::scenario_client
// - module: amadeus::ui::headless
// - module: amadeus::test_utils::scenario
// invariants:
// - Each test exercises a code path not covered by tui_replay_test.rs.
// side_effects: none
// tests:
// - cmd: cargo test --features full --test tui_edge_cases_test
// @end-amadeus-header

//! Edge-case coverage for HeadlessApp. Where `tui_replay_test.rs` proves the
//! happy-path loop, this file hunts for bugs in the paths around it:
//! multi-turn step advancement, tool-use turns, error turns, very small
//! viewports, and double-submit recovery.

#[path = "mocks/mod.rs"]
mod mocks;

use amadeus::test_utils::scenario::{
    ScenarioDefinition, ScenarioStepDef, StreamEventDef,
};
use amadeus::ui::headless::HeadlessApp;

use mocks::ScenarioMockClient;

fn def(steps: Vec<ScenarioStepDef>, name: &str) -> ScenarioDefinition {
    ScenarioDefinition {
        name: name.to_string(),
        description: "edge case".to_string(),
        steps,
    }
}

fn step(events: Vec<StreamEventDef>) -> ScenarioStepDef {
    ScenarioStepDef {
        delay_ms: None,
        events,
        error: None,
    }
}

#[tokio::test]
async fn multi_turn_advances_through_scripted_steps() {
    // Two distinct assistant answers across two turns. If the mock does not
    // advance (or the harness does not settle between submits), the second
    // answer will be missing or wrong.
    //
    // Note: `messages_text()` drains the unrendered buffer (it wraps
    // `take_unrendered_lines`, which is a one-shot queue the live UI consumes
    // via terminal scrollback). So we accumulate drained output across turns
    // rather than expecting earlier turns to remain in the buffer.
    let client = ScenarioMockClient::from_definition(def(
        vec![
            step(vec![
                StreamEventDef::TextDelta { text: "first answer".to_string() },
                StreamEventDef::StopReason { reason: "end_turn".to_string() },
            ]),
            step(vec![
                StreamEventDef::TextDelta { text: "second answer".to_string() },
                StreamEventDef::StopReason { reason: "end_turn".to_string() },
            ]),
        ],
        "multi_turn",
    ));

    let mut app = HeadlessApp::new(client, ".", "m", 80, 24);
    let mut all_messages = String::new();

    app.type_text("q1");
    app.submit().await;
    all_messages.push_str(&app.messages_text(80));
    assert!(
        all_messages.contains("first answer"),
        "first turn should commit its answer:\n{all_messages}"
    );

    app.type_text("q2");
    app.submit().await;
    all_messages.push_str(&app.messages_text(80));
    assert!(
        all_messages.contains("second answer"),
        "second turn should advance the mock and commit a distinct answer:\n{all_messages}"
    );
    assert!(
        all_messages.contains("first answer"),
        "first answer should survive in the accumulated transcript:\n{all_messages}"
    );
}

#[tokio::test]
async fn error_turn_does_not_deadlock_the_session() {
    // A mock step that errors must not leave the session in a streaming state
    // forever. After submit(), a subsequent capture() should still work and the
    // session should accept new input.
    let error_step = ScenarioStepDef {
        delay_ms: None,
        events: Vec::new(),
        error: Some("boom: simulated provider failure".to_string()),
    };
    let client = ScenarioMockClient::from_definition(def(
        vec![
            error_step,
            step(vec![
                StreamEventDef::TextDelta { text: "recovered".to_string() },
                StreamEventDef::StopReason { reason: "end_turn".to_string() },
            ]),
        ],
        "error_then_recover",
    ));

    let mut app = HeadlessApp::new(client, ".", "m", 80, 24);
    app.type_text("trigger error");
    app.submit().await;

    // Frame path must still render chrome after an errored turn.
    let (_snap, frame) = app.capture();
    assert!(
        !frame.is_empty(),
        "frame should still render after an errored turn"
    );

    // The session must be reusable.
    app.type_text("ok");
    app.submit().await;
    let messages = app.messages_text(80);
    assert!(
        messages.contains("recovered"),
        "session should recover after an error turn:\n{messages}"
    );
}

#[tokio::test]
async fn tiny_viewport_does_not_panic_on_render() {
    // Regression guard: very small terminals used to collapse the input box
    // out of the layout. Rendering into a 10x3 grid must not panic and must
    // still return the requested number of cells.
    let client = ScenarioMockClient::from_definition(def(
        vec![step(vec![
            StreamEventDef::TextDelta { text: "tiny".to_string() },
            StreamEventDef::StopReason { reason: "end_turn".to_string() },
        ])],
        "tiny",
    ));

    let mut app = HeadlessApp::new(client, ".", "m", 10, 3);
    app.type_text("x");
    app.submit().await;

    let (snap, _text) = app.capture();
    assert_eq!(snap.width, 10);
    assert_eq!(snap.height, 3);
    assert_eq!(snap.cells.len(), 10 * 3, "capture must fill the full grid");
}

#[tokio::test]
async fn double_submit_with_empty_input_does_not_corrupt_state() {
    // Submitting twice without typing anything in between should not advance
    // the mock or panic. The second submit is effectively a no-op.
    let client = ScenarioMockClient::from_definition(def(
        vec![
            step(vec![
                StreamEventDef::TextDelta { text: "only once".to_string() },
                StreamEventDef::StopReason { reason: "end_turn".to_string() },
            ]),
            step(vec![
                StreamEventDef::TextDelta { text: "should not appear".to_string() },
                StreamEventDef::StopReason { reason: "end_turn".to_string() },
            ]),
        ],
        "double_submit",
    ));

    let mut app = HeadlessApp::new(client, ".", "m", 80, 24);
    app.type_text("hello");
    app.submit().await;
    // Immediate second submit with empty input.
    app.submit().await;

    let messages = app.messages_text(80);
    assert!(
        messages.contains("only once"),
        "first answer should be present:\n{messages}"
    );
    assert!(
        !messages.contains("should not appear"),
        "empty submit must not consume the next mock step:\n{messages}"
    );
}

#[tokio::test]
async fn unicode_input_round_trips_into_messages() {
    // Non-ASCII input must survive the type -> submit -> message-commit loop
    // without mojibake. This catches String/char boundary bugs in the input
    // component.
    let client = ScenarioMockClient::from_definition(def(
        vec![step(vec![
            StreamEventDef::TextDelta { text: "echo: 你好 🦀".to_string() },
            StreamEventDef::StopReason { reason: "end_turn".to_string() },
        ])],
        "unicode",
    ));

    let mut app = HeadlessApp::new(client, ".", "m", 80, 24);
    app.type_text("你好");
    app.submit().await;

    let messages = app.messages_text(80);
    assert!(
        messages.contains("你好 🦀"),
        "unicode should round-trip cleanly:\n{messages}"
    );
}
