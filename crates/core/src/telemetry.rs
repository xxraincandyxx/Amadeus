// @amadeus-header
// summary: Compatibility wrapper re-exporting telemetry types from the telemetry crate.
// layer: core
// status: active
// feature_flags: none
// provides:
// - module: crate::telemetry
// - type: crate::telemetry::TelemetryEntry
// - type: crate::telemetry::TelemetryEvent
// - type: crate::telemetry::TelemetryRecorder
// - type: crate::telemetry::TelemetrySink
// - type: crate::telemetry::MemorySink
// - type: crate::telemetry::JsonlSink
// - type: crate::telemetry::TelemetryError
// uses:
// - module: amadeus_telemetry
// invariants:
// - Public telemetry paths remain stable while implementation lives in the telemetry crate.
// side_effects: none
// tests:
// - cmd: cargo test -p telemetry
// @end-amadeus-header

pub use amadeus_telemetry::{
    JsonlSink, MemorySink, TelemetryEntry, TelemetryError, TelemetryEvent, TelemetryRecorder,
    TelemetrySink,
};
