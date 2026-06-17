// @amadeus-header
// summary: Compatibility wrapper re-exporting structured config types from the config crate.
// layer: agent
// status: active
// feature_flags: none
// provides:
// - module: crate::agent::config
// - type: crate::agent::config::Provider
// - type: crate::agent::config::Config
// - type: crate::agent::config::ConfigError
// - type: crate::agent::config::PromptSettings
// - type: crate::agent::config::ToolOverrideConfig
// - type: crate::agent::config::ToolProfileConfig
// - type: crate::agent::config::ToolSettings
// - type: crate::agent::config::TuiSettings
// - type: crate::agent::config::LiveViewportConfig
// - type: crate::agent::config::LiveViewportMode
// uses:
// - module: amadeus_config
// invariants:
// - Public config paths remain stable while implementation lives outside core.
// side_effects: none
// tests:
// - tests/config_test.rs
// @end-amadeus-header

//! Compatibility re-exports for configuration types.

pub use amadeus_config::{
    Config, ConfigError, HookSandboxMode, LiveViewportConfig, LiveViewportMode, PromptMergeMode,
    PromptProfileConfig, PromptSectionConfig, PromptSettings, Provider, ToolOverrideConfig,
    ToolProfileConfig, ToolSettings, TuiSettings,
};
