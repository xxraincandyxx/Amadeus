// @amadeus-header
// summary: Stable re-export surface for HTTP request and response data transfer objects.
// layer: api
// status: active
// feature_flags:
// - api
// provides:
// - module: crate::api::types
// uses:
// - module: crate::api::types::capabilities
// - module: crate::api::types::core
// - module: crate::api::types::rag
// - module: crate::api::types::sessions
// - module: crate::api::types::settings
// invariants:
// - Existing crate::api::types paths remain stable through re-exports.
// side_effects: none
// tests:
// - cmd: cargo test -p api --all-features
// @end-amadeus-header

//! JSON data transfer objects for the HTTP API.

mod capabilities;
mod core;
mod rag;
mod sessions;
mod settings;

pub use capabilities::*;
pub use core::*;
pub use rag::*;
pub use sessions::*;
pub use settings::*;
