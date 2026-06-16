// @amadeus-header
// summary: Compatibility layer re-exporting shared prompt templates.
// layer: infra
// status: active
// feature_flags: none
// provides:
// - module: crate::prompts
// - const: crate::prompts::SYSTEM_PROMPT
// - fn: crate::prompts::render_system_prompt
// uses:
// - module: amadeus_prompts
// invariants:
// - Core prompt call sites stay compatible while prompt templates live in a dedicated crate.
// side_effects: none
// tests:
// - cmd: cargo test -p prompts
// @end-amadeus-header

//! Compatibility layer for shared system prompts.

pub use amadeus_prompts::sections;
pub use amadeus_prompts::sections::default_sections;
pub use amadeus_prompts::{
    build_system_prompt, render_system_prompt, PromptSection, SystemPromptBuilder, SYSTEM_PROMPT,
};
