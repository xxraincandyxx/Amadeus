#!/usr/bin/env python3
"""RAG retrieval quality evaluation vs baseline substring search.

Ingests LoCoMo conversation sessions as RAG documents, then measures
retrieval quality using recall@k, MRR, and hit@1.

Usage:
    python rag_eval_runner.py --server http://localhost:3000 --num-items 5
    python rag_eval_runner.py --server http://localhost:3000 --num-items 0  # all
"""

from __future__ import annotations

import argparse
import asyncio
import json
import math
import re
import sys
import time
from pathlib import Path
from typing import Optional

# --- paths -------------------------------------------------------------------
PROJECT_ROOT = Path(__file__).resolve().parents[2]
SDK_PATH = PROJECT_ROOT / "python-sdk"
sys.path.insert(0, str(SDK_PATH))

from amadeus_sdk import MemoryAgent  # noqa: E402
from amadeus_sdk.types import RagSearchResult  # noqa: E402


# ── LoCoMo data loading (compatible with openbench format) ─────────────────

# Try multiple paths for the dataset
def _find_dataset_path() -> Optional[Path]:
    candidates = [
        Path.home() / "Dev/openbench/datasets/locomo10.json",
        Path.home() / ".cache/openbench/locomo10.json",
        PROJECT_ROOT / "runtime/locomo/locomo10.json",
    ]
    for p in candidates:
        if p.exists():
            return p
    return None


def load_dataset(path: Optional[Path] = None) -> list[dict]:
    """Load LoCoMo dataset, returning a list of item dicts.

    Each item has: id, conversation (list of turns), qa (list of QA pairs).
    """
    if path is None:
        path = _find_dataset_path()
    if path is None or not path.exists():
        print(f"Dataset not found at candidates. Provide --dataset-path.")
        return []
    with open(path) as f:
        data = json.load(f)
    # Handle both list and dict-wrapped formats
    if isinstance(data, dict):
        for v in data.values():
            if isinstance(v, list):
                return v
    return data


def session_to_text(item: dict) -> str:
    """Convert a LoCoMo conversation session to a flat text string for chunking."""
    lines = []
    for turn in item.get("conversation", []):
        role = turn.get("role", turn.get("speaker", "unknown"))
        text = turn.get("content", turn.get("text", ""))
        lines.append(f"{role}: {text}")
    return "\n".join(lines)


def get_qa_pairs(item: dict) -> list[dict]:
    """Extract QA pairs from a dataset item."""
    return item.get("qa", [])


# ── Retrieval metrics ──────────────────────────────────────────────────────

def normalize_text(text: str) -> str:
    """Simple normalization: lowercase, collapse whitespace."""
    return re.sub(r"\s+", " ", text.lower().strip())


def answer_in_chunks(answer: str, chunks: list[RagSearchResult]) -> bool:
    """Check if the answer text appears in any retrieved chunk."""
    norm_answer = normalize_text(answer)
    for chunk in chunks:
        if norm_answer in normalize_text(chunk.content):
            return True
    return False


def compute_mrr(answer: str, results: list[RagSearchResult]) -> float:
    """Compute reciprocal rank: 1/rank of first hit, or 0 if no hit."""
    norm_answer = normalize_text(answer)
    for result in results:
        if norm_answer in normalize_text(result.content):
            return 1.0 / result.rank
    return 0.0


def compute_hit_at_k(answer: str, results: list[RagSearchResult], k: int = 1) -> bool:
    """True if answer is in top-k results."""
    top_k = results[:k]
    return answer_in_chunks(answer, top_k)


# ── Eval runner ─────────────────────────────────────────────────────────────

