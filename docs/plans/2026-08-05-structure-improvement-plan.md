# Codebase Structure Assessment & Improvement Plan

**Date:** 2026-08-05
**Scope:** Workspace structure, layering, test distribution, CI, and documentation hygiene.
**Baseline:** `cargo check --features full` passes; `python scripts/check_source_headers.py` reports ok.

---

## 1. Measured Snapshot

**Rust LOC by crate (`src/` only):**

| Crate | LOC | Files | Inline tests |
|-------|-----|-------|--------------|
| `core` | 21,934 | 74 | 115 |
| `tui` | 18,907 | 48 | 196 |
| `api` | 4,837 | 24 | **4** |
| `config` | 2,114 | **1** | 18 |
| `runtime` | 1,459 | 6 | 20 |
| `rag` | 1,288 | 5 | 21 |
| `context` | 859 | 5 | 20 |
| remaining 11 crates | < 500 each | 1–3 | 1–8 |
| root `src/` | 498 | 3 | — |
| `tests/` | 6,632 | 24 files | — |

**Files above the documented ~600-line split threshold: 18.** Largest:

| File | Lines |
|------|-------|
| `crates/tui/src/ui/app.rs` | 6,138 |
| `crates/tui/src/ui/components/messages.rs` | 2,203 |
| `crates/core/src/agent/loop_agent.rs` | 2,144 |
| `crates/config/src/lib.rs` | 2,114 |
| `crates/tui/src/ui/components/input.rs` | 1,332 |
| `crates/core/src/agent/orchestra.rs` | 1,243 |
| `crates/api/src/api/types.rs` | 1,111 |

**Healthy signals:** 1 real `TODO` in the whole tree, headers validate clean, 73 files carry inline test modules, feature matrix builds.

---

## 2. Findings

### F1 — `api` is the least-tested, most panic-prone crate (highest risk)
4,837 LOC, **4 inline tests**, and **31 of the codebase's 51 non-test `unwrap()`/`expect()` calls** — concentrated in `handlers/agents.rs` (17) and `handlers/stream.rs` (14), almost all `Event::default().json_data(..).unwrap()` in the SSE mapping. This directly violates the documented "never `unwrap()` in production code" rule, and a serialization failure panics the streaming task rather than degrading the event. This is also the exact surface `apps/web` depends on.

### F2 — CI is not running on the default branch
`.github/workflows/ci.yml` triggers on `push: branches: [main]`, but the repository's default branch is **`master`**. Push CI never fires; only `pull_request` runs. CI also never exercises `apps/web` (eslint + `node --test`) or any Python (`python-sdk`, `benchmarks/`, `runtime/`). *(Fixed 2026-08-06.)*

### F2b — CI runs less than half the test suite (discovered while executing P0)
`verify.sh` ends with `cargo test --features full`, which in a workspace with a root package selects **only the root package** — the facade's (zero) unit tests plus the `tests/` integration suites. Measured:

| Command | Tests run |
|---------|-----------|
| `cargo test --features full` (verify.sh / CI) | **401** |
| `cargo test --workspace --all-features --no-fail-fast` | **828** (827 pass, 1 fails) |

So the ~427 inline unit tests across `crates/*` — 196 in `tui`, 115 in `core`, 21 in `rag`, 20 each in `runtime`/`context`, 18 in `config`, and the rest — have never run in CI. That is how F2c stayed invisible.

### F2c — A pre-existing test failure, and the inert policy layer behind it
`api::handlers::execute::tests::validate_execute_permissions_denies_policy_gated_command` fails (confirmed pre-existing: it fails with all P0 changes stashed). Root cause chain:

1. `Policy::default()` (`crates/core/src/policy/mod.rs:98`) hard-codes `mode: ApprovalMode::Auto` — even though `ApprovalMode` itself derives `#[default] Ask`.
2. In `Auto` mode `Policy::needs_approval` returns `false` unconditionally.
3. `Policy::from_config(_config)` ignores its argument entirely — the codebase's one real `TODO` (`policy/mod.rs:135`).
4. `loop_agent.rs:205` builds agents with `Policy::default()`, and nothing in the TUI, API, or CLI path ever calls `with_policy` with an `Ask`/`Strict` policy. Only `benchmark/runner.rs` maps a mode string.

