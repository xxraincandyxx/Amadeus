// @amadeus-header
// summary: CLI to convert a recorded session_*.json into a scenario JSON.
// layer: example
// status: active
// feature_flags:
// - test-utils
// provides:
// - module: example::convert_session
// uses:
// - fn: amadeus::test_utils::replay::session_log_to_scenario
// invariants: none
// side_effects:
// - Reads an input file; writes stdout.
// tests:
// - cmd: cargo run --example convert_session --features test-utils -- path/to/session.json
// @end-amadeus-header

//! Usage: convert_session <session_log.json>
//!
//! Loads a recorded `SessionLog` (a `session_*.json` artifact) and prints the
//! reconstructed `ScenarioDefinition` as pretty JSON on stdout. The output can
//! be fed straight into `ScenarioMockClient::from_json` for replay.

use std::path::PathBuf;

use amadeus::test_utils::replay::session_log_to_scenario;
use amadeus::test_utils::testflow::recorder::load_session;

fn main() -> Result<(), Box<dyn std::error::Error>> {
  let path = std::env::args_os()
    .nth(1)
    .map(PathBuf::from)
    .expect("usage: convert_session <session_log.json>");
  let log = load_session(&path)?;
  let scenario = session_log_to_scenario(&log);
  println!("{}", serde_json::to_string_pretty(&scenario)?);
  Ok(())
}
