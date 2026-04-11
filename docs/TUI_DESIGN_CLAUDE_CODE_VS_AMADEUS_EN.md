# TUI Design Comparison: Claude Code vs Amadeus (tmux-cli session notes)

> Chinese: [TUI_DESIGN_CLAUDE_CODE_VS_AMADEUS_ZH.md](./TUI_DESIGN_CLAUDE_CODE_VS_AMADEUS_ZH.md)

This document summarizes what we observed when comparing the terminal UIs of **Claude Code** (`claude` CLI) and **Amadeus** (`cargo run --features full` / `target/debug/amadeus`) in isolated **tmux-cli** (`claude-code-tools`) sessions. It covers the **welcome dashboard** and **in-session motion and layout** (slash completions, context compaction, tool-running monitor, streaming live view, status bar, etc.)—not only the first screen. The workflow follows `skills/tui-tmux-debugging.md`. For a **step-by-step comparison runbook**, see the project skill [`.cursor/skills/tui-comparison-tmux/SKILL.md`](../.cursor/skills/tui-comparison-tmux/SKILL.md).

**Scope (important)**: This is product/design observation only. It does **not** require or recommend instructing a coding agent to change Amadeus source code. Any follow-up implementation should be human-reviewed and tracked separately.

---

## 1. How we tested (short)

1. **Environment**: In *remote* mode, `tmux-cli` manages windows under the `remote-cli-session` session. Start with `tmux-cli launch "zsh"` (or `bash`), then `tmux-cli send "<command>" --pane=<pane>`, so a failed process does not drop the pane with no output.
2. **Inspection**: `tmux capture-pane -t remote-cli-session:<window-index> -p` captures plain-text layout (ANSI may be simplified in some setups). Use `tmux-cli wait_idle` before captures when the UI is still updating.
3. **Notes**: Amadeus **exits immediately** if no valid API settings are configured; run from a directory with `.amadeus/settings.json` or `~/.amadeus/settings.json` to see the full TUI (including the dashboard). Claude Code depends on its own auth/billing setup and is independent of this repo.

---

## 2. Claude Code TUI: what it feels like

In a typical 80×24 tmux pane, the **empty-session splash** roughly shows:

- **Brand block**: A compact ASCII/Unicode mark (e.g. `▐▛███▜▌`-style geometry) with **Claude Code version**, **current model / billing line**, and **working directory** on the same visual band.
- **Secondary hint**: One short capability line (e.g. `/model ...` to switch models).
- **Divider**: A full-width `─` rule separating the info block from the input cue.
- **Primary CTA**: A highly visible **sample prompt** (e.g. `Try "edit app.rs to..."`) with a `❯` prefix, teaching the first user utterance.
- **Whitespace**: Many blank lines—**low density, single focal point**—plus `? for shortcuts` as the global help affordance.

**Keywords**: **minimal, single-column story, strong guidance, few exposed controls**. The hierarchy is brand → context → one rule → one sentence → help. There is no dedicated “dashboard” region; sidebars/multi-pane chrome is not emphasized on the splash screen.

---

## 3. Amadeus TUI: dark-red theme + dashboard

At the same pane width, with **empty history and a successful start**:

### 3.1 Dedicated dashboard (largest difference from Claude Code)

- **Title row**: `Amadeus v0.1.0` with accent styling and trailing `─` fill to the line width (`MessagesComponent::render_dashboard_lines`).
- **Welcome line**: Removed; dashboard goes from title row to mascot.
- **Mascot / brand art**: Large **Braille / block** artwork (`FULL_ART` / `FACE_ART`) gated by width—much more presence than Claude Code’s small logo.
- **Positioning copy**: Centered **“amadeus ◈ Premium CLI Coding Interface”** plus **path / project name**.
- **Tips**: A `Tips for getting started` header with bullets (`/help`, `Esc` mode switching, etc.) and a final full-width rule.

Semantic colors come from the theme system; **Dark Red** uses a warm near-black base, **blood-red** accents and links, and reddish grays (`src/ui/themes/dark_red.rs`, `SemanticColors`).

### 3.2 Conversation + input

