# Copilot / Agent Instructions for Amadeus

Purpose
-------
This file helps AI coding agents (Copilot Chat / code assistants) be productive in this repository by describing the minimal conventions, build/test/run commands, and where to find more detailed docs.

Quick Actions
-------------
- Build (dev): `cargo build --features full`
- Run (TUI): `cargo run --features full`
- Run HTTP server: `cargo run --features full -- --server [PORT]`
- Tests: `cargo test --features full` (use `-- --nocapture` to see outputs)
- Single crate: `cargo check -p tui`, `cargo test -p core`
- Format: `cargo fmt --all`; Lint: `cargo clippy --all-features -- -D warnings`
- Full verification before a PR: `./verify.sh`
- Web client: `cd apps/web && npm run dev` / `npm run test` / `npm run lint`

Where to look first
-------------------
- High-level project overview and commands: see `CLAUDE.md`
- Agent behavior rules: see `AGENTS.md`; long-form style guide: see `CODING_STYLE.md`
- Primary Rust source: `crates/` (agent loop: `crates/core/src/agent/loop_agent.rs`). The root `src/` is only the compatibility facade (`lib.rs`) and CLI entry point (`main.rs`).

Key Conventions (short)
-----------------------
- Feature flags: the crate has no default features — prefer `--features full` for development. See `CLAUDE.md` for the feature list.
- Code style: 4-space indentation (`rustfmt` default, no `rustfmt.toml`); `snake_case` for functions; `PascalCase` for types.
- Every in-scope source file starts with an `@amadeus-header` block (`docs/SOURCE_FILE_HEADERS.md`); keep it accurate and validate with `python scripts/check_source_headers.py`.
- No `unwrap()`/`expect()` outside `#[cfg(test)]`; return `crate::error::Result`.
- No inline comments unless explicitly requested.
- Tests: integration tests live in `tests/`; use `--features full` when running them.
- Tools and policies: tool implementations live in `crates/core/src/tools/`; policy in `crates/core/src/policy/`.
- User-facing UI strings go through the translation catalogs (`crates/tui/src/ui/i18n.rs`, `apps/web/src/i18n.js`), not literals.

Agent-behavior guidance for assistants
-------------------------------------
- Do not run destructive shell commands. The repo enforces blocked patterns (e.g., `rm -rf /`, `sudo`).
- When asked to modify code, create small, focused patches using the repository's style (4-space indentation, avoid unrelated reformatting).
- Prefer editing or adding tests when changing behavior—follow the project's testing strategy (mock-first).

If creating or updating agent instructions
----------------------------------------
- `AGENTS.md`, `CLAUDE.md`, and `GEMINI.md` describe the same codebase for different tools and are meant to stay in sync; keep this file short and link to them rather than duplicating.

Example prompts (for reviewers or maintainers)
---------------------------------------------
- "Run the unit tests and report failing tests with a short summary."
- "Add a focused unit test for `crates/core/src/agent/compaction.rs` that covers token threshold behavior."
- "Create a small integration test that runs `agent_integration_test.rs::test_agent_lifecycle` with mocks."

Suggested next customizations
-----------------------------
- Add an `applyTo` pattern set and smaller instruction files per area (e.g., `crates/core/src/agent/`, `crates/core/src/tools/`, `apps/web/`) if you want different agent behavior in different subtrees.
- Create `/.github/agent-prompts/` with 3–5 curated example prompts for common workflows (run tests, add feature, fix linter issues).

Where to find more
------------------
- Project overview and commands: `CLAUDE.md`
- Behavior rules and quick reference: `AGENTS.md`
- Formatting, headers, error handling: `CODING_STYLE.md`
- Architecture deep-dive: `docs/ARCHITECTURE.md`

If anything here is unclear, ask maintainers for the preferred scope (small patch, RFC, or test-first change).
