# Amadeus TUI User Guide

> Complete guide to the Amadeus Terminal User Interface - a high-performance, themable terminal for AI agent orchestration

## Getting Started

### Prerequisites

1. Set your API keys in `.env`:
   ```bash
   PROVIDER=anthropic  # or "openai"
   ANTHROPIC_API_KEY=sk-ant-xxx
   OPENAI_API_KEY=sk-xxx
   ```

2. Run the TUI:
   ```bash
   cargo run --features full
   ```

   If you launch the TUI through `tmux-cli`, send single-key shortcuts without the
   default Enter suffix. For example:
   ```bash
   tmux-cli send "?" --pane=<pane> --enter=False
   ```

   For a detailed coding-agent debugging and acceptance workflow, see
   [TMUX_TEST_FLOW.md](./TMUX_TEST_FLOW.md).

### First Steps

1. Type a prompt and press `Enter` to send
2. Use `Up/Down` arrows to navigate through prompt history
3. Press `Ctrl+B` to toggle the file explorer
4. Press `Ctrl+T` to cycle through color themes
5. Press `q` to quit (when input is empty)

---

## Interface Overview

The interface consists of several functional areas:

```
┌─────────────────────────────────────────────────────────────────────┐
│  Live Viewport (streaming content / tool monitor / compaction)      │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  Chat Stream (conversation history with tool panels)                │
│                                                                     │
│  ❯ YOU: Write a function to parse JSON                              │
│  ❯ AMADEUS: I'll create a parser for you...                         │
│    ┌─ Tool: write_file ─────────────────────────┐                   │
│    │ ◐ Running...                               │                   │
│    │ ✓ Success (0.2s)                           │                   │
│    └────────────────────────────────────────────┘                   │
│                                                                     │
├─────────────────────────────────────────────────────────────────────┤
│  tok/s: 45.2  ▲ 1,234  ▼ 567  ⟡ thinking                            │
├─────────────────────────────────────────────────────────────────────┤
│  ▸ root  ~/project (main)  [docker]  claude-sonnet  ████░ 40%  05:23│
└─────────────────────────────────────────────────────────────────────┘
```

### Components

| Component | Location | Purpose |
|-----------|----------|---------|
| **Live Viewport** | Top | Dynamic area for streaming content, tool monitor, or compaction animation |
| **Chat Stream** | Center | Conversation history with markdown rendering |
| **Input Area** | Bottom center | Multi-line text input with character/line count |
| **Status Bar** | Above input | Token speed, input/output counts, thinking state |
| **Footer** | Bottom | Session info, git branch, sandbox status, model, context % |
| **File Sidebar** | Left (toggleable) | Project file tree |
| **Skills Sidebar** | Right (toggleable) | Prompt templates/skills |
| **Help Sidebar** | Right (toggleable) | Keyboard shortcuts reference |

---

## Keyboard Shortcuts

### Input Mode (Default)

| Shortcut | Action |
|----------|--------|
| `Enter` | Submit prompt |
| `Ctrl+Enter` | Insert newline (multi-line prompts) |
| `Esc` | Cancel stream or switch to Normal mode |
| `Up` / `Down` | Navigate prompt history |
| `Ctrl+B` / `Cmd+B` | Toggle file sidebar |
| `Alt+S` | Toggle skills sidebar |
| `Ctrl+Alt+B` | Run stream in background |
| `Ctrl+O` | Toggle tool output expansion |
| `Ctrl+T` | Switch to next theme |

**Cursor Navigation:**

| Shortcut | Action |
|----------|--------|
| `Ctrl+F` / `→` | Move cursor right |
| `Ctrl+B` / `←` | Move cursor left |
| `Ctrl+A` | Move to line start |
| `Ctrl+E` | Move to line end |
| `Alt+B` | Move back one word |
| `Alt+F` | Move forward one word |

**Editing:**

| Shortcut | Action |
|----------|--------|
| `Ctrl+K` | Delete to end of line |
| `Ctrl+U` | Delete to start of line |
| `Ctrl+D` | Delete next character |
| `Ctrl+H` / `Backspace` | Delete previous character |
| `Alt+D` | Delete next word |
| `Alt+Backspace` | Delete previous word |

**Scrolling:**

