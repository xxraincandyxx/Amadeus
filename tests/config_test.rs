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
use std::sync::{Mutex, OnceLock};
use tempfile::tempdir;

fn env_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .expect("env test lock poisoned")
}

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
    let _guard = env_lock();
    let temp = tempdir().unwrap();
    let settings_dir = temp.path().join(".amadeus");
    std::fs::create_dir_all(&settings_dir).unwrap();
    std::fs::write(
        settings_dir.join("settings.json"),
        r#"{"provider":"anthropic","api_key":"test-key"}"#,
    )
    .unwrap();

    let config = Config::load_with_hierarchy(temp.path()).unwrap();
    assert_eq!(config.timeout_seconds, 300);
}

#[test]
fn test_config_workdir_type() {
    let _guard = env_lock();
    let temp = tempdir().unwrap();
    let settings_dir = temp.path().join(".amadeus");
    std::fs::create_dir_all(&settings_dir).unwrap();
    std::fs::write(
        settings_dir.join("settings.json"),
        r#"{"provider":"anthropic","api_key":"test-key"}"#,
    )
    .unwrap();

    let config = Config::load_with_hierarchy(temp.path()).unwrap();
    assert!(!config.workdir.as_os_str().is_empty());
}

#[test]
fn parity_doc_exists_and_mentions_reference_baseline() {
    let doc_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/ROADMAP_PARITY.md");
    let doc = std::fs::read_to_string(&doc_path).expect("docs/ROADMAP_PARITY.md must exist");
    assert!(doc.contains("claw-code-parity"));
    assert!(doc.contains("current architectural gaps"));
}

#[test]
fn load_with_hierarchy_reads_workspace_settings_json() {
    let _guard = env_lock();
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
    let _guard = env_lock();
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

    let current_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp.path()).unwrap();
    let config = Config::load_for_assessment().unwrap();
    std::env::set_current_dir(current_dir).unwrap();

    restore_env("PROVIDER", provider);
    restore_env("ANTHROPIC_API_KEY", anthropic);
    restore_env("OPENAI_API_KEY", openai);
    restore_env("MODEL_ID", model);

    assert_eq!(config.provider, Provider::Anthropic);
    assert!(config.api_key.is_empty());
    assert_eq!(config.model, "claude-sonnet-4-5-20250929");
}

#[test]
fn load_with_hierarchy_ignores_process_env_overrides() {
    let _guard = env_lock();
    let temp = tempdir().unwrap();
    let settings_dir = temp.path().join(".amadeus");
    std::fs::create_dir_all(&settings_dir).unwrap();
    std::fs::write(
        settings_dir.join("settings.json"),
        r#"{"provider":"openai","api_key":"json-key","model":"json-model","timeout_seconds":42}"#,
    )
    .unwrap();

    let provider = std::env::var("PROVIDER").ok();
    let anthropic = std::env::var("ANTHROPIC_API_KEY").ok();
    let openai = std::env::var("OPENAI_API_KEY").ok();
    let model = std::env::var("MODEL_ID").ok();
    let timeout = std::env::var("TIMEOUT_SECONDS").ok();

    std::env::set_var("PROVIDER", "anthropic");
    std::env::set_var("ANTHROPIC_API_KEY", "env-key");
    std::env::set_var("OPENAI_API_KEY", "env-openai-key");
    std::env::set_var("MODEL_ID", "env-model");
    std::env::set_var("TIMEOUT_SECONDS", "99");

    let config = Config::load_with_hierarchy(temp.path()).unwrap();

    restore_env("PROVIDER", provider);
    restore_env("ANTHROPIC_API_KEY", anthropic);
    restore_env("OPENAI_API_KEY", openai);
    restore_env("MODEL_ID", model);
    restore_env("TIMEOUT_SECONDS", timeout);

    assert_eq!(config.provider, Provider::OpenAI);
    assert_eq!(config.api_key, "json-key");
    assert_eq!(config.model, "json-model");
    assert_eq!(config.timeout_seconds, 42);
}
