# Amadeus Agent Guide

Amadeus is a Rust workspace for multi-provider AI agents. The root crate is a compatibility facade; implementation lives in `crates/`. `apps/client`, `python-sdk`, `benchmarks`, and the top-level `runtime` directory are outside the Cargo workspace.

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
| Web checks | `cd apps/client && npm run lint && npm run test` |

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
- Configuration: `.amadeus/README.md`; web client: `apps/client/README.md`
- GitNexus workflows: `.claude/skills/gitnexus/`

<!-- gitnexus:start -->
# GitNexus — Code Intelligence

This project is indexed by GitNexus as **Amadeus** (9799 symbols, 27604 relationships, 646 execution flows).

> Index stale? Run `node .gitnexus/run.cjs analyze --index-only` from the project root — it auto-selects an available runner. No `.gitnexus/run.cjs` yet? Bootstrap with `npx`, `bunx`, or `pnpm dlx` — e.g. `bunx gitnexus@latest analyze` (npm 11 npx crash; #1939).

## Always Do

- **MUST run impact analysis before editing.** Use `impact({target: "symbolName", direction: "upstream"})` (MCP) or `node .gitnexus/run.cjs impact "symbolName" --direction upstream --repo .` (CLI fallback); report callers, processes, and risk. Never substitute grep for graph analysis.
- **MUST analyze graph changes before committing.** Use `detect_changes({scope: "all"})` (MCP) or `node .gitnexus/run.cjs detect-changes --scope all --repo .` (CLI fallback). `partial: true` or `truncated: true` is not a clean check — a zero means unseen, not unaffected; re-run it. For regression review: `detect_changes({scope: "compare", base_ref: "master"})` or `node .gitnexus/run.cjs detect-changes --scope compare --base-ref "master" --repo .`.
- **MUST warn the user** if impact analysis returns HIGH or CRITICAL risk before proceeding with edits.
- **MUST treat `risk: UNKNOWN` as unresolved, not as low.** An empty caller set is not evidence the symbol is unused — it can also mean the callers are not resolvable by the index (plain-object property access, dynamic dispatch, cross-language calls). `impact` pairs `UNKNOWN` with a `riskNote` saying so. Confirm with a text search before treating the symbol as safe to change or delete; do not proceed on the strength of a zero.
- When exploring unfamiliar code, use `query({search_query: "concept"})` to find execution flows instead of grepping. It returns process-grouped results ranked by relevance.
- When you need full context on a specific symbol — callers, callees, which execution flows it participates in — use `context({name: "symbolName"})`.
- For security review, `explain({target: "fileOrSymbol"})` lists taint findings (source→sink flows; needs `analyze --pdg`).

## Never Do

- NEVER edit a function, class, or method before MCP/CLI impact analysis.
- NEVER ignore HIGH or CRITICAL risk warnings from impact analysis, and never read `UNKNOWN` as an all-clear — it means the walk could not answer, which is the one verdict that requires confirming by other means.
- NEVER rename symbols with find-and-replace — use `rename` which understands the call graph.
- NEVER commit before MCP/CLI graph change analysis.

## Resources

| Resource | Use for |
| --- | --- |
| `gitnexus://repo/Amadeus/context` | Codebase overview, check index freshness |
| `gitnexus://repo/Amadeus/clusters` | All functional areas |
| `gitnexus://repo/Amadeus/processes` | All execution flows |
| `gitnexus://repo/Amadeus/process/{name}` | Step-by-step execution trace |

## CLI

| Task | Read this skill file |
| --- | --- |
| Understand architecture / "How does X work?" | `.claude/skills/gitnexus-exploring/SKILL.md` |
| Blast radius / "What breaks if I change X?" | `.claude/skills/gitnexus-impact-analysis/SKILL.md` |
| Trace bugs / "Why is X failing?" | `.claude/skills/gitnexus-debugging/SKILL.md` |
| Rename / extract / split / refactor | `.claude/skills/gitnexus-refactoring/SKILL.md` |
| Tools, resources, schema reference | `.claude/skills/gitnexus-guide/SKILL.md` |
| Index, status, clean, wiki CLI commands | `.claude/skills/gitnexus-cli/SKILL.md` |

<!-- gitnexus:end -->
