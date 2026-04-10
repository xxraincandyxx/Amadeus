# Cite And Paste Plan

## Goal

Add a composer cite flow that behaves like Claude Code/Codex style file mentions:

- user types `@`
- composer offers workspace file suggestions
- accepting a suggestion inserts markdown link text into the actual prompt
- the input box renders the citation as a compact visible token instead of raw markdown

Add paste support alongside it:

- handle terminal paste events explicitly
- paste plain text safely into the composer
- convert single pasted filesystem paths into citation links when appropriate

## Boundary

Per `docs/AGENT_WORKFLOW_CHECKLIST.md`, the parsing, formatting, and path normalization logic belongs in core. The TUI should only:

- collect terminal events
- render cite chips and suggestion popups
- forward input mutations through the shared core helpers

## Scope

This implementation will ship:

1. core cite-query parsing
2. workspace citation candidate discovery
3. core citation insertion and markdown-link formatting
4. core pasted-path normalization
5. TUI `@` suggestion popup
6. TUI rendered cite-chip effect in the composer
7. bracketed paste event handling
8. tests for core logic and focused TUI composer behavior

This implementation will not yet ship:

- fuzzy symbol-aware mention lookup
- non-file mention types
- external clipboard API integration beyond terminal paste events
- persistent mention metadata in session history

## Core Design

Add a new core composer helper module that exposes deterministic functions for:

- scanning workspace file candidates
- finding the active `@query` at the cursor
- filtering candidates for the active query
- replacing the active `@query` with markdown link text
- parsing composer markdown links back into renderable citation spans
- normalizing pasted single-path payloads

The markdown written into the real prompt remains:

```md
[file-name.ext](/absolute/path/to/file-name.ext)
```

The rendered composer view will show a compact cite token while preserving raw prompt fidelity.

## TUI Design

### 1. Suggestion flow

- When the cursor is inside an active `@query`, show a popup below the composer.
- Candidate rows show:
  - visible token text such as `@reviewer.md`
  - relative path as the description
- `Tab` accepts the selected cite
- `Shift+Tab` and `Ctrl+Down` keep working for suggestion navigation

### 2. Rendered composer effect

- Keep the raw markdown link in the underlying composer state.
- Render an overlay paragraph over the textarea that replaces markdown citations with compact visible cite tokens.
- Pad rendered cite spans so visible width matches the raw markdown width, preserving cursor alignment against the hidden textarea.

### 3. Paste behavior

- Enable `crossterm` bracketed paste event support.
- Route `Event::Paste(String)` through the app event loop.
- In the composer:
  - plain pasted text inserts as-is
  - a single pasted path or `file://` URL that resolves to a file becomes a citation markdown link

## File Plan

### Core

- `crates/core/src/commands/composer.rs`
  - new cite and paste helper module

- `crates/core/src/commands/mod.rs`
  - export the new composer helpers

- `crates/core/src/lib.rs`
  - re-export the public composer helpers needed by the TUI

### TUI

- `crates/tui/src/ui/event.rs`
  - add paste event support

- `crates/tui/src/ui/components/input.rs`
  - cache citation candidates
  - compute active cite query
  - render cite-token overlay
  - apply selected cite insertion
  - handle pasted text/path insertion

- `crates/tui/src/ui/app.rs`
  - route paste events into the input component
  - keep input-mode behavior consistent with current submit/history shortcuts

- `crates/tui/Cargo.toml`
  - enable `crossterm` bracketed paste

## Test Plan

### Core tests

- active cite query is found at the cursor
- citation candidates filter by filename and relative path
- citation insertion replaces only the active `@query`
- markdown citation parsing finds renderable cite spans
- pasted `file://` URLs and quoted paths normalize correctly

### TUI tests

- typing `@rev` opens cite suggestions
- applying a cite inserts markdown while visible input renders compact token text
- bracketed paste inserts plain multiline text
- pasting a single path inserts a citation markdown link

## Success Criteria

The feature is complete when:

- `@` file cite suggestions work in the TUI
- accepted cites are stored as markdown links in the real prompt
- the composer renders cite tokens instead of raw markdown links
- paste events are handled explicitly
- path paste can become a cite link
- `cargo check --features full` and targeted composer tests pass
