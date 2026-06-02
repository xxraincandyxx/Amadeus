#!/usr/bin/env bash
# Amadeus system launcher
# Install: ln -sf "$(pwd)/scripts/amadeus-launch.sh" /usr/local/bin/amadeus
# Usage:  amadeus              (TUI mode)
#         amadeus --server     (HTTP API mode)

set -euo pipefail

resolve_script_dir() {
	local src="$0"
	while [ -L "$src" ]; do
		local dir
		dir="$(cd -P "$(dirname "$src")" && pwd)"
		src="$(readlink "$src")"
		[[ "$src" != /* ]] && src="$dir/$src"
	done
	cd -P "$(dirname "$src")" && pwd
}

AMADEUS_DIR="${AMADEUS_DIR:-$(cd "$(resolve_script_dir)/.." && pwd)}"
BINARY="${AMADEUS_DIR}/target/release/amadeus"

if [[ ! -f "$BINARY" ]]; then
	BINARY="${AMADEUS_DIR}/target/debug/amadeus"
fi

if [[ ! -f "$BINARY" ]]; then
	echo "error: amadeus binary not found. Build with: cargo build --features full --release" >&2
	exit 1
fi

exec "$BINARY" "$@"
