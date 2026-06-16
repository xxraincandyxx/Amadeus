# Runtime

Real-world tests, experiments, and mini software built on the Amadeus project.

This directory is for practical validation: benchmarks, integration tests against
live LLMs, proof-of-concept tools, and experiments that measure and improve the
behavior of Amadeus agents in realistic scenarios.

## Structure

Each subdirectory is a self-contained experiment or tool:

```
runtime/
├── locomo/           # LoCoMo conversational memory benchmark
└── README.md
```

## Running Experiments

Each experiment directory has its own README with specific instructions.
Generally you'll want the Amadeus server running first:

```bash
cargo run --features full -- --server 3000
```

## Measuring with openbench-quick

For benchmarks that use the `openbench-quick` harness:

```bash
# Baseline: direct model evaluation
scripts/openbench-quick \
  --base-url http://118.31.102.225:1112/v1 \
  --model gemma-4-26b-a4b-it-fp8 \
  --benchmarks mini_locomo

# With MemoryAgent (run the experiment script instead)
cd runtime/locomo && python memory_agent_runner.py
```
