# Benchmark Pipeline

Amadeus now includes a prompt-driven benchmark pipeline for simulation, debugging, and regression tracking.

## What it does

- Loads prompt-first benchmark cases from `tests/fixtures/benchmarks/`
- Executes them through the normal agent loop
- Captures every emitted `AgentEvent` with timestamps
- Aggregates metrics and evaluates rule-based quality checks
- Writes per-run artifacts to `benchmark_runs/`

## Run it

```bash
cargo run --bin benchmark -- --suite core
```

Useful filters:

```bash
cargo run --bin benchmark -- --case core_tool_bash
cargo run --bin benchmark -- --suite approval --output benchmark_runs/local
```

## Fixture shape

Each benchmark case is a JSON file with:

- `id`
- `suite`
- `description`
- `prompt`
- `mode`
- `mock_script` for deterministic mocked runs
- `expectations`
- `thresholds`

## Artifacts

Each run writes a directory like:

```text
benchmark_runs/<run_id>/
  summary.json
  results.jsonl
  cases/<case_id>/
    trace.json
    metrics.json
    evaluation.json
    result.json
    transcript.txt
```

These artifacts are designed for local debugging first and CI integration later.
