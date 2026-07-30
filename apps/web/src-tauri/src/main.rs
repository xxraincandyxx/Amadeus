// @amadeus-header
// summary: Provides the executable entry point for the native Amadeus desktop client.
// layer: ui
// status: active
// feature_flags: none
// provides:
// - runtime: amadeus-desktop executable
// uses:
// - fn: amadeus_desktop_lib::run
// invariants:
// - Desktop startup delegates directly to the shared Tauri library entry point.
// side_effects:
// - Starts the Amadeus desktop process.
// tests:
// - cmd: npm run desktop:build
// @end-amadeus-header

fn main() {
    amadeus_desktop_lib::run();
}
