# Agent Workflow Checklist

This checklist is the default workflow for coding agents working on Amadeus.

## Architectural Rules

- Keep all agent commands, orchestration, policies, session logic, and core workflows inside the core SDK/runtime crates.
- Treat the TUI as an adapter over exported core APIs. The TUI may compose, display, and route input, but it must not become the source of truth for agent behavior.
- Do not add new business logic only to the TUI path if the same behavior should exist for HTTP, tests, or future frontends.
- Prefer exported core types, traits, and harnesses over direct coupling to concrete TUI internals.
- When adding a command or agent capability, implement it in core first, then wire it into the TUI.

## Change Checklist

Before editing:
- Run GitNexus impact analysis for every function, method, or type you plan to change.
- Identify whether the requested behavior belongs in core, API, or TUI.
- If a TUI change requires new behavior, define or extend a core API first.

During implementation:
- Update interfaces in this order: core contracts, core implementation, API/TUI adapters, tests.
- Keep TUI code focused on rendering, event handling, navigation, and presentation state.
- Keep core code deterministic enough to test without an interactive terminal.
- Preserve `amadeus::...` compatibility paths unless there is an intentional breaking change.
- Maintain source-file headers for every touched in-scope Rust file.

After implementation:
- Run `cargo check --features full`.
- Run the most relevant targeted tests for the changed area.
- For TUI work, run deterministic scenario coverage before interactive acceptance checks.
- Run `gitnexus_detect_changes()` before committing.

## TUI Correctness Checklist

1. Core first
- Verify the state transition, command behavior, and approval policy in core tests.
- Add or update mocked scenario coverage for the underlying agent flow before relying on an interactive terminal.

2. Deterministic TUI integration
- Use mocked clients and scripted input sequences for TUI integration tests.
- Assert stable text anchors, state transitions, and session/tool visibility.
- Avoid brittle assertions on exact spacing, transient spinners, or theme-specific styling.

3. Interactive acceptance
- Use `tmux-cli` scenario packs from `docs/TMUX_TEST_FLOW.md`.
- Capture after each meaningful transition.
- Validate dashboard, completion, approval, session switching, tool monitor, and error rendering with focused scenarios.

4. Failure analysis
- Use `tui_capture.log` and testflow artifacts for frame-level debugging when a redraw or ordering bug is unclear.
- Fix the core state transition first if the visual issue reflects inconsistent underlying state.

## Commit Discipline

- Prefer small, mechanical commits with one purpose each.
- Separate docs, workspace plumbing, file moves, and behavioral fixes when possible.
- Stage only the files you changed.
- Do not bundle unrelated user changes into your commit.

## Recommended Refactor Order

1. Define the target boundary.
- Core SDK/runtime owns agent behavior, commands, tools, policy, sessions, and shared harnesses.
- TUI crate owns rendering, keyboard/mouse events, themeing, and presentation adapters.
- API crate owns HTTP transport and request/response adapters over the same core runtime.

2. Move code without changing behavior.
- Extract crates first.
- Preserve module paths through facade re-exports.
- Verify builds before making follow-up API cleanup changes.

3. Tighten boundaries after the move.
- Remove accidental TUI ownership of command logic.
- Replace cross-layer reach-through with exported core APIs.
- Add tests that prove the TUI consumes the same behavior the API and test harnesses use.
