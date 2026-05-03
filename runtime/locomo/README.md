# LoCoMo Memory Agent Experiment

Tests whether the MemoryAgent improves conversational recall on the
[LoCoMo](https://arxiv.org/abs/2406.08351) benchmark.

## Background

LoCoMo tests whether a model can answer questions about a long multi-session
conversation. The standard approach feeds the entire conversation (up to 108K
chars, 20+ sessions) as a single prompt and asks a question.

The MemoryAgent takes a different approach:
1. Feeds the conversation **session by session** across multiple turns
2. The LLM uses the `memory` tool to store important facts as it reads
3. After all sessions are processed, asks the questions
4. The LLM recalls stored memories to answer

This is more realistic — real conversations happen over time, and the memory
system should help retain facts that would otherwise be lost.

## Quick Start

```bash
# 1. Start the Amadeus server
cargo run --features full -- --server 3000

# 2. Run baseline (measures raw model performance on LoCoMo)
./run_baseline.sh

# 3. Run MemoryAgent experiment
python memory_agent_runner.py --server http://localhost:3000 --num-items 10

# 4. Compare results
python compare_results.py
```

## Files

| File | Purpose |
|------|---------|
| `run_baseline.sh` | Runs mini_locomo via openbench-quick (baseline) |
| `memory_agent_runner.py` | MemoryAgent-based LoCoMo evaluation |
| `compare_results.py` | Compares baseline vs MemoryAgent scores |

## Metrics

The key metric is **LoCoMo Core Accuracy** (Single-Hop, Multi-Hop, Open Domain,
Temporal). Adversarial questions (category 5) are tracked separately since they
require the model to recognize when information is NOT present.

A successful MemoryAgent should:
- Match or exceed baseline accuracy on core categories
- Show the memory tool being used effectively
- Not degrade on general knowledge outside the conversation
