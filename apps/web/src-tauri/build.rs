// @amadeus-header
// summary: Generates native Tauri build metadata for the Amadeus desktop client.
// layer: infra
// status: active
// feature_flags: none
// provides:
// - artifact: Tauri native build metadata
// uses:
// - fn: tauri_build::build
// invariants:
// - Build metadata stays synchronized with tauri.conf.json.
// side_effects:
// - Emits Cargo build instructions.
// tests:
// - cmd: npm run desktop:build
// @end-amadeus-header

fn main() {
    tauri_build::build()
}
