// @amadeus-header
// summary: End-to-end replay: scenario -> HeadlessApp -> rendered text assertion.
// layer: test
// status: test-only
// feature_flags:
// - full
// provides:
// - module: tests::tui_replay_test
// uses:
// - module: tests::mocks::scenario_client
// - module: amadeus::ui::headless
// invariants: none
// side_effects: none
// tests:
// - cmd: cargo test --features full --test tui_replay_test
// @end-amadeus-header

//! End-to-end replay coverage: build a `ScenarioDefinition`, hand it to a
//! `ScenarioMockClient`, drive the real `HeadlessApp`, and assert the assistant
//! text renders into the captured frame. This is the loop that ties together
//! Tasks 1-5 (scenario types, frame text, app/session wrappers, HeadlessApp).

// Pull in the scenario_client mock exactly as `tests/todo_test.rs` and other
// integration tests do: route `tests/mocks/mod.rs` in as a local module, then
// use the re-exported `ScenarioMockClient`.
#[path = "mocks/mod.rs"]
mod mocks;

use amadeus::test_utils::scenario::{ScenarioDefinition, ScenarioStepDef, StreamEventDef};
use amadeus::ui::headless::HeadlessApp;

use mocks::ScenarioMockClient;

fn text_turn() -> ScenarioDefinition {
  ScenarioDefinition {
    name: "text".to_string(),
    description: "one text turn".to_string(),
    steps: vec![ScenarioStepDef {
      delay_ms: None,
      events: vec![
        StreamEventDef::TextDelta { text: "Hello from the mock".to_string() },
        StreamEventDef::StopReason { reason: "end_turn".to_string() },
      ],
      error: None,
    }],
  }
}

#[tokio::test]
async fn scenario_drives_app_and_renders_assistant_text() {
  let client = ScenarioMockClient::from_definition(text_turn());
  let mut app = HeadlessApp::new(client, ".", "test-model", 80, 24);
  app.type_text("hi");
  app.submit().await;

  // Amadeus renders committed conversation via terminal scrollback
  // (insert_before), not the live frame buffer, so verify the turn's assistant
  // message was committed to the message store.
  let messages = app.messages_text(80);
  assert!(
    messages.contains("Hello from the mock"),
    "assistant message should be committed after the mock turn:\n{messages}"
  );

  // The frame buffer still renders chrome (footer carries the model name); this
  // confirms the frame path works even though transcript text uses scrollback.
  let (_snap, frame) = app.capture();
  assert!(
    frame.contains("test-model"),
    "frame should still render footer chrome after the turn:\n{frame}"
  );
}

#[tokio::test]
async fn loads_text_turn_fixture_from_json() {
  let json = std::fs::read_to_string("tests/tui/scenarios/text_turn.json")
    .expect("fixture should exist");
  let client = ScenarioMockClient::from_json(&json).expect("parse fixture");
  let mut app = HeadlessApp::new(client, ".", "test-model", 80, 24);
  app.type_text("ping");
  app.submit().await;

  let messages = app.messages_text(80);
  assert!(
    messages.contains("one-line answer"),
    "fixture-driven turn should commit the assistant text:\n{messages}"
  );
}
