# Golden End-to-End Workflow

## Goal

The golden workflow gives Amadeus one stable acceptance story: diagnose a calculator regression, make a minimal fix, verify it, and report the result. A predesigned prompt controls behavior and a preconfigured replay LLM makes the core run deterministic, offline, and suitable for CI.

The workflow is intentionally layered. Core behavior runs on every change, while transport, UI, retrieval, persistence, and real-provider checks reuse the same story at their appropriate boundaries. This keeps a failure attributable to one subsystem.

## Scenario Contract

User prompt:

> A calculator regression is failing. Diagnose the root cause, make the smallest safe fix, run the focused verification, and return a structured incident report.

System prompt profile:

> Follow this exact sequence: inspect evidence, state the root cause, make the smallest change, run focused verification, then report root cause, change, verification, and risk. Do not claim success without tool evidence.

Preconfigured LLM:

| Setting | Value |
|---|---|
| CI model | `amadeus-golden-replay-v1` via `ScenarioMockClient` |
| Fixture | `tests/fixtures/scenarios/golden_incident_workflow.json` |
| Provider smoke model | `MODEL_ID` from the normal runtime configuration |
| Temperature | Provider default until the client trait exposes sampling controls |
| Output budget | 8,000 tokens |

The replay emits thinking and text deltas, three tool calls, token usage, and a final answer. The tools must run in this order:

1. `read_file` inspects `calculator.rs`.
2. `edit_file` changes `a - b` to `a + b`.
3. `bash` compiles and runs the focused Rust test.

The final report must contain root cause, change, verification evidence, and risk.

## Acceptance Layers

| Layer | Features exercised | Expected evidence |
|---|---|---|
| Core CI | prompt profile, configured model, streaming, thinking, tool schemas, file read/edit, bash, token usage, history | `cargo test --test golden_workflow_test --features full` passes and the artifact is corrected |
| Safety CI | permission allow/ask/deny, approval round trip, path policy, blocked commands, hooks | Existing approval, permission, hook, bash, and file tests pass; denied operations leave the workspace unchanged |
| Context CI | memory store/load, project context, compaction, transcript/session restore | Existing context, compaction, messages, and session tests preserve the incident objective and verification result |
| Knowledge CI | skill discovery and RAG ingest/query/delete | API/component tests retrieve the calculator runbook and expose the selected skill in the prompt/tool context |
| Orchestra CI | capability routing, peer calls, sub-agents, locks, artifacts, telemetry | Existing product-flow, P2P, sub-agent, lock, and telemetry tests pass with correlated task events |
| API smoke | health, config, prompts, tools, sessions, streaming, approvals, tasks, memory, RAG | Routes return successful contracts and streamed events preserve core ordering |
| TUI smoke | input, streaming transcript, tool groups, approvals, sessions, compaction, agent panel | Headless replay renders the final report and stable chrome at desktop and narrow sizes |
| Provider smoke | Anthropic and OpenAI adapters, real streaming and tool-call encoding | The same prompt completes once per configured provider in an opt-in, credentialed job |
| Recovery smoke | stream failure, tool failure, timeout, denial, cancellation | Failure is visible, state is consistent, retry resumes without duplicating the edit |

## Execution

Run the complex single-sample benchmark. This is the primary feature-tour workflow and persists a trace, metrics, evaluation details, result JSON, and transcript:

```bash
cargo run --features full --bin benchmark -- \
  --suite single-sample \
  --mode mock \
  --output benchmark_runs/single_sample
```

The fixture is `tests/fixtures/benchmarks/single_sample_full_workflow.json`. It seeds six historical messages, forces repeated extract-based compaction, performs streamed discovery with `glob` and `grep`, tracks work through `todo`, runs the focused compaction suite through `bash`, probes an approval-gated workspace write, recovers from denial, emits token usage, and receives a scored structured report.

Run the deterministic golden path:

```bash
cargo test --test golden_workflow_test --features full
```

Run all automated feature coverage:

```bash
cargo check --features full
cargo test --features full
```

Run the repository quality gate before release:

```bash
./verify.sh
```

For API and TUI smoke testing, start Amadeus with normal runtime configuration and replay the scenario contract above through each surface. Provider smoke runs must be opt-in because they require credentials, cost money, and are not deterministic.

## Pass Criteria

The workflow passes only when the file contains `a + b`, the focused test exits successfully, tool completion order is stable, the configured prompt reaches every LLM request, token usage is emitted, the final report cites verification, no scripted LLM steps remain, and all boundary-layer suites pass.

The single-sample benchmark additionally requires at least one compaction event, an approval event, all four required tool types, no terminal errors, bounded duration and tool calls, structured report sections, and absence of the denied write artifact. Repeated compaction is valid and expected because tool results continue to grow the deliberately tiny context window.

“All features” means every supported subsystem is covered by at least one layer. It does not mean forcing every subsystem through one process: RAG requires an embedding service, TUI needs rendering assertions, API approval is asynchronous, and real providers require credentials. Those boundaries are tested separately while sharing the same scenario contract.