| Shortcut | Action |
|----------|--------|
| `Shift+Up/Down` | Scroll messages by 1 line |
| `PageUp/PageDown` | Scroll by page |
| `Ctrl+Home` / `Shift+Home` | Scroll to top |
| `Ctrl+End` / `Shift+End` | Scroll to bottom |

### Normal Mode

Press `Esc` from Input mode to enter Normal mode for navigation.

| Shortcut | Action |
|----------|--------|
| `q` | Quit |
| `Ctrl+D` | Quit |
| `i` | Switch to Input mode |
| `Ctrl+T` | Switch to next theme |
| `Ctrl+K` | Manual context compaction |
| `Esc` | Close sidebar, collapse tools |
| `Up/Down` | Navigate file sidebar |
| Any character | Restore input focus and type |

### Approval Mode

When a tool requires approval, a modal dialog appears:

| Shortcut | Action |
|----------|--------|
| `Up/Down` | Navigate options |
| `Enter` | Submit selected option |
| `Esc` | Cancel/Deny |
| `y` | Quick approve |
| `n` | Quick deny |
| `a` | Always approve (remember for future) |

### Tool Monitor Navigation

When viewing the tool monitor in the live viewport:

| Shortcut | Action |
|----------|--------|
| `Ctrl+X i` | Select previous tool |
| `Ctrl+X k` | Select next tool |
| `Ctrl+X j` | Exit parent (go back) |
| `Ctrl+X l` | Enter selected (drill into nested tools) |

### Multi-Session Navigation

| Shortcut | Action |
|----------|--------|
| `Tab` | Switch to next session |
| `Shift+Tab` | Switch to previous session |
| `Ctrl+]` | Switch to first direct sub-agent |
| `Ctrl+[` | Switch to parent session |
| `Ctrl+Backspace` | Close current sub-agent session |

### Vim Mode

Optional vim-style navigation (enabled via settings):

| Shortcut | Action |
|----------|--------|
| `j` | Scroll down |
| `k` | Scroll up |
| `g` | Go to top |
| `G` | Go to bottom |
| `Ctrl+D` | Page down |
| `Ctrl+U` | Page up |
| `i` | Enter input mode |
| `:` | Enter command mode |
| `q` | Quit |
| `Ctrl+C` | Force quit |

### Slash Commands

Type these in the input field:

| Command | Action |
|---------|--------|
| `/compact` or `/compress` | Manual context compaction |
| `/exit` | Quit |
| `/new-agent` | Spawn a new independent session with fresh agent |

---

## Color Themes

Cycle through themes with `Ctrl+T`. Available themes:

| Theme | Type |
|-------|------|
| Default Dark | Dark |
| Default Light | Light |
| Dracula | Dark |
| GitHub Dark | Dark |
| GitHub Light | Light |
| Solarized Dark | Dark |
| Solarized Light | Light |
| Atom One Dark | Dark |
| Ayu Dark | Dark |

---

## Tool Panels

Tools display inline in the chat stream with status indicators:

| Indicator | Meaning |
|-----------|---------|
| `◐ Running` | Tool is executing |
| `✓ Success` | Tool completed successfully |
| `✗ Error` | Tool failed |

**Features:**
- Shows exact shell commands being executed
- Intelligently truncates large outputs
- Collapsible with `Ctrl+O`
- Nested tool visualization for sub-agents
- Progress percentage and status messages

---

## Multi-Session Support

Amadeus supports two types of sessions:

1. **Independent Sessions** - Created with `/new-agent`. Each has its own fresh agent with empty history. Use for parallel, unrelated tasks.
2. **Sub-Agent Sessions** - Created when the supervisor delegates tasks to workers. Share parent context and are organized hierarchically.

### Session Indicators

The footer shows session information:
- **In MESH mode**: Displays `MESH` indicator
- **With named agents**: Shows agent name (e.g., `reviewer `)
- **Session breadcrumbs**: Hierarchical view of sub-agents (e.g., `root ▸ sub1 ▸ sub2`)

Session status markers:
| Marker | Meaning |
|--------|---------|
| `*` | Currently streaming |
| `?` | Pending approval |
| `!` | Last error |
| `>` | Active session |

### Background Tasks

- Press `Ctrl+Alt+B` to run a stream in the background
- Background indicator: `⏳ BG` in footer
- Continue working while task runs

