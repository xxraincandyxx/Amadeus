# Amadeus tmux-cli Test Flow

This runbook defines the local-first `tmux-cli` acceptance flow that coding agents should use when debugging the Amadeus TUI. It is centered on deterministic prompts, stable text captures, and focused scenario packs rather than one brittle end-to-end script.

## Scope

Use this flow to validate the TUI and the major subsystems it exposes:
- startup dashboard and footer state
- input, completion, and mode switching
- slash commands and sidebars
- session switching and sub-agent navigation
- approval modals and policy-sensitive tool execution
- tool monitor, nested tools, truncation, and error rendering
- deterministic supervisor and multi-session visibility

Do not make live provider/API checks part of the core acceptance path. The baseline flow should remain runnable with local fixtures and deterministic behavior.

## Preconditions

1. Build the app with:
   ```bash
   cargo build --features full
   ```
2. Run targeted unit or integration tests for the area you are changing before relying on TUI captures.
3. Launch the TUI inside `tmux-cli` instead of attaching directly to your terminal.
4. Prefer launching the prebuilt binary after a successful build to avoid compile noise in captures:
   ```bash
   tmux-cli send "cd /Users/raincandy_u/Dev/amadeus && target/debug/amadeus" --pane=<pane>
   ```
   Use `cargo run --features full` only when you specifically need to validate the launch path.
4. Prefer an 80x24 or larger pane when comparing splash layout across runs.
5. When sending single-key shortcuts like `?`, disable the default Enter suffix:
   ```bash
   tmux-cli send "?" --pane=<pane> --enter=False
   ```

## Core Loop

Use the same loop for every debugging task:

1. Launch a fresh pane:
   ```bash
   tmux-cli launch "bash"
   ```
2. Start the TUI:
   ```bash
   tmux-cli send "cd /Users/raincandy_u/Dev/amadeus && cargo run --features full" --pane=<pane>
   ```
3. Wait for a stable frame:
   ```bash
   tmux-cli wait_idle --pane=<pane> --idle-time=2.0 --timeout=30
   ```
4. Capture the screen:
   ```bash
   tmux-cli capture --pane=<pane>
   ```
5. Reproduce the behavior with focused prompts or key presses.
6. Capture after each important transition.
7. Patch the code.
8. Re-run the same focused scenario before widening to smoke coverage.
9. Clean up the pane:
   ```bash
   tmux-cli interrupt --pane=<pane>
   tmux-cli kill --pane=<pane>
   ```

## Stable Assertion Style

Treat `tmux-cli` as a text-state inspector, not a screenshot diff tool.

Prefer assertions like:
- capture contains `Try "how does src/main.rs work?"`
- capture contains `? for shortcuts`
- capture contains `root>`
- capture does not contain `Tips for getting started`

Avoid depending on:
- exact whitespace alignment
- full-screen snapshots
- transient spinner frames
- color or border glyph fidelity

## Scenario Packs

### 1. Startup Smoke

Purpose: confirm the TUI launches into the expected empty-session state.

Steps:
1. Launch the TUI in a fresh pane.
2. Wait for idle.
3. Capture the splash.

Expected anchors:
- `Amadeus v0.1.0`
- `Try "how does src/main.rs work?"`
- `? for shortcuts`
- `[root]`

Expected absences:
- `Tips for getting started`
- `Welcome`

### 2. Help and Mode Switching

Purpose: verify the lightweight help affordance and mode transitions.

Steps:
1. From the empty splash, send `?` with `--enter=False`.
2. Capture the shortcuts overlay.
3. Send `Esc`.
4. Capture again.

Expected anchors:
- the overlay contains shortcut labels like `Next session` or `To parent`
- after `Esc`, the overlay is gone and the input hint row returns

### 3. Input and Completion

Purpose: validate editable input behavior without involving provider responses.

Steps:
1. Type `/`.
2. Capture the completion popup.
3. Press `Tab`.
4. Capture the selected completion state.
5. Press `Esc` to leave completion.
6. Type a short prompt such as `abc`.
7. Send `Ctrl+B`, `Ctrl+F`, `Ctrl+P`, and `Ctrl+N` in focused scenarios and capture after each.

Expected anchors:
- slash command suggestions are visible
- `Tab` is consumed by completion when the popup is active
- returning to the input field hides the popup cleanly
- `Ctrl+B/F` move like `Left/Right`
- `Ctrl+P/N` behave like `Up/Down` for the active composer state

### 4. Session Navigation

Purpose: validate independent session creation and traversal.

