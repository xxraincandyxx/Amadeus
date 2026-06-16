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

  // The agent turn may resolve asynchronously; poll-render until the assistant
  // text shows up or we exhaust a bounded number of attempts. `submit()` awaits
  // `settle()` (which yields while the session is still streaming), but the
  // assistant message may be committed to the message component slightly after
  // the stream closes, so re-rendering until the text appears is the robust
  // check.
  let mut seen = false;
  let mut last_text = String::new();
  for _ in 0..200 {
    let (_snap, text) = app.capture();
    if text.contains("Hello from the mock") {
      seen = true;
      break;
    }
    last_text = text;
    tokio::task::yield_now().await;
  }
  if !seen {
    eprintln!("captured text on final iteration:\n{last_text}");
  }
  assert!(seen, "assistant text should render after the mock turn completes");
}
