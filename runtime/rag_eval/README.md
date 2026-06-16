# RAG Eval

Measures retrieval quality of the RAG embedding-based semantic search.

## Usage

```bash
python runtime/rag_eval/rag_eval_runner.py \
  --server http://localhost:3000 \
  --num-items 5 \
  --top-k 5
```

With a custom dataset path and JSON output:

```bash
python runtime/rag_eval/rag_eval_runner.py \
  --server http://localhost:3000 \
  --dataset-path ~/Dev/openbench/datasets/locomo10.json \
  --num-items 10 \
  --top-k 5 \
  --output runtime/rag_eval/results/rag_eval.json
```

## What it measures

| Metric | Description |
|--------|-------------|
| Recall@k | Fraction of QA pairs where the answer appears in the top-k chunks |
| MRR | Mean Reciprocal Rank — average of 1/rank of first hit |
| Hit@1 | Fraction of QA pairs where the answer is in the top-1 chunk |

## How it works

1. Loads LoCoMo conversation sessions from the dataset
2. Ingests each session as a RAG document (chunk → embed → store)
3. For each QA pair: embeds the question, retrieves top-k chunks, checks if answer text is present
4. Computes aggregate retrieval metrics
