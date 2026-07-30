// @amadeus-header
// summary: Re-exports shared composable prompt-building APIs.
// layer: infra
// status: active
// feature_flags: none
// provides:
// - module: crate::prompts
// - fn: crate::prompts::build_system_prompt
// uses:
// - module: amadeus_prompts
// invariants:
// - Core prompt call sites use the shared composable prompt implementation.
// side_effects: none
// tests:
// - cmd: cargo test -p prompts
// @end-amadeus-header

//! Shared composable system prompt APIs.

pub use amadeus_prompts::sections;
pub use amadeus_prompts::sections::default_sections;
pub use amadeus_prompts::{build_system_prompt, PromptSection, SystemPromptBuilder};
