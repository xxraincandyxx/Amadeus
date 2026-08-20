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

<!-- gitnexus:start -->
# GitNexus — Code Intelligence

This project is indexed by GitNexus as **Amadeus** (7784 symbols, 18875 relationships, 300 execution flows). Use the GitNexus MCP tools to understand code, assess impact, and navigate safely.

> Index stale? Run `node .gitnexus/run.cjs analyze` from the project root — it auto-selects an available runner. No `.gitnexus/run.cjs` yet? `npx gitnexus analyze` (npm 11 crash → `npm i -g gitnexus`; #1939).

## Always Do

- **MUST run impact analysis before editing any symbol.** Before modifying a function, class, or method, run `impact({target: "symbolName", direction: "upstream"})` and report the blast radius (direct callers, affected processes, risk level) to the user.
- **MUST run `detect_changes()` before committing** to verify your changes only affect expected symbols and execution flows. For regression review, compare against the default branch: `detect_changes({scope: "compare", base_ref: "master"})`.
- **MUST warn the user** if impact analysis returns HIGH or CRITICAL risk before proceeding with edits.
- When exploring unfamiliar code, use `query({search_query: "concept"})` to find execution flows instead of grepping. It returns process-grouped results ranked by relevance.
- When you need full context on a specific symbol — callers, callees, which execution flows it participates in — use `context({name: "symbolName"})`.
- For security review, `explain({target: "fileOrSymbol"})` lists taint findings (source→sink flows; needs `analyze --pdg`).

## Never Do

- NEVER edit a function, class, or method without first running `impact` on it.
- NEVER ignore HIGH or CRITICAL risk warnings from impact analysis.
- NEVER rename symbols with find-and-replace — use `rename` which understands the call graph.
- NEVER commit changes without running `detect_changes()` to check affected scope.

## Resources

| Resource | Use for |
|----------|---------|
| `gitnexus://repo/Amadeus/context` | Codebase overview, check index freshness |
| `gitnexus://repo/Amadeus/clusters` | All functional areas |
| `gitnexus://repo/Amadeus/processes` | All execution flows |
| `gitnexus://repo/Amadeus/process/{name}` | Step-by-step execution trace |

## CLI

| Task | Read this skill file |
|------|---------------------|
| Understand architecture / "How does X work?" | `.claude/skills/gitnexus/gitnexus-exploring/SKILL.md` |
| Blast radius / "What breaks if I change X?" | `.claude/skills/gitnexus/gitnexus-impact-analysis/SKILL.md` |
| Trace bugs / "Why is X failing?" | `.claude/skills/gitnexus/gitnexus-debugging/SKILL.md` |
| Rename / extract / split / refactor | `.claude/skills/gitnexus/gitnexus-refactoring/SKILL.md` |
| Tools, resources, schema reference | `.claude/skills/gitnexus/gitnexus-guide/SKILL.md` |
| Index, status, clean, wiki CLI commands | `.claude/skills/gitnexus/gitnexus-cli/SKILL.md` |

<!-- gitnexus:end -->
