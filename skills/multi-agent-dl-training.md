---
name: multi-agent-dl-training
description: Orchestrate 4 workers (Architect, Programmer, Tester, Reviewer) to build a DL project
allowed_tools:
  - bash
  - glob
  - grep
  - read_file
  - write_file
---

## LOOP FOREVER

1. Spawn workers via `sub_agent` tool: Architect, Programmer, Tester, Reviewer
2. Delegate: Architect designs → Programmer implements → Tester writes tests → Reviewer analyzes
3. Monitor: `tmux-cli capture`, `tmux-cli wait_idle`
4. Fix: send commands to workers to resolve bugs
5. Iterate: go to 2
6. Collect: verify model size, accuracy, generate viz, save best models
7. Cleanup: `tmux-cli kill --pane=<pane> && rm -rf /tmp/amadeus_dl_project`

## Reference

**Setup:**
```bash
mkdir -p /tmp/amadeus_dl_project
mkdir -p /tmp/amadeus_dl_project/.amadeus
cp .amadeus/settings.json /tmp/amadeus_dl_project/.amadeus/settings.json 2>/dev/null
cargo build --features full
tmux-cli launch "bash"
tmux-cli send "/path/to/amadeus --record /tmp/amadeus_dl_project/logs" --pane=remote-cli-session:1
```

**Initial prompt template:**
```
Spawn 4 sub-agents:
1. Architect — design, architecture, planning
2. Programmer — code, python, implementation
3. Tester — testing, evaluation, analysis
4. Reviewer — review, optimization, quality

Goal: build a DL project training transformers on math ops (+, -, *, /, mod).
Requirements:
- Model: 5M-10M params (verify before training)
- RAM: batch size < 1GB
- Use uv for env management
- Viz training metrics
- Keep best 5 models

Start: Architect → /tmp/amadeus_dl_project/SPEC.md
```

**tmux-cli:**
- `tmux-cli capture --pane=remote-cli-session:1`
- `tmux-cli wait_idle --pane=remote-cli-session:1 --idle-time=3.0 --timeout=30`
- `tmux-cli interrupt --pane=remote-cli-session:1`

**Bug fixes:**
- Missing deps → `cd /tmp/amadeus_dl_project && uv venv && uv pip install -r requirements.txt`
- Model too large → reduce d_model, num_layers, verify with `sum(p.numel() for p in model.parameters())`
- Agent stuck → `interrupt` then `""`
