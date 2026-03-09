# Repository Guidelines

## Project Structure & Module Organization
`src/` contains the library and binary code. Core areas include `src/agent/` for orchestration, `src/client/` for provider integrations, `src/tools/` for built-in tools, `src/ui/` for the ratatui interface, and `src/api/` for Axum handlers. Integration and scenario tests live in `tests/`, with shared mocks under `tests/mocks/` and reusable scenario helpers under `tests/scenarios/`. Examples are in `examples/`, longer design notes in `docs/`, and helper scripts such as [`verify.sh`](/Users/raincandy_u/Dev/amadeus/verify.sh) live at the repo root.

## Build, Test, and Development Commands
Use Cargo directly; feature flags matter in this repository.

- `cargo check --no-default-features` validates the minimal crate.
- `cargo check --features tui` or `cargo check --features api` verifies specific optional surfaces.
- `cargo build --features full` builds the full SDK and CLI.
- `cargo run --features full` launches the main binary.
- `cargo run --example tui --features tui` runs the TUI example.
- `cargo test --features full` runs the main test suite.
- `./verify.sh` runs formatting, metadata, Clippy, feature-matrix checks, and tests.

## Coding Style & Naming Conventions
Follow Rust 2021 idioms and keep modules focused. Use `snake_case` for files, modules, and functions, `PascalCase` for types, and descriptive feature names like `supervisor` or `test-utils`. `cargo fmt --all` is the formatting authority, and `cargo clippy --all-features -- -D warnings` should pass before opening a PR. Prefer `Result`-based error handling and avoid `unwrap()` in production paths.

## Testing Guidelines
Add unit tests close to the implementation when practical, and place cross-module or behavioral coverage in `tests/`. Name test files by behavior or subsystem, for example `tool_approval_test.rs` or `stress_memory_test.rs`. If a test depends on gated functionality, use the matching feature flag, such as `cargo test --test e2e_product_flow --features full -- --nocapture`.

## Commit & Pull Request Guidelines
Recent history uses Conventional Commit style with scopes, for example `feat(ui): ...`, `fix(agent): ...`, and `style(agent): ...`. Keep commits small and reviewable. PRs should explain the user-visible change, note any required feature flags or env vars, link related issues, and include screenshots or terminal captures when UI behavior changes.

## Configuration & Safety
Copy `.env.example` to `.env` for local setup. Never commit real API keys or edited secrets files. Treat policy, tool-execution, and approval-flow changes as high risk and cover them with targeted tests.
