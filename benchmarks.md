# Benchmarks

Performance record for the Amadeus agent and its model backbones on external
benchmarks. Each entry states the model, the benchmark, the methodology, and
the measured result so numbers are reproducible and interpretable.

---

## MBPP (Python algorithmic) — intercode

| | |
|---|---|
| **Benchmark** | [princeton-nlp/intercode](https://github.com/princeton-nlp/intercode), Python/MBPP track (`data/python/mbpp/ic_mbpp.json`, 974 problems) |
| **Backbone model** | `gemma-4-26b-a4b-it-fp8-25603` — Gemma-4 26B, A4B (4-active-expert MoE), instruction-tuned, FP8 quantized |
| **Endpoint** | OpenAI-compatible API at `http://118.31.102.225:1113/v1` (self-hosted), via Amadeus's OpenAI client adapter |
| **Agent** | Amadeus ReAct agent (default tools), driven over the HTTP `/agents/:id/chat` endpoint; one fresh agent per problem |
| **Metric** | **Single-turn pass@1** (one attempt, no test-feedback retries) |
| **Scoring** | intercode's canonical `get_reward_mbpp` in the `intercode-python` Docker container (rpyc execution server); reward = fraction of hidden unit tests whose output matches the gold solution's output |
| **Prompt** | NL query **+ the canonical function signature** extracted from the gold's `def` line (standard MBPP setup; no test leakage) |
| **Tracing** | Amadeus server run with `--llm-trace`; every turn's full request/response recorded to `logs/llm_trace_*.jsonl` for diagnosis |
| **Date** | 2026-07-23 |

### Result

| metric | value |
|---|---|
| Problems evaluated | 973 / 974 |
| **Solved (pass@1) — headline** | **506 / 973 = 52.0%** |
| Solved, among clean attempts | 506 / 945 = 53.5% |
| Mean reward — all | 0.559 |
| Mean reward — clean attempts only | 0.576 |
| Problems lost to contained server-bounce windows | 28 (2.9%) |

**Bottom line:** gemma-4-26b solves **~52%** of MBPP single-turn through the
Amadeus agent, with a mean per-problem reward of ~0.56 (some partial credit
from problems passing a subset of tests).

### Methodology notes

- **Single-turn pass@1** is the standard, comparable MBPP number (one sample,
  no feedback). An earlier 3-turn interactive variant (agent sees test
  pass/fail and retries) scored higher in a 5-problem smoke test (~40%), but is
  non-standard and slower; it is not the headline number.
- **Canonical intercode scoring.** The same `intercode-python` Docker image and
  `get_reward_mbpp` reward function as `experiments/eval_n_turn.py`. Only the
  *policy* is swapped — intercode's GPT policy is replaced by the Amadeus agent.
- **Function signature is provided.** Without it the model guesses names and
  parameter arity, which (per trace-log diagnosis) was the dominant *genuine*
  failure mode. Providing the gold's `def name(params):` line is the standard
  MBPP prompt format and leaks no test information.

### Bugs found and fixed during the run

These materially affected the measured number and were located via the
per-turn LLM trace logs:

1. **Reward clobbering (the 0/N run).** `summarize_obs()` called `.get()` on
   rpyc netref dicts → `AttributeError`, caught by the turn loop's outer
   `except`, which forced `reward = 0` for *every* correct solution. The first
   full-split attempt scored 0/N because of this. Fix: capture the true env
   reward before any observation processing, and make `summarize_obs`
   netref-safe. Validation on a fixed sample: 0/10 → 6/10.
2. **Server streaming degradation.** Under sustained load the Amadeus server's
   streaming path intermittently returned `result expired` for every
   `/agents/:id/chat`. The harness self-heals: HTTP retry/backoff, per-problem
   `try/except`, and an auto-bounce that restarts the server listener on ≥5
   consecutive errors (an auto-restart wrapper re-spawns it). Cost: ~3% of
   problems fall in contained error windows and score 0, slightly depressing
   the headline number.

### Reproduce

```bash
# 1. Clone intercode + build the canonical python container (see benchmarks/README.md).
git clone --depth 1 https://github.com/princeton-nlp/intercode.git benchmarks/intercode
docker build -t intercode-python -f benchmarks/Dockerfile.intercode-python benchmarks/intercode
python3 -m venv benchmarks/.venv && benchmarks/.venv/bin/pip install -r benchmarks/requirements.txt

# 2. Amadeus server (auto-restarted) with LLM tracing.
bash -c 'while true; do ./target/debug/amadeus --server 3000 --llm-trace benchmarks/results/llm_trace; sleep 3; done' &

# 3. Run the full split.
benchmarks/.venv/bin/python benchmarks/run_amadeus.py --problems 974 --max_turns 1
```

Artifacts land in `benchmarks/results/<run_id>/` (`summary.json`,
`per_problem.jsonl`) and the per-turn trace in `benchmarks/results/llm_trace/`.
Full setup/diagnosis detail: `benchmarks/README.md`.

### Related commits (branch `feat/llm-trace`)

- `feat(agent): log full LLM request/response payloads per turn` — the Phase-A tracer
- `test(benchmarks): add intercode python/mbpp harness driven by the Amadeus agent`
- `fix(benchmarks): record true reward; add canonical signature prompt`
- `fix(benchmarks): make the intercode harness self-healing under load`

---

## fs-coding mini-suite — agent tool-loop check

| | |
|---|---|
| **Benchmark** | bespoke, `benchmarks/fs_bench/` — 15 single-file Python bug-fix tasks |
| **Backbone model** | `gemma-4-26b-a4b-it-fp8-25603` (same endpoint as above) |
| **Agent** | Amadeus default agent, one fresh agent per task, driven over HTTP `/agents/:id/chat` |
| **Metric** | tasks where `test_solution.py` exits 0 after the agent's turn |
| **What it exercises** | the agent's **native tools** (`read_file`, `edit_file`, `bash`, `grep`) and the ReAct read→edit→test→iterate loop — the surface MBPP skips |
| **Date** | 2026-07-23 |

### Result

| metric | value |
|---|---|
| **Solved** | **15 / 15 = 100%** |
| Median time / task | ~8 s |
| Tools observed (via trace) | `read_file`, `edit_file`, `bash` |

### Interpretation and caveat

This is the **complement** of the MBPP number: MBPP showed gemma's raw coding
(~52%); fs-coding shows the **Amadeus agent's tool loop works** (the agent
reads the failing test, edits the file, runs the test, iterates to green).

But 100% means the suite is **at ceiling** — each task is a one-line fix in a
~5-line function, so it validates the loop without discriminating between
agents. Treat it as a "smoke test that the agent's tools function end-to-end,"
not a ranking signal. Harder variants (multi-file, non-obvious bugs) can be
added in `tasks.py` when a discriminating agent benchmark is wanted.

Reproduce and details: `benchmarks/fs_bench/README.md`.