- After chat begins, the main area shows **turn separators** (e.g. `──────── turn N ────────`), message-style layout, and a bottom **input** strip: gray full-width rules, a **`❯ Try "…"`** line (example from `AMADEUS_TRY_PROMPT`), **character/line counts** on the right of that row, optional **status hint** row (`awaiting approval`, scramble + dots), then a borderless editor with placeholder `Type a message... (Enter: send, Alt+Enter: newline)`.
- Compared to Claude Code’s single **Try** line, Amadeus reads more like **IDE-style transcript + explicit turn markers**.

### 3.3 Two-line footer

Captures often show two summary rows, for example:

- **Top**: Session/agent name, `◈` model, context bar `[░░░░░░░░]`, percent, `◷` session time, etc.
- **Bottom**: `root>`-style cue, `📂` path, `⎇` branch, `◫` sandbox status, etc.

Claude Code’s splash **does not stack** this kind of monitoring strip in the same place; it keeps state closer to the logo block.

### 3.4 Runtime motion and layout (live region, input, status)

The following maps to **non-welcome** behavior (`Session::render`, `render_live_viewport`, and related code under `src/ui/app.rs`, `src/ui/components/*`).

**Main column (top to bottom)**: Live viewport → multi-line **input** (with a top border title) → optional single-line **StatusBar** (when a request is active) → **two-line Footer**. Opening a sidebar (Context / Files / Help, etc.) reserves additional width on the right.

**Live viewport priority** (mutually exclusive, code order): If there is no streaming body, no compaction pending, no running tools, no `stream_rx`, and **no messages**, show the **dashboard**. Otherwise a **focused-border** block shows, in order:

1. **Tool activity**: When tools are running and there is no stream text and no compaction pending, the title is **“ Monitor ”** with a tool summary (tool name + `LoadingIndicator` scramble/ellipsis hint + optional progress) and navigation hints such as `ctrl+x then i/k/j/l` (`render_tool_activity_preview`).
2. **Context compaction**: `/compact` / `/compress` triggers **pending compaction** content from **CompactionAnimator**: a **single muted status line** (`Compacting context` + light dot pulse + percent + elapsed); then a short result state before history updates (`compaction_animation.rs`, `messages.rs`).
3. **Streaming / thinking without body yet**: `LoadingIndicator::prompt_hint()`—**scramble** animation settling on `responding` / `working`, trailing **`.` / `..` / `...`**; approval wait is static **“awaiting approval”** on a **dedicated hint row** under the gray rules (not in the textarea title).
4. **Streaming markdown body**: Title **“ Live ”** with rendered `streaming_buffer` text.

**Input and slash completion**: Input starting with `/` opens a **“ Commands ”** popup (up to six items, `completion.rs`) **below** the input area with a border. **Tab** applies the selection; **Shift+Tab** and **Ctrl+arrow** move selection (`app.rs` key handling). Selected rows use **LightCyan** (ratatui default) alongside theme colors.

**StatusBar** (`status_bar.rs`): When a model turn is active, an extra row may show `thinking` / `generating`, optional **tok/s**, estimated ▲/▼ token counts, and a **⟡** marker while thinking—more **instrumented** than a typical minimal CLI status line.

**Claude Code at runtime (observational)**: During tools and replies, Claude Code tends to keep state **lighter and more fused** into the main transcript, with fewer distinct “monitor” blocks and dual-line gauges. Amadeus explicitly separates **Monitor / Live / compaction** from the **footer context meter**. For a “Claude-like concise” benchmark, capture **multiple runtime frames**, not only the splash.

**tmux caveat**: `capture-pane` is a **single frame**; spinners and scramble need **several captures** or `tmux-cli attach` recording to judge motion; use `wait_idle` for stable post-action frames.

---

## 4. Side-by-side (design axes)

| Axis | Claude Code | Amadeus |
|------|-------------|---------|
| **Splash density** | Low, lots of whitespace | Medium–high: dashboard + copy + art |
| **Brand** | Small mark + version/model line | Large mascot + tagline + path |
| **Guidance** | One **Try …** sample | Tips list + `/help` / `Esc` |
| **Session structure** | Almost no “turn” chrome at start | Explicit `turn N` + message region |
| **Status / environment** | Folded near the header | Dedicated two-line footer (“dash” feel) |
| **Theme** | Neutral dark by default | Switchable themes; **Dark Red** as a differentiator |
| **Sidebars / multi-pane** | Not emphasized on splash | Context/File/Help sidebars in code paths (after modes expand) |
| **Runtime live region** | Relatively unified transcript | Monitor / Live / compaction states with focused border |
| **Loading and tool feedback** | Often minimal status copy | Scramble label, dot animation, compaction bar, tool monitor, optional StatusBar |
| **Command completion** | Varies by product version (verify live) | `/` popup list + keyboard navigation |

