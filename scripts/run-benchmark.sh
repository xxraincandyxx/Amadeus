#!/usr/bin/env bash
# @amadeus-header
# summary: Project automation script for run-benchmark.
# layer: script
# status: active
# feature_flags: none
# provides:
# - cmd: scripts/run-benchmark.sh
# uses:
# - cmd: cargo
# invariants:
# - Script command flow remains non-interactive and order-dependent.
# side_effects:
# - Runs external commands or subprocesses.
# tests:
# - cmd: bash ./scripts/run-benchmark.sh
# @end-amadeus-header

set -euo pipefail

cargo run --bin benchmark -- "$@"