Steps:
1. Send `/new-agent`.
2. Wait for idle.
3. Capture the footer session tabs.
4. Send a literal tab through `tmux-cli`:
   ```bash
   tmux-cli send "\t" --pane=<pane> --enter=False
   ```
5. Capture.
6. Send raw `Shift+Tab` or `BTab`.
7. Capture.

Expected anchors:
- session tabs change from `[root] session1` to `root [session1]`
- `Tab` switches away from the current session
- `Shift+Tab` switches back
- the capture taken immediately after a switch is never blank

Regression checkpoint — blank screen after switch:
- capture immediately after a literal `Tab` sent with `tmux-cli send "\t" --enter=False`
- if the pane is blank, the switch path cleared or rebuilt the terminal without completing the redraw in the same event cycle
- the fix: keep the existing inline terminal alive during the switch and replay the active session history into that same page
- verify: `switching_from_populated_to_empty_session_allows_immediate_redraw` unit test passes

Regression checkpoint — dashboard flicker on empty session switch:
- after switching to an empty session, the dashboard (`Amadeus v0.1.0`, mascot art) must remain visible
- the dashboard must not appear briefly then vanish on the next tick
- verify: `switching_between_empty_sessions_allows_immediate_redraw` unit test passes
- verify: `switching_from_empty_to_populated_session_defers_redraw` unit test passes

Populated-session regression:
1. Create `session1`.
2. Send `hello?` and wait for the assistant response.
3. Capture the session and note a stable reply line.
4. Switch back to `root`.
5. Switch again into `session1`.
6. Capture immediately after the second switch.

### 4a. Citation And Paste Composer Regression

Purpose: validate that `@` cite suggestions and paste handling do not hide the composer.

Steps:
1. Start from the empty splash in a fresh pane launched from `target/debug/amadeus`.
2. Type `@`.
3. Capture immediately.
4. Type `rev` or another known workspace filename prefix.
5. Capture the cite popup.
6. Press `Tab`.
7. Capture the accepted cite state.
8. Repeat with a known folder prefix such as `@doc`.
9. Capture the folder cite popup.
10. Paste a single absolute file or folder path with bracketed paste or `tmux-cli send`.
11. Capture again.

Expected anchors:
- the composer prompt `❯` remains visible after typing `@`
- the raw in-progress query such as `@rev` remains visible before acceptance
- cite suggestions appear without replacing the editor area
- after `Tab`, the composer shows a rendered cite token such as `@reviewer.md`
- folder suggestions appear for workspace directories such as `docs`
- pasting a single file or folder path inserts a cite instead of hiding the input row

Regression checkpoint — cite popup must not consume the composer row:
- if typing `@` makes the input row disappear, the app/layout height contract is wrong
- verify: cite completion height is counted in the app-level input area reservation
- verify: `render_keeps_composer_visible_when_citation_completion_is_open` passes

Expected anchors:
- the active label is `root [session1]`
- `Amadeus v0.1.0` appears before the restored conversation history
- `Try "how does src/main.rs work?"` appears before the restored conversation history
- the assistant reply appears exactly once
- `turn 1` appears once for that turn

Regression checkpoint — populated session switch must include the dashboard:
- if both sessions already have history, switching into either one must still prepend the dashboard block before the restored transcript
- the replay should look like a same-page append, not like a raw transcript dump that starts mid-conversation
- verify: `recycle_terminal_after_switch_replays_populated_session_history` unit test passes
- verify: `test_session_switch_replay_prepends_dashboard_before_existing_history` unit test passes

Notes:
- Prefer `tmux-cli send "\t" --enter=False` for `Tab` in remote sessions.
- `tmux send-keys` can be flaky for `Tab`/`Ctrl+I` in remote tmux-cli sessions; use it mainly for chords that tmux-cli cannot express cleanly.

### 5. Parent and Child Session Traversal

Purpose: validate direct parent/child navigation once a child session exists.

Steps:
1. Create or expose a child/sub-agent session through a deterministic scenario.
2. Capture the breadcrumb with both parent and child visible.
3. Send `Ctrl+]`.
4. Capture.
5. Send `Ctrl+[`.
6. Capture.

Expected anchors:
- `Ctrl+]` moves into the direct child when present
- `Ctrl+[` returns to the parent
- if no child or parent exists, the capture remains unchanged

Notes:
- Some terminals collapse `Ctrl+[` to `Esc`; prefer explicit tmux key sends when testing this path.

### 6. Approval Modal Flow

Purpose: validate ask-mode interaction without depending on live external systems.

