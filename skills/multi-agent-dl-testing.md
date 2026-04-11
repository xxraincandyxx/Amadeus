# Multi-Agent DL Math Testing Skill

> **Description:** Use this skill to test multi-agent collaboration in Amadeus by orchestrating 4 specialized workers (Architect, Programmer, Tester, Reviewer) to build a deep learning project that trains transformers on math operations.

## Prerequisites

- `tmux-cli` installed (via `uv tool install claude-code-tools`)
- `tmux` installed
- Amadeus built with `cargo build --features full`
- API key configured in `.env`

## The Testing Workflow

### Phase 1: Setup Isolated Environment

```bash
# 1. Create clean workspace
mkdir -p /tmp/amadeus_dl_project
cp .env /tmp/amadeus_dl_project/.env 2>/dev/null

# 2. Build amadeus
cargo build --features full

# 3. Launch tmux session
tmux-cli launch "bash"
```

### Phase 2: Launch Amadeus and Setup Workers

Start amadeus in the tmux pane:
```bash
tmux-cli send "/path/to/amadeus --record /tmp/amadeus_dl_project/logs" --pane=remote-cli-session:1
```

Send the initial multi-agent prompt:
```
I want to test multi-agent collaboration. Please spawn 4 sub-agents using the sub_agnet tool:
1. Architect Engineer - with capabilities: design, architecture, planning
2. Programmer - with capabilities: code, python, implementation
3. Tester - with capabilities: testing, evaluation, analysis
4. Reviewer - with capabilities: review, optimization, quality

Each sub-agent should be spawned with a specific role prompt that defines their responsibilities.

The goal is to build a deep learning project that trains transformer models on math operations (+, -, *, /, mod).

Project requirements:
- Model size: 5M-10M parameters (CRITICAL: verify before training)
- RAM threshold: Batch size must keep RAM < 1GB
- Use uv for Python environment management
- Create beautiful visualizations of training metrics
- Keep best 5 models with experimental logs

Start by having the Architect design the project in /tmp/amadeus_dl_project/SPEC.md
```

### Phase 3: Monitor and Debug

**Capture UI state:**
```bash
tmux-cli capture --pane=remote-cli-session:1
```

**Wait for idle:**
```bash
tmux-cli wait_idle --pane=remote-cli-session:1 --idle-time=3.0 --timeout=30
```

**Send interrupt if stuck:**
```bash
tmux-cli interrupt --pane=remote-cli-session:1
```

### Phase 4: Guide the Test Flow

The test should follow this progression:

1. **Design Phase** (Architect + Reviewer)
   - Architect creates SPEC.md with architecture design
   - Reviewer suggests optimizations

2. **Implementation Phase** (Programmer + Tester)
   - Programmer implements based on spec
   - Tester writes tests BEFORE implementation (TDD)
   - Programmer fixes bugs found by tests

3. **Training Phase** (Tester + Reviewer)
   - Tester runs training experiments
   - Reviewer analyzes results
   - Iterate to find best architecture

4. **Verification Phase**
   - Verify model size is 5M-10M parameters
   - Run comprehensive tests
   - Generate visualizations
   - Save best 5 models

### Phase 5: Issue Commands to Fix Bugs

If bugs are encountered:

**Missing dependencies:**
```
Please use uv to install dependencies: cd /tmp/amadeus_dl_project && uv venv && uv pip install -r requirements.txt
```

**Model size too large:**
```
The model has too many parameters. Please reduce:
- d_model from 512 to 256
- num_layers from 4 to 2
- Verify parameters stay under 10M
```

**Training bugs:**
```
Please fix the bug and re-run the training test.
```

### Phase 6: Capture Final Results

When complete, capture the final state:
```bash
tmux-cli capture --pane=remote-cli-session:1
find /tmp/amadeus_dl_project -type f \( -name "*.py" -o -name "*.md" -o -name "*.txt" -o -name "*.png" -o -name "*.jpg" \) > /tmp/file_list.txt
```

## Expected Deliverables

### Code Artifacts
- `project/src/model.py` - Transformer model
- `project/src/train.py` - Training loop
- `project/src/data.py` - Data generators
- `project/src/viz.py` - Visualization utilities
- `project/requirements.txt` - Dependencies
- `project/SPEC.md` - Architecture specification

### Verification Checklist
- [ ] Model parameters: 5M-10M (verify with `python -c "import torch; model = ...; print(sum(p.numel() for p in model.parameters()))"`)
- [ ] RAM < 1GB per batch
- [ ] Training runs without errors
- [ ] Test accuracy > 90% on held-out data
- [ ] Visualizations generated
- [ ] Best 5 models saved

## Cleanup

```bash
tmux-cli kill --pane=remote-cli-session:1
rm -rf /tmp/amadeus_dl_project
```

## Troubleshooting

**Agent stuck in loop:**
- Send interrupt: `tmux-cli interrupt --pane=remote-cli-session:1`
- Then send: `""` with enter to get back to prompt

**Session frozen:**
- Try escape: `tmux-cli escape --pane=remote-cli-session:1`
- Then try to exit any nested bash shells

**Files not being created:**
- Check if agent has correct workdir
- Send explicit file creation commands

## Full Test Flow (copy-paste for automation)

```bash
# Setup
mkdir -p /tmp/amadeus_dl_project
cp .env /tmp/amadeus_dl_project/.env
cargo build --features full
tmux-cli launch "bash"

# Start amadeus
tmux-cli send "/path/to/amadeus" --pane=remote-cli-session:1
sleep 5

# Send initial prompt (modify path as needed)
tmux-cli send "YOUR_INITIAL_PROMPT_HERE" --pane=remote-cli-session:1

# Monitor loop
while true; do
  tmux-cli wait_idle --pane=remote-cli-session:1 --idle-time=5.0 --timeout=60
  tmux-cli capture --pane=remote-cli-session:1
  echo "---"
  sleep 10
done
```