Net effect: the `Policy` layer is **inert in every default path**; real approval gating comes from `PermissionEnforcer`/`PermissionMode`. The `/execute` HTTP endpoint's second gate (`policy.needs_approval`) can therefore never fire. The failing test asserts the *documented* behavior, so it is the code (or the documentation) that is wrong — not the test.

Note that `CLAUDE.md`, `AGENTS.md`, `GEMINI.md`, and `README.md` all state Policy's default is "**Ask** (default; ask only for dangerous ops)". That claim is currently false.

**This needs a decision before any code change** (see P0.6) — it is a security-gating behavior question, not a cleanup.

### F3 — `app.rs` is a 6,138-line module with clean, unexploited seams
Roughly 4,700 lines of implementation plus a ~1,400-line test module. The `App<C>` struct itself does not begin until line 3600 — everything before it is self-contained helper types: `LiveViewportRuntime`, `StreamingBuffer`, `TagFilter`, `ToolMonitorNode`/`ToolMonitorState`/`ToolActivitySnapshot`, `AppMode`, `SessionAction`, `RewindCheckpointRecord`, `CodeSnapshot(Summary)`, `HooksDialogState`, `RewindDialogState`, `RewindConfirmState`, `SlashDialogState`, `TransientSlashResponse`. 214 methods total, clustering into `switch_*` (14), `handle_*` (14), `render_*` (13), `slash_*` (6), `record_*` (6).

### F4 — The `core` ↔ leaf-crate boundary is undefined
Seven concerns exist in both places. Some are genuine thin shims — `core/src/context.rs` (35 lines), `prompts.rs` (22), `telemetry.rs` (27) are pure `pub use`. Others hold substantial parallel logic: `core/src/permissions.rs` (460 lines) vs `crates/permissions` (240), `core/src/commands/` (1,356 lines across 4 files) vs `crates/commands` (279). Nothing documents which layer owns what, so the next extraction is a coin flip.

### F5 — `crates/config` is a 2,114-line single-file crate
Layered settings loading, TUI settings, permission rules, and provider config all live in one `lib.rs` — the largest single-file module in the workspace and a merge-conflict magnet given how often settings keys are added (`tui.language` landed there this week).

### F6 — Localization is half-migrated with no drift detection
7 of 19 TUI components reference `i18n::`; ~81 candidate literal strings remain in the other 12. The TUI catalog (144 keys) and the web catalog are independent with no shared key inventory, so a string translated in one UI silently stays English in the other.

### F7 — Dead and stray artifacts
- `crates/tools/src/` — empty directory, not a workspace member.
- `test_ratatui` — a stray Mach-O binary at the repo root (untracked and already gitignored; safe to delete). `*.profraw` is cleaned by `make clean` but was not gitignored.
- `python-sdk/` declares `pytest`/`pytest-asyncio` dev deps and ships **zero** tests.
- Tracked root docs `benchmarks.md` and `code_review.md` sit beside `README.md`; `docs/` mixes reference material (`ARCHITECTURE.md`, `HTTP_API.md`, `TOOLS.md`) with point-in-time artifacts (`CITE_AND_PASTE_PLAN.md`, `MULTI_AGENT_TEAM_HARDENING_PLAN.md`, `ROADMAP_PARITY.md`, `memory-agent-briefing-20260503T162117Z.md`, generated `header_map.html` / `header_mindmap.mmd`).

---

## 3. Plan

### P0 — Cheap, high leverage (half a day total)

**Status: P0.1–P0.4 done 2026-08-06. P0.5 and P0.6 are new, opened by what P0 execution surfaced.**

**P0.1 Fix the CI trigger.** ✅ `push` now covers `master` and `main`.

**P0.2 Extend CI to the non-Rust surfaces.** ✅ Added a `web` job (`npm ci` → `npm run lint` → `npm run test`, Node 20, npm cache keyed on `apps/web/package-lock.json`). Verified locally: eslint clean, 15/15 tests pass. A `python` job still waits on P2.2.

