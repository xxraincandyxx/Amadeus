#!/bin/bash
# @amadeus-header
# summary: Builds and stages the Amadeus API executable for Tauri sidecar packaging.
# layer: script
# status: active
# feature_flags:
# - full
# provides:
# - cmd: npm run desktop:sidecar
# uses:
# - cmd: cargo build --release --features full
# - cmd: rustc -vV
# invariants:
# - The staged filename includes the active Rust host target triple.
# side_effects:
# - Builds the root Amadeus executable.
# - Writes an ignored sidecar binary under apps/web/src-tauri/binaries.
# tests:
# - cmd: npm run desktop:build
# @end-amadeus-header

set -euo pipefail

script_dir=$(cd "$(dirname "$0")" && pwd)
repository_root=$(cd "$script_dir/../../.." && pwd)
target_triple=$(rustc -vV | sed -n 's/^host: //p')
sidecar_dir="$repository_root/apps/web/src-tauri/binaries"

cargo build --manifest-path "$repository_root/Cargo.toml" --release --features full
mkdir -p "$sidecar_dir"
cp "$repository_root/target/release/amadeus" "$sidecar_dir/amadeus-server-$target_triple"
