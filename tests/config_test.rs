// @amadeus-header
// summary: Integration tests covering config test behavior.
// layer: test
// status: test-only
// feature_flags:
// - full
// provides:
// - module: tests::config_test
// uses:
// - module: amadeus::agent::config
// - artifact: filesystem paths and files
// invariants:
// - Assertions stay aligned with current user-visible behavior.
// side_effects:
// - Reads or writes filesystem state.
// tests:
// - cmd: cargo test config_test --features full
// @end-amadeus-header

use amadeus::agent::config::{Config, Provider};

#[test]
fn test_provider_equality() {
    assert_eq!(Provider::Anthropic, Provider::Anthropic);
    assert_eq!(Provider::OpenAI, Provider::OpenAI);
    assert_ne!(Provider::Anthropic, Provider::OpenAI);
}

#[test]
fn test_provider_debug_formatting() {
    let provider = Provider::Anthropic;
    assert_eq!(format!("{:?}", provider), "Anthropic");

    let provider = Provider::OpenAI;
    assert_eq!(format!("{:?}", provider), "OpenAI");
}

#[test]
fn test_config_timeout_value() {
    std::env::set_var("ANTHROPIC_API_KEY", "test-key");
    std::env::set_var("PROVIDER", "anthropic");

    let config = Config::load().unwrap();
    assert_eq!(config.timeout_seconds, 300);
}

#[test]
fn test_config_workdir_type() {
    std::env::set_var("ANTHROPIC_API_KEY", "test-key");
    std::env::set_var("PROVIDER", "anthropic");

    let config = Config::load().unwrap();
    assert!(!config.workdir.as_os_str().is_empty());
}

#[test]
fn parity_doc_exists_and_mentions_reference_baseline() {
    let doc_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/EVALUATION.md");
    let doc = std::fs::read_to_string(&doc_path).expect("docs/EVALUATION.md must exist");
    assert!(doc.contains("refs/claw-code-parity/rust/PARITY.md"));
    assert!(doc.contains("Amadeus vs claw-code-parity Evaluation"));
}