---

## Context Management

### Context Window

The footer displays context usage as a percentage bar:
```
████░ 40%
```

**Recommendations:**
- Monitor the context bar during long conversations
- Compress when approaching 70%+ usage
- Use `/compact` or `Ctrl+K` to trigger compaction

### Compaction

Manual compaction (`Ctrl+K` or `/compact`):
- Summarizes conversation history
- Shows animated progress indicator
- Reports token savings after completion
- Non-blocking background operation

---

## Status Bar

Located above the input area:

| Indicator | Meaning |
|-----------|---------|
| `tok/s: 45.2` | Token generation speed |
| `▲ 1,234` | Input tokens consumed |
| `▼ 567` | Output tokens generated |
| `⟡ thinking` | Model is in extended thinking mode |

---

## Footer

The bottom status line displays:

```
▸ root  ~/project (main)  [docker]  claude-sonnet  ████░ 40%  05:23
```

| Section | Description |
|---------|-------------|
| Session breadcrumb | Current session hierarchy |
| Working directory | Current project path |
| Git branch | Active git branch |
| Sandbox status | `docker`, `seatbelt`, or none |
| Model | Active LLM model name |
| Context % | Context window usage with visual bar |
| Duration | Session time (MM:SS) |

---

## Configuration

### Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `PROVIDER` | `anthropic` | LLM provider (`anthropic` or `openai`) |
| `MODEL_ID` | varies | Model identifier |
| `MAX_OUTPUT_BYTES` | `50000` | Max tool output displayed |
| `SESSION_LOG_DIR` | none | Path to save conversation logs |
| `SESSION_LOG_COMPRESS` | `false` | Enable Gzip compression for logs |
| `AMADEUS_TOOL_MONITOR_LINES` | `16` | Height of tool monitor (min: 6) |
| `AMADEUS_TRY_PROMPT` | `how does src/main.rs work?` | Example text inside the `Try "…"` input hint (English) |
| `SANDBOX` | none | Sandbox type (`docker`, `podman`, `sandbox-exec`) |

### Session Logging

Enable automatic conversation logging:

```bash
# .env
SESSION_LOG_DIR=logs/sessions
SESSION_LOG_COMPRESS=true
```

Log format: Structured JSON (or `.json.gz`)

Contents:
- Full conversation history
- Tool inputs and outputs
- Timestamps for every turn

---

## Loading Phrases

During generation, rotating phrases appear:

**Witty examples:**
- "Reticulating splines..."
- "Trying to exit Vim..."
- "Consulting the digital spirits..."
- "Rewriting in Rust for no particular reason..."

**Informative tips:**
- "Use /compact to summarize context..."
- "Toggle Vim mode for a modal editing experience..."
- "Change CLI output format to JSON for scripting..."

---

## Mouse Support

- **Click scrollbar**: Jump to position
- **Scroll wheel**: Navigate messages (3 lines per scroll)

---

## Tips & Best Practices

### Efficient Navigation
1. Use `Ctrl+K` to compact context before it fills up
2. Press `Esc` to quickly exit sidebars or cancel streams
3. Navigate history with `Up/Down` arrows

### During Long Tasks
1. Press `Ctrl+Alt+B` to run in background
2. Use `Ctrl+O` to expand/collapse tool outputs
3. Navigate nested tools with `Ctrl+X` chords

### Multi-Agent Workflows
1. Sub-agents spawn automatically for delegated tasks
2. Switch sibling sessions with `Tab` and `Shift+Tab`
3. Jump between parent and child sessions with `Ctrl+[` and `Ctrl+]`
4. Close sub-sessions with `Ctrl+Backspace`

### Clean Exit
Always use `Ctrl+C` or `q` to quit - this ensures:
- Terminal raw mode is properly disabled
- Session logs are flushed to disk
- Background tasks are terminated cleanly

---

## Troubleshooting

### Common Issues

**TUI not displaying correctly:**
- Ensure terminal supports 256 colors
- Try setting `TERM=xterm-256color`

**Slow streaming:**
- Check network connection
- Verify API endpoint accessibility

**Context full:**
- Use `/compact` to summarize history
- Start a new session if needed

---

*Last updated: 2026-03-21*