Steps:
1. Launch with a deterministic scenario or config that triggers an approval-requiring tool.
2. Capture the approval dialog.
3. Send `n` and capture denial state.
4. Re-run and send `y`.
5. Capture the approved execution state.

Expected anchors:
- approval modal is visible
- denial produces a visible cancelled/denied outcome
- approval proceeds to tool execution

### 7. Tool Monitor and Nested Tools

Purpose: validate live tool rendering and drill-in affordances.

Steps:
1. Use a deterministic prompt that triggers a visible tool.
2. Capture while the tool is pending.
3. Wait for completion and capture again.
4. If nested tools are present, use `Ctrl+X i/k/j/l` to navigate.

Expected anchors:
- running indicator appears while work is active
- tool name and summarized input/output are visible
- nested navigation hints appear when relevant

### 8. Error and Truncation Rendering

Purpose: validate failure surfaces and long-output containment.

Steps:
1. Run a deterministic prompt or fixture that produces a tool error.
2. Capture the error panel.
3. Run a deterministic command that emits long output.
4. Capture the truncated rendering.

Expected anchors:
- error state is clearly marked
- large output is truncated rather than flooding the transcript
- footer and input remain usable after the error

### 9. Sidebar and Command Surface

Purpose: validate non-transcript UI chrome.

Steps:
1. Toggle file sidebar with `Ctrl+B`.
2. Capture.
3. Toggle skills sidebar if enabled.
4. Capture.
5. Return to the base layout.

Expected anchors:
- sidebar visibility changes are obvious in the capture
- the app returns cleanly to the normal single-pane composition

### 10. Multi-Session File Locks

Purpose: validate that file locking prevents stale writes across two or more sessions.

Precondition:
- use a deterministic scripted client or local fixture that makes specific sessions issue `read_file`, `write_file`, and `edit_file` on the same relative path
- seed a small shared file such as `lock-demo.txt` before launching the TUI

Steps:
1. In `root`, trigger a deterministic `read_file` on `lock-demo.txt`.
2. Capture and confirm the read completed.
3. Create `session1`.
4. In `session1`, trigger a deterministic `write_file` that changes `lock-demo.txt`.
5. Capture and confirm the write completed.
6. Switch back to `root`.
7. In `root`, trigger a deterministic `edit_file` against the stale content it read before `session1` wrote.
8. Capture the failure state.
9. Optionally create `session2` and trigger a fresh `read_file` on `lock-demo.txt`.
10. Capture the success state with the latest content.

Expected anchors:
- `root` can read the shared file before any competing write
- `session1` can write the shared file after `root` finishes reading
- the stale follow-up edit from `root` fails because the old text is no longer present
- a fresh third-session read sees the latest file contents

Regression checkpoint — stale session edit must not overwrite newer content:
- if session A reads and session B writes the same file, session A must not be allowed to apply an edit based on the stale text it saw earlier
- verify: `stale_edit_is_rejected_after_another_session_writes` integration test passes

Regression checkpoint — blocked reader must recover after writer finishes:
- if one session holds the write lock, another session must remain blocked only until the writer releases it
- verify: `waiting_reader_proceeds_after_writer_session_releases_lock` integration test passes

## Recommended Run Order

For normal bug work:
1. one focused scenario pack for the subsystem you are changing
2. startup smoke
3. one adjacent pack if your change touched shared navigation or layout code

Before merging larger TUI changes:
1. startup smoke
2. help and mode switching
3. input and completion
4. session navigation
5. approval modal flow
6. tool monitor and nested tools
7. error and truncation rendering

## Debugging Heuristics

If the capture is blank:
- check whether the process exited immediately
- verify the pane is still attached to a running shell
- confirm the app was launched inside `tmux-cli`, not replacing the pane process unexpectedly

If a shortcut appears broken:
- retry with `--enter=False` for single-key shortcuts
- fall back to `tmux send-keys` for control chords that `tmux-cli` misses

If the UI looks stale:
- use `tmux-cli wait_idle` before capturing
- capture multiple frames if a spinner or stream is active

If the pane is wedged:
- `tmux-cli interrupt --pane=<pane>`
- if needed, relaunch a fresh pane instead of debugging a polluted shell state

## Agent Rules

Coding agents using this flow should:
- start narrow with the scenario pack closest to the bug
- keep captures tied to named checkpoints
- prefer deterministic prompts and fixtures over ad hoc exploration
- verify the exact interaction they changed in tmux before claiming the TUI behavior is fixed
- only expand to a broader pass after the focused scenario is green