async def run_rag_eval(
    server_url: str,
    num_items: int = 5,
    dataset_path: Optional[Path] = None,
    top_k: int = 5,
) -> dict:
    """Run RAG retrieval evaluation.

    1. Load dataset
    2. For each item: ingest conversation as RAG document, run QA queries
    3. Collect metrics: recall@k, MRR, hit@1
    """
    items = load_dataset(dataset_path)
    if not items:
        return {"error": "No dataset found"}
    if num_items > 0:
        items = items[:num_items]

    print(f"Starting RAG eval on {len(items)} items (top_k={top_k})")
    print(f"Server: {server_url}")

    async with MemoryAgent(server_url, timeout=300.0) as agent:
        total_qa = 0
        total_hit = 0
        mrr_sum = 0.0
        hit_at_1 = 0
        ingest_times: list[float] = []
        query_times: list[float] = []
        per_item: list[dict] = []

        for idx, item in enumerate(items):
            item_id = item.get("id", f"item_{idx}")
            qa_pairs = get_qa_pairs(item)
            if not qa_pairs:
                continue

            # 1. Ingest the conversation session
            text = session_to_text(item)
            t0 = time.monotonic()
            try:
                ingest_resp = await agent.rag.ingest_text(
                    text, document_id=item_id
                )
                ingest_time = time.monotonic() - t0
                ingest_times.append(ingest_time)
                print(f"[{idx+1}/{len(items)}] {item_id}: "
                      f"ingested {ingest_resp.chunk_count} chunks "
                      f"({ingest_time:.1f}s)")
            except Exception as e:
                print(f"[{idx+1}/{len(items)}] {item_id}: ingest failed — {e}")
                continue

            # 2. Run queries against the ingested document
            item_hits = 0
            item_mrr = 0.0
            item_qa = 0

            for qa in qa_pairs[:10]:  # limit to 10 QA pairs per item
                question = qa.get("question", qa.get("q", ""))
                answer = qa.get("answer", qa.get("a", ""))
                if not question or not answer:
                    continue

                t0 = time.monotonic()
                try:
                    results = await agent.rag.query(question, top_k=top_k)
                    query_time = time.monotonic() - t0
                    query_times.append(query_time)
                except Exception as e:
                    print(f"  query failed: {e}")
                    continue

                hit = answer_in_chunks(answer, results)
                mrr = compute_mrr(answer, results)
                h1 = compute_hit_at_k(answer, results, k=1)

                item_qa += 1
                total_qa += 1
                if hit:
                    total_hit += 1
                    item_hits += 1
                mrr_sum += mrr
                item_mrr += mrr
                if h1:
                    hit_at_1 += 1

            # 3. Delete the document to keep the store lean
            try:
                await agent.rag.delete_document(item_id)
            except Exception:
                pass

            if item_qa > 0:
                item_recall = item_hits / item_qa
                item_avg_mrr = item_mrr / item_qa
                per_item.append({
                    "item_id": item_id,
                    "qa_count": item_qa,
                    "recall_at_k": round(item_recall, 3),
                    "mrr": round(item_avg_mrr, 3),
                })
                print(f"  {item_id}: recall@{top_k}={item_recall:.3f}, "
                      f"MRR={item_avg_mrr:.3f}")

        # ── Aggregate metrics ──
        if total_qa == 0:
            return {"error": "No QA pairs evaluated"}

        recall_at_k = total_hit / total_qa
        avg_mrr = mrr_sum / total_qa
        hit1_rate = hit_at_1 / total_qa

        results = {
            "num_items": len(items),
            "num_qa_pairs": total_qa,
            f"recall_at_{top_k}": round(recall_at_k, 4),
            "mrr": round(avg_mrr, 4),
            "hit_at_1": round(hit1_rate, 4),
            "avg_ingest_time_s": (
                round(sum(ingest_times) / len(ingest_times), 2)
                if ingest_times
                else 0
            ),
            "avg_query_time_s": (
                round(sum(query_times) / len(query_times), 2)
                if query_times
                else 0
            ),
            "per_item": per_item,
        }

        return results


def main() -> None:
    parser = argparse.ArgumentParser(description="RAG retrieval eval")
    parser.add_argument(
        "--server", default="http://localhost:3000",
        help="Amadeus server URL"
    )
    parser.add_argument(
        "--num-items", type=int, default=5,
        help="Number of LoCoMo items to evaluate (0 = all)"
    )
    parser.add_argument(
        "--dataset-path", type=Path, default=None,
        help="Path to LoCoMo dataset JSON"
    )
    parser.add_argument(
        "--top-k", type=int, default=5,
        help="Top-k for recall computation"
    )
    parser.add_argument(
        "--output", type=Path, default=None,
        help="Save results to JSON file"
    )
    args = parser.parse_args()

    results = asyncio.run(
        run_rag_eval(
            server_url=args.server,
            num_items=args.num_items,
            dataset_path=args.dataset_path,
            top_k=args.top_k,
        )
    )

    print("\n" + "=" * 60)
    print("RAG Retrieval Evaluation Results")
    print("=" * 60)
    if "error" in results:
        print(f"ERROR: {results['error']}")
        return

    print(f"Items evaluated:  {results['num_items']}")
    print(f"QA pairs:         {results['num_qa_pairs']}")
    print(f"Recall@{args.top_k}:        {results[f'recall_at_{args.top_k}']:.4f}")
    print(f"MRR:              {results['mrr']:.4f}")
    print(f"Hit@1:            {results['hit_at_1']:.4f}")
    print(f"Avg ingest time:  {results['avg_ingest_time_s']}s")
    print(f"Avg query time:   {results['avg_query_time_s']}s")

    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        with open(args.output, "w") as f:
            json.dump(results, f, indent=2)
        print(f"\nResults saved to {args.output}")


if __name__ == "__main__":
    main()
