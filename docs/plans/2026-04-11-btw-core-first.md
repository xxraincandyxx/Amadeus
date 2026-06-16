# /btw Core-First Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a real `/btw` side-question feature without breaking dashboard/layout behavior or moving agent behavior into the TUI.

**Architecture:** Core owns side-question execution and transcript isolation. The TUI only recognizes `/btw`, submits it through the core API, and renders the result in a completion-style drop-up above the composer.

**Tech Stack:** Rust, tokio, ratatui, existing Amadeus core agent/runtime APIs, tmux-cli verification

---

### Task 1: Add core-side side-question execution

**Files:**
- Modify: `crates/core/src/commands/mod.rs`
- Modify: `crates/core/src/agent/loop_agent.rs`
- Test: `crates/core/src/commands/mod.rs`

**Step 1: Write the failing test**
- Add a focused test proving a side question:
- uses history as context
- does not mutate history
- sends no tools
- returns a single text answer

**Step 2: Run test to verify it fails**
- Run: `cargo test -p core side_question --features full`

**Step 3: Write minimal implementation**
- Add a small core API for side questions.
- Keep it frontend-agnostic.

**Step 4: Run test to verify it passes**
- Run: `cargo test -p core side_question --features full`

### Task 2: Add deterministic TUI adapter behavior

**Files:**
- Modify: `crates/tui/src/ui/components/input.rs`
- Modify: `crates/tui/src/ui/app.rs`
- Test: `crates/tui/src/ui/app.rs`

**Step 1: Write the failing test**
- Add tests proving `/btw` uses a drop-up pattern above the composer and does not hide the dashboard/logo.

**Step 2: Run test to verify it fails**
- Run: `cargo test -p tui --features test-utils btw -- --nocapture`

**Step 3: Write minimal implementation**
- Reuse input/completion-style reserved height and rendering.
- Keep execution routing in app/session code thin.

**Step 4: Run test to verify it passes**
- Run: `cargo test -p tui --features test-utils btw`

### Task 3: Verification

**Files:**
- Modify: none

**Step 1: Run targeted checks**
- `cargo check --features full`
- `cargo test -p core side_question --features full`
- `cargo test -p tui --features test-utils btw`

**Step 2: Run tmux-cli acceptance**
- Verify startup dashboard/logo remains fully visible.
- Verify `/btw` usage layout.
- Verify `/btw <question>` layout and dismissal.
- Verify `/btw` while the main stream is active.

