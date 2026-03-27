# tmux-cli TUI Debugging

> **Description:** Use this skill to safely test and debug interactive terminal user interfaces (TUIs) like Amadeus without getting trapped in UI loops or deadlocks, and without polluting the main project repository.

When working on a complex CLI or TUI project, running the application directly in your agent shell can cause terminal hangs, raw mode issues, or loss of output. By using `tmux-cli` from `claude-code-tools`, you can sandbox the execution in a remote, headless tmux session, send simulated user input, and capture the visual state of the application.

## Prerequisites

Ensure `tmux-cli` is installed in the environment:
```bash
if ! command -v tmux-cli &> /dev/null; then
    uv tool install claude-code-tools
fi
```

## The Debugging Workflow

### 1. Build and Isolate
Always build the project in the main directory but run it in a temporary workspace.
```bash
# 1. Build system-wise
cargo build --features full

# 2. Setup safe workspace
mkdir -p /tmp/amadeus_debug_env
cp .env /tmp/amadeus_debug_env/.env 2>/dev/null || cp .env.example /tmp/amadeus_debug_env/.env
cd /tmp/amadeus_debug_env
```

### 2. Launch the Headless Session
Start a remote tmux session and execute the binary inside it.
```bash
# Start a shell first to prevent the pane from exiting immediately if the app crashes
tmux-cli launch "bash"  # Returns a pane id, e.g., remote-cli-session:1

# Run the app
tmux-cli send "/Users/raincandy_u/Dev/amadeus/target/debug/amadeus" --pane=remote-cli-session:1
```

### 3. Interact and Inspect
Use standard `tmux-cli` commands to interact with the application.

*   **View the screen:** 
    ```bash
    tmux-cli capture --pane=remote-cli-session:1
    ```
*   **Send normal input:**
    ```bash
    tmux-cli send "Please write a test file" --pane=remote-cli-session:1
    ```
*   **Send special keys (e.g. Esc, Ctrl+C):**
    ```bash
    tmux-cli escape --pane=remote-cli-session:1
    tmux-cli interrupt --pane=remote-cli-session:1
    ```
*   **Wait for the UI to settle (avoid polling):**
    ```bash
    tmux-cli wait_idle --pane=remote-cli-session:1 --idle-time=2.0 --timeout=15
    ```

### 4. Cleanup
Always clean up your testing environment so subsequent tests start fresh.
```bash
tmux-cli kill --pane=remote-cli-session:1
rm -rf /tmp/amadeus_debug_env
```

## Amadeus-specific notes (inline viewport / multi-session)

- **Shared scrollback:** Amadeus uses ratatui `Viewport::Inline` and `insert_before`; switching agent sessions clears host scrollback and rebuilds the `Terminal`. In **tmux**, `ClearType::Purge` can briefly break cursor position queries (DSR); the app retries `Terminal::with_options` a few times after a flush + short sleep.
- **Blank `capture-pane`:** If the pane looks empty but `pgrep amadeus` shows a process, confirm the TTY matches: `lsof /dev/ttysXXX` for the tmux pane vs `lsof -p $(pgrep amadeus) | grep tty`. Stale PIDs on other TTYs are often a local IDE terminal, not the tmux pane.
- **Automation keys:** `tmux-cli` does not expose every chord; use `tmux send-keys -t <target> C-]` / `C-[` for session switching when needed.
- **Grepping captures:** Use `tmux capture-pane -p -e | strings` when the UI uses styling that strips poorly with plain `grep`.

## Useful TUI Debugging Scenarios

1.  **Testing Deadlocks/Hangs:** Send a task that requires long background processing or recursive tool calls. If the app hangs, use `tmux-cli capture` to read the UI state and `ps aux | grep amadeus` to check child processes.
2.  **Testing Policy & Approvals:** Send a command that triggers a block (e.g., writing to `.env`). Use `tmux-cli capture` to verify the approval dialog is drawn, and `tmux-cli send "y" --enter=False` to simulate dialog interaction.
3.  **Testing Layout Breakages:** Resize the tmux window or send unusually long inputs to see if the `ratatui` layout panics.
## Full Test Flow Guide

When testing a new build of the TUI, follow this progression from simple to complex to ensure all systems are functioning.

### Level 1: Basic Sanity Check
**Goal:** Ensure the TUI boots, handles basic text, and doesn't crash on simple inputs.
1. Launch the app in the tmux pane.
2. Send: `tmux-cli send "Hello, are you working?" --pane=remote-cli-session:1`
3. Wait 5 seconds, capture the pane. Ensure the agent replied with a standard greeting and the UI shows `[Idle]`.

### Level 2: Tool Execution & Policy Blocking
**Goal:** Verify file tools and the Ask/Strict policy engines work.
1. Send: `tmux-cli send "Please run the ls command" --pane=remote-cli-session:1`
2. Capture and verify the `bash` tool ran and returned directory contents.
3. Send: `tmux-cli send "Please write a dummy api key to the .env file" --pane=remote-cli-session:1`
4. Capture the pane. You should see the `Approval Required` dialog popup.
5. Send `n` (without enter) to deny the request. 
6. Capture and verify the agent gracefully handled the denial without crashing.

### Level 3: Complex Multi-Agent (Supervisor/Worker)
**Goal:** Verify sub-agent spawning, context switching, and stream stability.
1. Send: `tmux-cli send "Please create a Flappy Bird game in Python using pygame. Act as the supervisor, and delegate the task to a worker agent using the sub_agent tool. Tell it to write the code and run it to verify." --pane=remote-cli-session:1`
2. Wait and capture repeatedly (`tmux-cli wait_idle` or `sleep 10 && tmux-cli capture`).
3. Verify the UI updates the bottom status bar (e.g., `root* ▸ sub1*>`) to reflect the active sub-agent.
4. Verify that tool approvals bubble up correctly from the sub-agent to the UI, allowing you to approve (`y`) or deny (`n`) their actions.
5. Check if you can interrupt the process mid-flight using `/cancel` or `Ctrl+C`.
