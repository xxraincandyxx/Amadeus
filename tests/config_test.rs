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
