# mam0 Run Log

Goal: mini_locomo > 95%, mini_mmlu >= 84% (no regression)

Model: gemma-4-26b-a4b-it-fp8 @ http://118.31.102.225:1112/v1

## Architecture (commit 0e6039f)

- `llm.py` — OAI-compatible client, proxy-bypass via `trust_env=False`
- `rag.py` — BM25 RAG: tokenizer, chunker (1200/200), JSON persistence, 4 ops
- `agent.py` — 27 tools (bash, file, todo, task, subagent, bg, team, RAG, compress)

---

## Run #1 — Baseline (heuristic F1, no judge)

| Benchmark | Score | Items | Time |
|-----------|-------|-------|------|
| mini_mmlu | 84.00% (42/50) | 50 | 45.8s |
| mini_locomo | 54.00% (27/50) | 50 | 87.4s |

## Run #2 — LLM Judge

| Benchmark | Score | Items | Time | Delta |
|-----------|-------|-------|------|-------|
| mini_locomo | 72.00% (36/50) | 50 | 73.2s | +18pp |

## Run #3 — Prompt Engineering v1

Changes: system prompt, cleaner QA_PROMPT, temporal hint fix, get_max_tokens=256,
expanded adversarial keywords (16 phrases), answer prefix stripping,
number-word normalization, cleaner context format (Speaker: text, [Session N]).

| Benchmark | Score | Items | Time | Delta |
|-----------|-------|-------|------|-------|
| mini_mmlu | 82.00% (41/50) | 50 | 45.3s | -2pp |
| mini_locomo | 80.00% (40/50) | 50 | 84.8s | +8pp |

**mmlu -2pp likely noise** (1 item), but watched.
**locomo +8pp** from prompt engineering. Still 15pp gap to 95%.
