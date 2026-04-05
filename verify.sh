#!/bin/bash
# @amadeus-header
# summary: Repository verification runner for formatting, linting, feature checks, and tests.
# layer: script
# status: active
# feature_flags:
# - full
# provides:
# - cmd: verify.sh
# uses:
# - cmd: cargo
# invariants:
# - Script command flow remains non-interactive and order-dependent.
# side_effects:
# - Runs external commands or subprocesses.
# - Writes output to stdout or stderr.
# tests:
# - cmd: bash ./verify.sh
# @end-amadeus-header

set -e

# Amadeus SDK Sanity Verification Script
# This script catches build-system, formatting, and feature-flag errors.

echo "🎨 Checking formatting..."
cargo fmt --all -- --check

echo "⚙️  Verifying Cargo.toml and metadata..."
cargo metadata --format-version 1 > /dev/null

echo "🔍 Running Clippy (All Features)..."
cargo clippy --all-features -- -D warnings

echo "🏗️  Verifying Feature Matrix..."

echo "   [1/4] Core only..."
cargo check --no-default-features

echo "   [2/4] TUI only..."
cargo check --features tui

echo "   [3/4] API only..."
cargo check --features api

echo "   [4/4] Full SDK..."
cargo check --features full

echo "🧪 Running Test Suite..."
cargo test --features full

echo -e "
✅ PROJECT IS HEALTHY"
