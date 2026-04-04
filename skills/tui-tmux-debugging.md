---
name: tui-tmux-debugging
description: Safely test and debug TUIs without terminal hangs using tmux-cli
allowed_tools:
  - bash
  - glob
  - grep
---

## LOOP FOREVER

1. Build: `cargo build --features full`
2. Launch: `tmux-cli launch "bash"` → saves pane id
3. Send: `tmux-cli send "<binary>" --pane=<pane>`
4. Capture: `tmux-cli capture --pane=<pane>`
5. Diagnose: inspect UI state, check `ps aux | grep amadeus`
6. Fix: send commands, `tmux-cli interrupt`, or `tmux-cli escape` as needed
7. Verify: `tmux-cli capture` after fix
8. Iterate: go to 4
9. Cleanup: `tmux-cli kill --pane=<pane> && rm -rf /tmp/amadeus_debug_env`

## Reference

**Prerequisites:**
```bash
if ! command -v tmux-cli &> /dev/null; then
    uv tool install claude-code-tools
fi
```

**Key commands:**
- `tmux-cli capture --pane=<pane>` — view screen
- `tmux-cli send "<input>" --pane=<pane>` — send text
- `tmux-cli wait_idle --pane=<pane> --idle-time=2.0 --timeout=15` — wait for UI
- `tmux-cli interrupt --pane=<pane>` — send Ctrl+C
- `tmux-cli escape --pane=<pane>` — send Esc

**Amadeus notes:**
- Blank capture + running process → check TTY: `lsof /dev/ttysXXX` vs `lsof -p $(pgrep amadeus) | grep tty`
- Shared scrollback: `ClearType::Purge` briefly breaks cursor DSR; app retries after flush
- `tmux-cli` misses some chords → fallback: `tmux send-keys -t <target> C-]` / `C-[`

**Common fixes:**
- Stuck in loop → `interrupt` then send `""`
- Session frozen → `escape`, exit nested shells
- Layout panic → resize tmux window, capture again
