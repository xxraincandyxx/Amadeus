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
use tempfile::tempdir;

fn restore_env(key: &str, original: Option<String>) {
    if let Some(value) = original {
        std::env::set_var(key, value);
    } else {
        std::env::remove_var(key);
    }
}

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

#[test]
fn load_with_hierarchy_reads_workspace_settings_json() {
    let temp = tempdir().unwrap();
    let settings_dir = temp.path().join(".amadeus");
    std::fs::create_dir_all(&settings_dir).unwrap();
    std::fs::write(
        settings_dir.join("settings.json"),
        r#"{"provider":"openai","api_key":"json-key","model":"gpt-test"}"#,
    )
    .unwrap();

    let provider = std::env::var("PROVIDER").ok();
    let anthropic = std::env::var("ANTHROPIC_API_KEY").ok();
    let openai = std::env::var("OPENAI_API_KEY").ok();
    std::env::remove_var("PROVIDER");
    std::env::remove_var("ANTHROPIC_API_KEY");
    std::env::remove_var("OPENAI_API_KEY");

    let config = Config::load_with_hierarchy(temp.path()).unwrap();

    restore_env("PROVIDER", provider);
    restore_env("ANTHROPIC_API_KEY", anthropic);
    restore_env("OPENAI_API_KEY", openai);

    assert_eq!(config.provider, Provider::OpenAI);
    assert_eq!(config.api_key, "json-key");
    assert_eq!(config.model, "gpt-test");
}

#[test]
fn load_with_hierarchy_ignores_legacy_dotenv_files() {
    let temp = tempdir().unwrap();
    std::fs::write(
        temp.path().join(".env"),
        "PROVIDER=openai\nOPENAI_API_KEY=legacy-key\nMODEL_ID=legacy-model\n",
    )
    .unwrap();

    let provider = std::env::var("PROVIDER").ok();
    let anthropic = std::env::var("ANTHROPIC_API_KEY").ok();
    let openai = std::env::var("OPENAI_API_KEY").ok();
    let model = std::env::var("MODEL_ID").ok();
    std::env::remove_var("PROVIDER");
    std::env::remove_var("ANTHROPIC_API_KEY");
    std::env::remove_var("OPENAI_API_KEY");
    std::env::remove_var("MODEL_ID");

    let config = Config::load_with_hierarchy(temp.path()).unwrap();

    restore_env("PROVIDER", provider);
    restore_env("ANTHROPIC_API_KEY", anthropic);
    restore_env("OPENAI_API_KEY", openai);
    restore_env("MODEL_ID", model);

    assert_eq!(config.provider, Provider::Anthropic);
    assert!(config.api_key.is_empty());
    assert_eq!(config.model, "claude-sonnet-4-5-20250929");
}
