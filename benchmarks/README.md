# Benchmarks

External benchmark harnesses that exercise the Amadeus agent against
third-party evaluation suites. Each subdirectory names the benchmark it
targets.

## intercode (princeton-nlp/intercode) — Python algorithmic track

Tests the Amadeus agent on the **MBPP** coding benchmark using intercode's
**canonical** evaluation: a Docker container (`intercode-python`) runs an rpyc
Python execution server, and `intercode.envs.PythonEnv` scores each submission
with `get_reward_mbpp` (fraction of hidden unit tests matching the gold's
outputs). The Amadeus agent replaces intercode's GPT policy, driven over the
HTTP `/chat` endpoint.

### Why this shape

- **Execution is canonical intercode.** The same docker image, the same rpyc
  server (`docker/utils/python_server.py`), the same reward function as
  `experiments/eval_n_turn.py`. Only the *policy* is swapped for Amadeus.
- **Every turn is traced.** Run the Amadeus server with `--llm-trace` and each
  provider call is recorded to `logs/llm_trace_*.jsonl` (full request payload +
  assembled response), so a failed problem can be diagnosed by reading exactly
  what the model saw and said.

### Setup

```bash
# 1. Clone intercode (gitignored — not committed to this repo).
git clone --depth 1 https://github.com/princeton-nlp/intercode.git benchmarks/intercode

# 2. Build the canonical python container with the corrected Dockerfile.
#    (Upstream docker/python.Dockerfile has an invalid `COPY ../` that escapes
#    the build context; benchmarks/Dockerfile.intercode-python fixes it.)
docker build -t intercode-python \
    -f benchmarks/Dockerfile.intercode-python benchmarks/intercode

# 3. Python deps for the harness.
python3 -m venv benchmarks/.venv
benchmarks/.venv/bin/pip install -r benchmarks/requirements.txt
```

If Docker Hub is unreachable directly, export a proxy before building/pulling:
`HTTP_PROXY=… HTTPS_PROXY=… docker build …`.

### Run

```bash
# Terminal 1 — Amadeus server with LLM tracing (uses .amadeus/settings.json).
cargo run --features full -- --server 3000 --llm-trace benchmarks/results/llm_trace

# Terminal 2 — the harness.
benchmarks/.venv/bin/python benchmarks/run_amadeus.py \
    --problems 5 --start 0 --max_turns 3 --amadeus_url http://127.0.0.1:3000
```

Results are written to `benchmarks/results/<run_id>/`:
- `summary.json` — aggregate (mean reward, solved count).
- `per_problem.jsonl` — per-problem query, reward, and full action/observation history.

### Diagnosing failures

The matching `benchmarks/results/llm_trace/llm_trace_*.jsonl` captures every
`/chat` turn as two JSONL records (`kind: "request"` with system + messages +
tools, and `kind: "response"` with the model's text + tool calls + token usage).
To see why a problem failed, cross-reference the action in `per_problem.jsonl`
against the request/response pair of the same turn in the trace.

### Smoke-test result (2026-07-22)

gemma-4-26b-a4b-it endpoint, 5 MBPP problems, max 3 turns each:

| run | solved | mean reward | notes |
|-----|--------|-------------|-------|
| 1   | 2/5    | 0.40        | — |
| 2   | 0/5    | 0.00        | model produced alternative (incorrect) algorithms; harness verified correct (gold scores 1.0/1.0 on all 5) |

The across-run variance reflects the model's coding ceiling on these problems
plus sampling noise, not a scoring defect — feeding the gold solution through
the same `def` + `submit` path yields reward 1.0 on every problem. The first
diagnosis pass also caught and fixed a real harness bug (rpyc netref objects
were not JSON-serializable when fed back as observations).
