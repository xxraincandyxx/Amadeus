# Amadeus Agent Guide

Amadeus is a Rust workspace for multi-provider AI agents. The root crate is a compatibility facade; implementation lives in `crates/`. `apps/web`, `python-sdk`, `benchmarks`, and the top-level `runtime` directory are outside the Cargo workspace.

## Required Workflow

- Use `--features full` for repository development; there are no default features.
- Before editing a symbol, run GitNexus `impact` upstream and report its direct callers, affected processes, and risk. Warn before HIGH or CRITICAL changes.
- Use GitNexus `query` for unfamiliar execution flows and `context` for a symbol's callers and callees.
- Before every commit, run GitNexus `detect_changes` and review the affected scope.
- Preserve behavior unless the task explicitly changes it. Keep patches focused and retain unrelated worktree changes.
- After code changes, run `cargo check --features full`, relevant tests, and the source-header check. Run `./verify.sh` for the final gate.

## Commands

| Task | Command |
| --- | --- |
| Build / run | `cargo build --features full` / `cargo run --features full` |
| HTTP server | `cargo run --features full -- --server [PORT]` |
| Check / test | `cargo check --features full` / `cargo test --features full` |
| Format / lint | `cargo fmt --all` / `cargo clippy --all-features -- -D warnings` |
| Full verification | `./verify.sh` |
| Source headers | `python3 scripts/check_source_headers.py` |
| Web checks | `cd apps/web && npm run lint && npm run test` |

## Universal Rules

- Follow `CODING_STYLE.md`. Use rustfmt defaults and do not add `rustfmt.toml`.
- Use `crate::error::Result<T>` and never use `unwrap()` or `expect()` in production code.
- Public items require rustdoc. Do not add optional implementation comments unless requested.
- Keep required `@amadeus-header` blocks accurate when touching source files.
- Use Tokio throughout. Prefer generics on hot paths and `Arc` for shared ownership.
- Route changed UI text through the TUI or web localization catalog.
- Add slash commands to the spec table, enum, and `parse`/`name`; the web palette excludes TUI-only commands.
- Never expose secrets or modify `.env`, `.pem`, or `.key` files.

## Detailed References

- Architecture and ownership: `docs/ARCHITECTURE.md`
- Style, errors, features, and tests: `CODING_STYLE.md`
- Source-header schema: `docs/SOURCE_FILE_HEADERS.md`
- HTTP contract: `docs/HTTP_API.md`; TUI testing: `docs/TUI_TESTING.md`
- Configuration: `.amadeus/README.md`; web client: `apps/web/README.md`
- GitNexus workflows: `.claude/skills/gitnexus/`