---

## 5. Design direction (intent only—not a work order)

If the goal is **Claude-like concise modernism** while **keeping Amadeus dark red and the dedicated dashboard**, the following tensions are useful to reason about at the **experience** level (**no code changes implied**):

1. **Concision**: Claude’s “modern” feel comes from **few elements, one strong CTA, generous whitespace**. Amadeus’s dashboard is richer and can feel heavier on first paint—moving closer to Claude means **information architecture** choices on the dashboard (e.g. collapsing tips, raising the width threshold for the mascot), not only palette tweaks.
2. **Modern layout**: Claude combines **geometric mark + full-width rule + single-line command cue** into one top “card.” Amadeus already has a title row and rules; strengthening **one visual focal point** (e.g. welcome + one CTA, rest under `/help`) would align with that pattern.
3. **Dark red**: `Dark Red` already separates body, secondary, links, borders, and status—**concision is layout, red is palette**. **Low-saturation base + high-saturation accents** stays modern without harshness.
4. **Dashboard as differentiator**: This is a **clear Amadeus signature** (no Claude equivalent on splash). While keeping it, Claude can still inspire **only 3–5 top-priority facts** above the fold, with the rest in-session or via `/help`.
5. **Runtime motion**: For Claude-like **lightness**, review whether **Monitor + compaction** stacks too many tracks at once, and whether **scramble + dots + StatusBar + footer meter** duplicate the same story; motion can stay while **hierarchy** tightens.

---

## 6. tmux-cli text snapshots (excerpts)

Plain-text captures from the same machine; illustrations only—not full UI fidelity.

**Claude Code (splash excerpt)**:

```text
 ▐▛███▜▌   Claude Code v2.1.69
▝▜█████▛▘  <model> · <billing>
  ▘▘ ▝▝    ~/Dev/amadeus

  /model to try Opus 4.6

────────────────────────────────────────────────────────────────────────────────
❯ Try "edit app.rs to..."
────────────────────────────────────────────────────────────────────────────────
  ? for shortcuts
```

**Amadeus (dashboard + input/footer excerpt)**:

```text
 Amadeus v0.1.0 ───────────────────────────────────────────────────────────────

                                  <multi-line Braille mascot>

  ... (Tips: /help, Esc, etc.—may need scroll depending on pane height) ...

────────────────────────────────────────────────────────────────────────────────
❯ Try "how does src/main.rs work?"                    0 ch · 1 line
────────────────────────────────────────────────────────────────────────────────
  Type a message... (Enter: send, Alt+Enter: newline)

main  │ ◈ <model> [░░░░░░░░] 0% │ ◷ 00:03
root> │📂 ~/Dev/amadeus ⎇ dev │ ◫ no sandbox
```

**Runtime (illustrative)**: A hint row may show scramble + dots or `awaiting approval`; live block titles read ` Live ` or ` Monitor `; compaction shows one line such as `  Compacting context.  ·  45%  ·  3s`.

---

## 7. Cleanup

Per `skills/tui-tmux-debugging.md` after experiments:

```bash
tmux-cli kill --pane=remote-cli-session:<window>
# or
tmux-cli cleanup
```

This avoids leaving `remote-cli-session` busy or stray `amadeus` processes.

---

## 8. Maintaining this doc

- **Claude Code baseline**: `claude --version` was **2.1.69** at capture time (varies by install).
- **Amadeus version string**: Dashboard shows **v0.1.0** hard-coded (see `messages.rs`; align manually on release).
- **Comparison runbook**: [`.cursor/skills/tui-comparison-tmux/SKILL.md`](../.cursor/skills/tui-comparison-tmux/SKILL.md).
- Updating this documentation alone does not require application code changes.
