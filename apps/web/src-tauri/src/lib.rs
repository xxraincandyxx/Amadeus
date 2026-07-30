// @amadeus-header
// summary: Starts the native Tauri shell that hosts the Amadeus React workspace.
// layer: ui
// status: active
// feature_flags: none
// provides:
// - fn: run
// - runtime: Tauri desktop application
// uses:
// - fn: tauri::Builder::run
// - artifact: tauri.conf.json
// invariants:
// - The desktop shell delegates agent operations to the configured HTTP API.
// side_effects:
// - Creates and runs a native application window.
// tests:
// - cmd: npm run desktop:build
// @end-amadeus-header

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("failed to run Amadeus desktop application");
}
