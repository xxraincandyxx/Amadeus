# Benchmark Pipeline Implementation Plan

## Objective

Build a prompt-driven simulation and debug pipeline for Amadeus that behaves like a benchmark harness: repeatable, inspectable, and detailed enough to diagnose output-quality and orchestration regressions.

## Why this work is needed

The repository already has strong primitives for mocked clients, streaming events, stress tests, and session logs, but it does not yet have a true prompt-first evaluation pipeline.

Current gaps:

- The existing scenario DSL in `tests/scenarios/` is not prompt-centric and mixes user intent with model output semantics.
- There is no benchmark case schema for prompt datasets and expected outcomes.
- There is no batch runner that executes a suite and emits structured artifacts.
- There is no unified scoring/quality layer for regression detection.
- Existing tracing/session logging is useful but not assembled into a focused debug pipeline.

## Design principles

- Prompt first: benchmark cases must start from real user prompts, not synthetic output events.
- Deterministic by default: mocked runs should be stable enough for CI and regression checks.
- Fully inspectable: every run should preserve raw events, metrics, and final evaluation results.
- Modular: case definition, execution, metrics, scoring, and reporting should remain separate.
- Extensible: the same framework should support both mocked simulation and live provider runs.

## Scope of the first implementation

The first implementation should deliver a complete end-to-end pipeline for mocked benchmark runs, plus a thin live mode path where practical.

Included in scope:

1. Prompt-driven benchmark case schema.
2. Deterministic mock execution adapter.
3. Batch runner built on top of `Agent::run_stream()`.
4. Full event capture with timestamps.
5. Metrics aggregation.
6. Quality checks and scoring.
7. Structured artifact persistence.
8. Script/CLI entrypoint.
9. Seed benchmark fixtures and docs.

Out of scope for the first pass unless trivial:

- Rich dashboard UI.
- Distributed benchmark execution.
- External tracing backends.
- LLM-as-a-judge scoring.

## Proposed architecture

### 1. Benchmark case model

Introduce a benchmark module that defines prompt-driven case specs.

Each case should include:

- `id`: stable identifier.
- `suite`: logical grouping.
- `description`: human-readable purpose.
- `tags`: filtering and reporting.
- `prompt`: the user input to send to the agent.
- `mode`: `mock` or `live`.
- `config`: optional per-case overrides.
- `mock_script`: scripted model stream and optional tool expectations for deterministic runs.
- `expectations`: pass/fail rules.
- `thresholds`: latency and behavior constraints.

### 2. Execution adapters

Support two adapters behind one runner interface.

- `mock` adapter:
  - Uses a scripted `LLMClient`.
  - Replays predefined stream events and failures.
  - Enables deterministic benchmarking.

- `live` adapter:
  - Uses configured Anthropic/OpenAI client.
  - Runs real prompts against a provider.
  - Intended for manual validation, spot checks, and later scheduled runs.

The first pass should prioritize `mock` mode but keep the runner interface generic enough for `live` mode.

### 3. Event capture layer

Wrap `Agent::run_stream()` and capture:

- wall-clock start/end time,
- per-event timestamp,
- event ordinal,
- agent event payload,
- normalized transcript fragments,
- raw completion state.

This layer should preserve raw data rather than reducing too early.

### 4. Metrics aggregation layer

Derive benchmark/debug metrics from the event stream, including:

- total duration,
- time to first event,
- time to first text token,
- number of text deltas,
- number of thinking deltas,
- tool call count,
- tool durations where inferable,
- approval request count,
- approval denial count,
- compaction count,
- token usage totals,
- error count,
- final output length.

### 5. Quality evaluation layer

Support explicit rule-based checks such as:

- required substring,
- forbidden substring,
- required regex,
- maximum length,
- minimum length,
- required tool usage,
- forbidden tool usage,
- expected error presence/absence,
- expected approval presence/absence,
- expected compaction presence/absence.

Each check should yield:

- pass/fail,
- message,
- observed evidence.

### 6. Artifact persistence layer

For every case run, store:

- raw event trace JSON,
- normalized transcript text,
- metrics JSON,
- evaluation result JSON,
- a compact summary row for aggregation.

For each suite run, store:

- manifest of executed cases,
- aggregate summary JSON,
- optional JSONL rows for easy diffing and ingestion.

### 7. Runner entrypoint

Expose the pipeline through a simple executable path.

Possible options:

- a dedicated binary under `src/bin/`, or
- a shell script calling `cargo run --bin ...`.

The runner should support:

- suite filtering,
- case filtering,
- output directory selection,
- mock/live mode selection,
- fail-fast or continue-on-error behavior.

## File layout proposal

### New library module

Add a new module tree such as:

- `src/benchmark/mod.rs`
- `src/benchmark/case.rs`
- `src/benchmark/mock.rs`
- `src/benchmark/metrics.rs`
- `src/benchmark/eval.rs`
- `src/benchmark/report.rs`
- `src/benchmark/runner.rs`

### New binary entrypoint

- `src/bin/benchmark.rs`

### New fixtures/docs

- `tests/fixtures/benchmarks/*.json`
- `docs/BENCHMARK_PIPELINE.md`

### Optional script wrapper

- `scripts/run-benchmark.sh`

If `scripts/` does not exist, create it only if the wrapper script adds value beyond the binary.

## Case schema draft

Initial JSON-friendly shape:

