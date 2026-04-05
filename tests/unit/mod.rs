// @amadeus-header
// summary: Test module root for unit coverage and shared exports.
// layer: test
// status: test-only
// feature_flags:
// - full
// provides:
// - module: tests::unit
// uses: none
// invariants:
// - Module exports stay aligned with child modules and re-exports.
// side_effects: none
// tests:
// - cmd: cargo test --features full
// @end-amadeus-header

pub mod bash_test;
