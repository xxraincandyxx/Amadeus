# fs-coding mini-suite

A lightweight benchmark that exercises the **Amadeus agent's native tool loop**
(read/edit/bash/grep + ReAct), as opposed to backbone-only benchmarks like
MBPP where the agent is a pass-through.

Each task is a tiny working directory with a buggy `solution.py` and a failing
`test_solution.py`. A fresh agent is given the paths and must read the files,
locate the bug, edit the implementation, and verify by running the tests.
Scoring is objective: `test_solution.py` exits 0 after the agent's turn.

## Why this exists

intercode/MBPP (`benchmarks/README.md`) measures the **backbone LLM** — the
agent emits one function and uses no tools. This suite measures the **agent**:
it must use tools and iterate against real test output. It is intentionally
*lightweight* (single-file tasks, no large Docker images, ~10 s each).

## Run

```bash
# Amadeus server with LLM tracing (auto-restart wrapper optional).
./target/debug/amadeus --server 3000 --llm-trace benchmarks/results/llm_trace &

# Full suite (15 tasks) or --smoke (first 2).
python benchmarks/fs_bench/run_fs_bench.py --timeout 150
```

Results land in `benchmarks/fs_bench/runs/<run_id>/` (`summary.json`,
`per_task.jsonl`). Every agent turn is captured in the server's
`llm_trace_*.jsonl` for diagnosis.

## Result (2026-07-23)

gemma-4-26b-a4b-it-fp8 via the OpenAI-compatible endpoint, default Amadeus
agent, 15 single-file Python bug-fix tasks:

| metric | value |
|---|---|
| **Solved** | **15 / 15 = 100%** |
| Median time / task | ~8 s |
| Tools used (observed via trace) | `read_file`, `edit_file`, `bash` |

### Caveat: this suite is at ceiling

100% means the suite **validates that the agent can run the tool loop**
(read → edit → run tests → iterate) but it does **not discriminate** between
model/agent quality — every task is a one-line fix in a ~5-line function, so a
competent agent solves all of them. It is the right instrument for confirming
"the agent's tools work end-to-end," and the wrong instrument for ranking
agents. For a discriminating signal, add harder variants (multi-file repos,
non-obvious bugs, refactors requiring several coordinated edits); the harness
supports this by adding entries to `tasks.py`.