```json
{
  "id": "tool-basic-bash",
  "suite": "core",
  "description": "Agent uses bash and summarizes the result cleanly",
  "tags": ["tools", "regression"],
  "prompt": "List the files in the current directory and tell me what you found.",
  "mode": "mock",
  "mock_script": {
    "steps": [
      {
        "events": [
          { "type": "tool_call_start", "id": "tool_1", "name": "bash" },
          { "type": "tool_call_delta", "arguments": "{\"command\":\"ls\"}" },
          { "type": "tool_call_done", "id": "tool_1" },
          { "type": "stop_reason", "reason": "tool_use" }
        ]
      },
      {
        "events": [
          { "type": "text_delta", "text": "I found Cargo.toml and src/." },
          { "type": "stop_reason", "reason": "end_turn" }
        ]
      }
    ]
  },
  "expectations": {
    "required_substrings": ["Cargo.toml"],
    "required_tools": ["bash"],
    "forbidden_substrings": ["I cannot access files"]
  },
  "thresholds": {
    "max_duration_ms": 5000,
    "max_tool_calls": 1
  }
}
```

## Implementation phases

### Phase 1: Plan and scaffolding

Deliverables:

- This implementation plan file.
- Module scaffolding for the benchmark pipeline.
- Initial case and result data structures.

### Phase 2: Prompt-first case model

Deliverables:

- Replace reliance on `tests/scenarios/` for benchmark purposes.
- Define a prompt-first schema and parsing path.
- Create a fixture loader for benchmark case files.

Notes:

- Keep the old scenario system intact unless a small compatibility bridge is useful.
- Do not overload the current `ScenarioBuilder` abstraction with benchmark semantics.

### Phase 3: Deterministic execution runner

Deliverables:

- Batch runner that loads one or more benchmark cases.
- Mock client adapter backed by scripted stream events.
- Full event capture with timestamps and final result collation.

Notes:

- Use `Agent::run(prompt)` or `Agent::run_stream()` correctly by pushing the prompt through the normal path.
- Avoid re-implementing the agent loop.

### Phase 4: Metrics and debug instrumentation

Deliverables:

- Metrics reducer that computes timing, token, tool, approval, compaction, and error stats.
- Transcript reconstruction helper.
- Raw trace serialization.

Notes:

- Prefer deriving metrics from emitted events, not hidden agent internals.

### Phase 5: Quality evaluation

Deliverables:

- Rule evaluation engine.
- Per-check and aggregate score reporting.
- Failure messages with concrete observed evidence.

Notes:

- The first version should be deterministic and interpretable.
- Scoring should be transparent enough to debug without another model.

### Phase 6: Reporting and artifact layout

Deliverables:

- Case-level artifact directories.
- Suite-level summary files.
- JSON and text outputs optimized for local debugging and CI diffs.

Suggested run directory shape:

```text
benchmark_runs/
  20260307_123456/
    manifest.json
    summary.json
    results.jsonl
    cases/
      tool-basic-bash/
        trace.json
        transcript.txt
        metrics.json
        evaluation.json
```

### Phase 7: Runner UX

Deliverables:

- CLI/binary entrypoint.
- Optional script wrapper.
- Filter flags and output path configuration.

Example UX target:

```bash
cargo run --bin benchmark -- --suite core
cargo run --bin benchmark -- --case tool-basic-bash
cargo run --bin benchmark -- --suite core --output benchmark_runs/local
```

### Phase 8: Seed benchmark suites

Initial suites to include:

- `core`: direct answers, single-tool flows, summary quality.
- `streaming`: chunked output and long response monitoring.
- `approval`: approval-required and denial paths.
- `recovery`: flaky/error scenarios.
- `compaction`: long-context behavior where feasible with mocks.

## Detailed implementation tasks

1. Create the benchmark module and re-export it from `src/lib.rs`.
2. Define serializable benchmark case structures.
3. Define serializable run result, metrics, evaluation, and artifact manifest structures.
4. Implement fixture loading from `tests/fixtures/benchmarks/`.
5. Implement a scripted mock client adapter suitable for benchmark cases.
6. Implement the event capture runner on top of `Agent::run_stream()`.
7. Implement transcript reconstruction and summary extraction.
8. Implement metrics aggregation from `AgentEvent` values.
9. Implement rule-based quality evaluation.
10. Implement report persistence helpers.
11. Add a binary entrypoint for local execution.
12. Add seed fixtures covering at least 3-5 meaningful flows.
13. Add documentation for authoring and running benchmark cases.
14. Run targeted checks and fix compile/test issues.

## Validation plan

The implementation should be validated in layers.

### Compile validation

- `cargo check --features full`

### Targeted benchmark validation

- Run the benchmark binary against a small mock suite.
- Verify artifacts are written as expected.
- Inspect one successful and one failing case manually.

### Regression validation

- Confirm the new code does not break existing tests.
- Run targeted relevant tests if benchmark code reuses test utilities.

## Risks and mitigations

### Risk: current scenario infrastructure leaks into the new design

Mitigation:

- Build the benchmark pipeline in a separate module with separate semantics.

### Risk: event-derived metrics miss important timing/context

Mitigation:

- Timestamp every captured event externally in the runner.

### Risk: live mode introduces nondeterminism too early

Mitigation:

- Ship mock mode first and keep live mode opt-in.

### Risk: report format becomes too ad hoc

Mitigation:

- Define result structs first, then serialize them consistently.

## Success criteria

The first implementation is successful if all of the following are true:

- A user can define prompt-driven benchmark cases in fixtures.
- A batch runner can execute those cases deterministically.
- Every run emits raw trace, transcript, metrics, and evaluation artifacts.
- Failures are understandable from the stored artifacts alone.
- The pipeline is useful both for regression checks and deep debugging.

## Immediate next step

Proceed by implementing the benchmark module scaffolding, fixture schema, and the first working mocked runner path before adding richer scoring and documentation.