**P0.5 Make CI run the whole suite (new — blocked on P0.6).** ✅ `verify.sh` now runs `cargo test --workspace --all-features --no-fail-fast` instead of `cargo test --features full`, covering all member-crate unit tests in addition to the `tests/` integration suites. CI coverage: **401 → 1,233 tests** (832 unit/doc + 401 integration), all green.

**P0.6 Decide what `Policy` is for (new — needs a human decision).** ✅ Resolved 2026-08-06 with **Option B** (make today's behavior explicit). The unreachable `policy.needs_approval` gate in `/execute` and its failing test were removed; `CLAUDE.md`, `AGENTS.md`, `GEMINI.md`, and `README.md` were corrected to state that `PermissionEnforcer` is the default/always-on gate and `Policy`/`ApprovalMode` is opt-in (only via explicit `with_policy`). Option C (folding `Policy` into `PermissionEnforcer`) is deferred to P1.3. F2c leaves three coherent options:

| Option | Change | Consequence |
|--------|--------|-------------|
| **A. Make it real** | `Policy::from_config` reads config; `Policy::default()` uses `ApprovalMode::Ask` | Matches all four instruction files and the failing test. Starts gating writes/pipes in the agent loop *and* `/execute` — re-verify the 31 `tool_approval_test` cases and TUI UX. |
| **B. Make it explicit** | Keep `Auto` default; delete `/execute`'s dead `needs_approval` branch and the failing test; document that `PermissionEnforcer` is the only gate | Smallest diff, honest about today's behavior. Requires correcting the "Ask (default)" claim in `CLAUDE.md`, `AGENTS.md`, `GEMINI.md`, `README.md`. |
| **C. Merge the layers** | Fold `Policy`'s dangerous-pattern matching into `PermissionEnforcer`, delete `Policy` | Removes the two-gate confusion for good; largest diff; do it as part of P1.3. |

Recommendation: **B now** (stop the documentation from lying, unblock P0.5 the same day), then **C** during P1.3, since two overlapping approval layers is the same class of problem as F4.

*Whatever is chosen, keep the test:* under A it should pass; under B it should be deleted with the branch it covers, not left failing.

**P0.3 Eliminate the `api` panics.** ✅ Added `pub(crate) fn sse_event(name: &str, payload: impl Serialize) -> Event` in `crates/api/src/api/handlers/mod.rs`, degrading to the protocol's own `error` event on serialization failure. All 31 `unwrap()` call sites in `handlers/agents.rs` (17) and `handlers/stream.rs` (14) now route through it, and `#![deny(clippy::unwrap_used, clippy::expect_used)]` on `crates/api/src/lib.rs` prevents regression — verified by probe (a scratch `unwrap` produces `error: used unwrap() on an Option value`, lint level attributed to `lib.rs:21`). Two unit tests cover the success and fallback paths. `api` is now at **0** non-test `unwrap`/`expect`, down from 31; workspace total 51 → 20.

**P0.4 Delete `crates/tools/`; gitignore `*.profraw`.** ✅ Both done.

### P1 — Structural (1–2 weeks, sequence matters)

**P1.1 Decompose `crates/tui/src/ui/app.rs` (do this first — it unblocks everything else in the TUI).**
Purely mechanical, no behavior change. Suggested split into `crates/tui/src/ui/app/`:

| New module | Extracted contents | Approx lines |
|-----------|--------------------|--------------|
| `viewport.rs` | `LiveViewportRuntime`, `StreamingBuffer` | ~250 |
| `tool_monitor.rs` | `ToolMonitorNode`, `ToolMonitorState`, `ToolActivitySnapshot`, `MonitorStatus` | ~350 |
| `tag_filter.rs` | `TagFilter` | ~300 |
| `dialogs.rs` | `HooksDialogState`, `RewindDialogState`, `RewindConfirmState`, `SlashDialogState`, `TransientSlashResponse` | ~200 |
| `rewind.rs` | `RewindCheckpointRecord`, `CodeSnapshot`, `CodeSnapshotSummary`, `record_*` methods | ~400 |
| `sessions.rs` | the 14 `switch_*` / `SessionAction` methods | ~600 |
| `mod.rs` | `App<C>`, `AppMode`, `handle_*` / `render_*` dispatch | ~1,200 |

Move the `#[cfg(test)] mod tests` block with each extracted type; anything genuinely end-to-end goes to `crates/tui/tests/` behind `test-utils`.
*Verify:* `cargo test -p tui --features test-utils` before and after must show the same test count and all pass; `cargo clippy -p tui --all-features -- -D warnings`.
*Note:* run GitNexus `impact` on `App` first — it is the highest-fan-in type in the TUI.

**P1.2 Split `crates/config/src/lib.rs`** into `lib.rs` (re-exports + `load`), `layers.rs` (precedence), `provider.rs`, `tui.rs`, `permissions.rs`. Same header discipline per new file.
*Verify:* `cargo test -p config`; settings-precedence tests unchanged.

**P1.3 Write down the `core` ↔ leaf ownership rule, then make the code match.** Proposed rule, to be added to `docs/ARCHITECTURE.md` and summarized in `CODING_STYLE.md`:
> A leaf crate owns the data model and all pure logic for its concern. `core` may only contain (a) a pure `pub use` shim re-exporting it, or (b) glue that legitimately depends on other `core` subsystems (agent, client, tools). No concern has two implementations.

Then resolve the two violations: move the pure parts of `core/src/permissions.rs` into `crates/permissions`, and split `core/src/commands/` (keep agent-dependent command execution in `core`, move parsing/formatting to `crates/commands`).
*Verify:* per-crate `cargo test -p permissions -p commands -p core`; the shim files should shrink to `pub use` only.

**P1.4 Bring `api` to a defensible test level.** Table-driven tests for `bridge_event_to_sse` covering every `BridgeEvent`/`AgentEvent` variant (this is where P0.3's fallback path gets asserted), plus handler tests for `external_sessions.rs`, `approvals.rs`, and `execute.rs` against a mock bridge. Target ≥ 30 tests. Sequence after P0.3 so the tests encode the non-panicking contract.

### P2 — Consistency & hygiene (opportunistic)

**P2.1 Finish the i18n migration.** Route the remaining 12 TUI components through `i18n::text`, then add a test asserting the TUI and web catalogs cover the same logical key set (and that no catalog key is orphaned). Cheapest durable form: a shared JSON key inventory checked by both `cargo test -p tui i18n` and `apps/web` `node --test`.

**P2.2 Give `python-sdk` the tests its manifest already promises.** A smoke suite against `apps/web/mock-server.mjs` or a live `--server` instance; wire into the CI `python` job from P0.2.

**P2.3 Documentation topology.** Move `benchmarks.md`, `code_review.md`, and `audit.md` under `docs/`; split `docs/` into `docs/reference/` (architecture, HTTP API, tools, compaction, testing, design system) and `docs/plans/` (dated, archivable), and move generated header artifacts (`header_map.html`, `header_mindmap.mmd`, `HEADER_*.md`) under `docs/generated/`. The four instruction files now point into `docs/` by name, so do this as one commit and update those pointers in the same change.

---

## 4. Explicit non-goals

- **Do not add `rustfmt.toml`.** The defaults are the style (4 spaces).
- **Do not reintroduce `team` / `supervisor` / `mesh` features.** `orchestra` is the only multi-agent flag.
- **Do not split `core` further** until P1.3's ownership rule is written down — more crates without a rule makes F4 worse, not better.
- **Do not merge `crates/runtime` into `core`.** The model/logic separation there is one of the cleaner boundaries in the workspace.

---

## 5. Suggested order

```
P0.1 → P0.2 → P0.3 → P0.4        ✅ done 2026-08-06
P0.6 → P0.5                       ✅ done 2026-08-06 (CI runs all 1,233 tests)
P1.1                              (mechanical, unblocks TUI work)
P1.4                              (locks in P0.3's contract)
P1.2 → P1.3                       (layering, needs the written rule first)
P2.1 → P2.2 → P2.3                (opportunistic)
```

`F1`'s "`api` has 4 tests" is now 6, and the two new ones cover the panic class specifically. The remaining coverage work is unchanged and still tracked as P1.4.

Every step ends with `./verify.sh`, and per repo policy: GitNexus `impact` before editing a symbol, `detect_changes()` before committing.
