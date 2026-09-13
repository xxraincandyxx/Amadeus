# Testing the Web Client — A Coding-Agent's Guide

> How to black-box test the Amadeus React workspace end to end — with the mock API or the real
> Gemma backend — find real bugs, and land fixes as focused, verified pull requests.
>
> Companion to `docs/WEB_DESIGN_SYSTEM.md` (the *visual contract* a fix must respect) and
> `docs/TUI_TESTING.md` (the equivalent automated path for the terminal UI).

## TL;DR

```bash
# Terminal 1 — backend. Either the deterministic mock…
cd apps/web && npm run mock-api            # port 3000

# …or the real runtime (uses .amadeus/settings.json, e.g. the Gemma vLLM endpoint)
cargo run --features full -- --server 3000

# Terminal 2 — the client
cd apps/web && npm run dev                 # http://localhost:5173

# Verification gate for any fix
cd apps/web && npm run test && npm run lint && npm run build
```

Point the client at `http://127.0.0.1:3000` either way; the UI is backend-agnostic.

## The two backends (use both)

| Backend | Strengths | Use it for |
| --- | --- | --- |
| Mock (`mock-server.mjs`) | Deterministic, offline, scripts multi-agent sessions, approvals, SSE quirks | State-machine tests, offline/error paths, repeatable demos |
| Real runtime (`--server 3000`) | Real streaming, real tool calls, real Markdown, real latency | Streaming/tool/approval UX, history fidelity, anything the mock hardcodes |

Prefer the real backend for bug hunting: the mock hardcodes shapes that diverge from the server
(a past example — the mock's `/tools/catalog` had no `description` and invented `core`/`extended`
levels; only the real server showed what the Tools workspace actually renders). Keep `.amadeus/settings.json`
provider config as-is; the project Gemma model (`openai`-compatible vLLM) needs no key.

Reconnecting the client from the mock to the real server while stale sessions linger is itself a
test: the client must drop vanished sessions and re-seed a Main Agent without crashing.

## The mental model (read this first)

The client renders every conversation through **two paths**, and divergence between them is the
most productive bug mine:

1. **Live path** — SSE events reduced by `reduceEvent` (`apps/web/src/sessionState.js`).
2. **Hydrated path** — `GET /v1/sessions/{id}/history` rebuilt by `historyToTimeline`, which
   replaces the live timeline when a turn finishes, when switching sessions, and on reload.

A turn that looks right while streaming can still lose content when hydration replaces it. Past
findings of this class: tool outputs were attached per-message so they never matched their
cross-message `tool_use` block. Test every artifact **after reload**, not only live.

| Artifact | Live path | Hydrated path |
| --- | --- | --- |
| Assistant text | `text` events → `done` commit | `text` blocks |
| Reasoning | `thinking` events → committed on `done` | `thinking` blocks or preserved live reasoning |
| Tool call | `tool_start`/`tool_input`/`tool_done` | `tool_use` + cross-message `tool_result` |
| Notices | `error`/`compaction` events | absent |

## Driving the UI

Use a DOM snapshot (`role`/`name` tree) as ground truth and build locators only from names visible
in the latest snapshot; verify `count()` before acting. Screenshots are for visual judgement
(spacing, contrast, hierarchy) — sample computed styles for contrast numbers, never eyeball ratios.

Composer specifics that matter:

- Type per-keystroke (`pressSequentially`/equivalent). `fill()` + `Enter` can skip React state
  updates and produces false "Enter doesn't send" reports.
- The slash palette opens only when `/` is character zero; typing an argument closes it.
- IME safety: dispatch a `keydown` with `key: "Enter"` and `isComposing: true` — the draft must
  **not** send (regression guard for the CJK path).

React Flow canvases (both designers):

- Connecting nodes needs **pointer events** (`pointerdown` on the source handle → `pointermove` →
  `pointerup` on the target handle). Mouse-event drags do nothing; that is an automation artifact,
  not an app bug.
- Node IDs are read-only by design; validation warnings are only rendered in the overview panel,
  so deselect the node (click the pane) to read them.

Environment gotchas:

- The sidebar re-renders on a 2.5 s `refreshSessions` poll; automated clicks can stall on
  actionability. Re-snapshot, or click via one in-page `el.click()` and move on.
- Workflow persistence to `localStorage` (`amadeus.workflowLibrary.v1`) is debounced — never
  assert on stored state immediately after a click; read the live DOM instead.
- Locale and preference keys: `amadeus.language`, `amadeus.themeColor`, `amadeus.apiUrl`,
  `amadeus.activeSession`. Test both English and 简体中文; untranslated or raw-browser error
  strings are findings.

## What to test (checklist)

Run this matrix in both languages, at 1280×720 and at the narrow layout:

- **Conversation**: empty/welcome state, streaming text and Markdown (headings, lists, tables,
  code blocks with copy), reasoning disclosure, tool cards (input **and** output), cancellation
  mid-turn, error events, export (`/export markdown|json`).
- **Sessions**: create/rename/close, switching hydrates correctly, stale sessions after a backend
  restart, slash commands (`/tools`, `/context`, `/settings`, unknown command error).
- **Settings**: theme presets and custom hex, language switch, connection Test with a dead URL
  (localized, actionable error) and a live URL, reset-to-default.
- **Agent designer / Task workflows**: duplicate, rename, node add/delete, connect, validation
  chip and diagnostics, undo/redo, import (invalid JSON → inline error), export, persistence
  across reload, destructive-delete confirmation.
- **Accessibility spot checks**: contrast ≥ 4.5:1 for text below 18 px (compute from computed
  styles), visible `:focus-visible`, icon-only buttons have accessible names, status not conveyed
  by color alone, `prefers-reduced-motion` honored.

## From finding to merged fix

1. **Reproduce** against the real backend and note the exact steps; capture before-state
   (snapshot, screenshot, computed values — numbers, not adjectives).
2. **Root-cause in code.** Prefer the smallest honest fix; keep behavior otherwise untouched.
3. **GitNexus first**: `impact <symbol> --direction upstream` before editing. `risk: UNKNOWN`
   is unresolved — confirm callers with a text search before treating it as low risk. Run
   `detect-changes` before every commit.
4. **Branch per fix**, conventional commit (`fix(web): …`).
5. **Verify**: unit tests (add a regression test when the fix is logic), `lint`, `build`, and a
   live re-check of the exact repro in both locales.
6. **PR**: problem, root cause, fix, verification evidence (before/after numbers or screenshots
   for visual changes), then merge to master and re-verify the merged result.

Design-facing fixes must satisfy `docs/WEB_DESIGN_SYSTEM.md`; if a change adds a new UI pattern,
update that document in the same PR.
