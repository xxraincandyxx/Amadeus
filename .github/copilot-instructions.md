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
- Format: `cargo fmt`; Lint: `cargo clippy --features full`

Where to look first
-------------------
- High-level project overview and commands: see CLAUDE.md
- Agent-focused guidance and educational agent examples: see refs/AGENTS.md
- Primary Rust source: `src/` (agent loop: `src/agent/loop_agent.rs`)

Key Conventions (short)
-----------------------
- Feature flags: prefer `--features full` for development. See CLAUDE.md for feature list.
- Code style: 2-space indentation; `snake_case` for functions; `PascalCase` for types.
- Tests: integration tests live in `tests/`; use feature flag `full` when running them.
- Tools and policies: tool implementations live in `src/tools/`; policy in `src/policy/`.

Agent-behavior guidance for assistants
-------------------------------------
- Do not run destructive shell commands. The repo enforces blocked patterns (e.g., `rm -rf /`, `sudo`).
- When asked to modify code, create small, focused patches using the repository's style (2-space indentation, avoid unrelated reformatting).
- Prefer editing or adding tests when changing behavior—follow the project's testing strategy (mock-first).

If creating or updating agent instructions
----------------------------------------
- If the repo already has an `AGENTS.md` (here: `refs/AGENTS.md`), preserve its educational contents and add only workspace-specific quickstart bits here.
- Keep this file short and link to deeper docs rather than duplicating them.

Example prompts (for reviewers or maintainers)
---------------------------------------------
- "Run the unit tests and report failing tests with a short summary."  
- "Add a focused unit test for `src/agent/compaction.rs` that covers token threshold behavior."  
- "Create a small integration test that runs `agent_integration_test.rs::test_agent_lifecycle` with mocks."  

Suggested next customizations
-----------------------------
- Add an `applyTo` pattern set and smaller instruction files per area (e.g., `src/agent/`, `src/tools/`) if you want different agent behavior in different subtrees.
- Create `/.github/agent-prompts/` with 3–5 curated example prompts for common workflows (run tests, add feature, fix linter issues).

Where to find more
------------------
- Project overview and commands: CLAUDE.md
- Educational agent examples and coding conventions: refs/AGENTS.md

If anything here is unclear, ask maintainers for the preferred scope (small patch, RFC, or test-first change).
