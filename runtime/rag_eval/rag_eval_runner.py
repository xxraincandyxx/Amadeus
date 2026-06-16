#!/usr/bin/env python3
"""RAG retrieval quality evaluation — LLM-based QA from retrieved chunks.

Ingests LoCoMo conversation sessions as RAG documents, then measures
whether the LLM can answer questions from top-k retrieved chunks vs.
the full conversation (oracle upper bound).

Matches the openbench LoCoMo benchmark data format and scoring heuristics.

Usage:
    python rag_eval_runner.py --server http://localhost:3000
    python rag_eval_runner.py --server http://localhost:3000 --num-items 5
    python rag_eval_runner.py --server http://localhost:3000 --qa-per-item 30
"""

from __future__ import annotations

import argparse
import asyncio
import json
import random
import re
import string
import sys
import time
from pathlib import Path
from typing import Optional

import httpx

# --- paths -------------------------------------------------------------------
PROJECT_ROOT = Path(__file__).resolve().parents[2]
SDK_PATH = PROJECT_ROOT / "python-sdk"
sys.path.insert(0, str(SDK_PATH))

from amadeus_sdk import MemoryAgent  # noqa: E402
from amadeus_sdk.types import RagSearchResult  # noqa: E402


# ── LoCoMo data loading (matches openbench locomo.py) ────────────────────

LOCOMO_CATEGORY_MAP: dict[int, str] = {
    1: "Multi-Hop",
    2: "Temporal",
    3: "Open Domain",
    4: "Single-Hop",
    5: "Adversarial",
}

_LOCOMO_DATASET_PATHS = [
    Path.home() / "Dev/openbench/data/LoCoMo/locomo10.json",
    Path.home() / "Dev/openbench/data/locomo/locomo10.json",
    Path.home() / "Dev/openbench/refs/Archivolt/data/locomo/locomo10.json",
]

QA_PROMPT = (
    "Based on the above context, write an answer in the form of a short phrase "
    "for the following question. Answer with exact words from the context "
    "whenever possible.\n\nQuestion: {question}\nShort answer:"
)


def _find_dataset_path() -> Optional[Path]:
    for p in _LOCOMO_DATASET_PATHS:
        if p.exists():
            return p
    return None


def load_dataset(path: Optional[Path] = None) -> list[dict]:
    if path is None:
        path = _find_dataset_path()
    if path is None or not path.exists():
        print("Dataset not found. Provide --dataset-path.")
        return []
    with open(path) as f:
        data = json.load(f)
    if not isinstance(data, list):
        return []
    return data


def session_to_text(conv: dict) -> str:
    """Convert a LoCoMo conversation dict to a flat text string.

    Mirrors openbench's _get_input_context: iterates session_N keys,
    formatting speaker/text dialogs with date headers.
    """
    session_keys = [
        key for key in conv
        if key.startswith("session_") and not key.endswith("_date_time")
    ]
    session_ids = sorted(int(key.split("_")[-1]) for key in session_keys)

    chunks: list[str] = []
    for sid in session_ids:
        date_key = f"session_{sid}_date_time"
        convo_key = f"session_{sid}"
        dialogs = conv.get(convo_key, [])
        turns: list[str] = [f"DATE: {conv.get(date_key, '')}", "CONVERSATION:"]
        for dialog in dialogs:
            speaker = dialog.get("speaker", "unknown")
            text = dialog.get("text", "")
            line = f'{speaker} said, "{text}"'
            caption = dialog.get("blip_caption")
            if caption:
                line += f" and shared {caption}."
            turns.append(line)
        chunks.append("\n".join(turns))

    return "\n\n".join(chunks)


# ── Scoring heuristics (exact match to openbench locomo.py) ──────────────

def _normalize_answer(text: str) -> str:
    lowered = str(text).lower().replace(",", " ")
    punct = set(string.punctuation)
    no_punct = "".join(ch for ch in lowered if ch not in punct)
    tokens = [tok for tok in no_punct.split() if tok not in {"a", "an", "the", "and"}]
    return " ".join(tokens)


def _f1_score(predicted: str, answer: str) -> float:
    pred_tokens = _normalize_answer(predicted).split()
    ans_tokens = _normalize_answer(answer).split()
    if not pred_tokens or not ans_tokens:
        return 0.0
    pred_counts: dict[str, int] = {}
    ans_counts: dict[str, int] = {}
    for token in pred_tokens:
        pred_counts[token] = pred_counts.get(token, 0) + 1
    for token in ans_tokens:
        ans_counts[token] = ans_counts.get(token, 0) + 1
    overlap = sum(min(pred_counts.get(t, 0), ans_counts.get(t, 0)) for t in pred_counts)
    if overlap == 0:
        return 0.0
    precision = overlap / len(pred_tokens)
    recall = overlap / len(ans_tokens)
    return (2 * precision * recall) / (precision + recall)


def _multi_answer_f1(predicted: str, answer: str) -> float:
    preds = [p.strip() for p in predicted.split(",") if p.strip()]
    answers = [a.strip() for a in answer.split(",") if a.strip()]
    if not preds or not answers:
        return 0.0
    scores = [max(_f1_score(candidate, expected) for candidate in preds) for expected in answers]
    return sum(scores) / len(scores)


def score_answer(predicted: str, answer: str, category: int) -> float:
    """Score predicted answer against gold, per LoCoMo category rules."""
    if category == 5:  # Adversarial
        lowered = predicted.lower()
        if "no information available" in lowered or "not mentioned" in lowered:
            return 1.0
        return 1.0 if _f1_score(predicted, answer) >= 0.5 else 0.0

    if category in (2, 3, 4):  # Temporal, Open Domain, Single-Hop
        return 1.0 if _f1_score(predicted, answer) >= 0.5 else 0.0

    if category == 1:  # Multi-Hop
        return 1.0 if _multi_answer_f1(predicted, answer) >= 0.5 else 0.0

    return 1.0 if _f1_score(predicted, answer) >= 0.5 else 0.0


# ── LLM answer generation ────────────────────────────────────────────────

def _build_context_from_chunks(chunks: list[RagSearchResult]) -> str:
    parts: list[str] = []
    for c in chunks:
        parts.append(c.content)
    return "\n\n---\n\n".join(parts)


async def llm_answer(
    client: httpx.AsyncClient,
    llm_base_url: str,
    model: str,
    context: str,
    question: str,
    category: int,
    timeout: float = 60.0,
) -> str:
    """Ask the LLM to answer a question given context."""
    if category == 2:  # Temporal — hint for approximate dates
        question = question + " Use DATE of CONVERSATION to answer with an approximate date."

    prompt = f"{context}\n\n{QA_PROMPT.format(question=question)}"

    resp = await client.post(
        f"{llm_base_url}/chat/completions",
        json={
            "model": model,
            "messages": [{"role": "user", "content": prompt}],
            "max_tokens": 64,
            "temperature": 0.0,
        },
        timeout=timeout,
    )
    resp.raise_for_status()
    data = resp.json()
    return data["choices"][0]["message"]["content"].strip()


# ── Eval runner ──────────────────────────────────────────────────────────

async def run_rag_eval(
    server_url: str,
    num_items: int = 0,
    dataset_path: Optional[Path] = None,
    top_k: int = 5,
    qa_per_item: int = 50,
    quick: bool = False,
    seed: int = 42,
    llm_base_url: str = "http://118.31.102.225:1112/v1",
    llm_model: str = "gemma-4-26b-a4b-it-fp8",
    no_full_context: bool = False,
) -> dict:
    """Run RAG retrieval evaluation with LLM-based answer generation.

    For each QA pair:
    1. Retrieve top-k chunks via RAG embedding search
    2. Ask LLM to answer from chunks → score vs gold
    3. (Optional) Ask LLM to answer from full conversation → oracle score
    """
    all_items = load_dataset(dataset_path)
    if not all_items:
        return {"error": "No dataset found"}

    rng = random.Random(seed)

    if num_items > 0 and num_items < len(all_items):
        by_category: dict[int, list[dict]] = {}
        for item in all_items:
            qa = item.get("qa", [])
            if qa:
                cat = int(qa[0].get("category", 4))
                by_category.setdefault(cat, []).append(item)

        sampled: list[dict] = []
        for cat in sorted(by_category):
            cat_items = by_category[cat]
            target = max(1, round(len(cat_items) / len(all_items) * num_items))
            sampled.extend(rng.sample(cat_items, min(target, len(cat_items))))
        rng.shuffle(sampled)
        items = sampled[:num_items]
    elif quick and len(all_items) > 10:
        items = rng.sample(all_items, min(10, len(all_items)))
    else:
        items = all_items

    print(f"Starting RAG eval on {len(items)}/{len(all_items)} sessions "
          f"(top_k={top_k}, qa_per_item={qa_per_item})")
    print(f"Amadeus server: {server_url}")
    print(f"LLM: {llm_model} @ {llm_base_url}")

    async with httpx.AsyncClient(
        timeout=httpx.Timeout(120.0),
        headers={"Authorization": "Bearer EMPTY"},
    ) as llm_client, \
    MemoryAgent(server_url, timeout=300.0) as agent:

        # Track metrics
        total_qa = 0
        rag_correct = 0
        full_correct = 0
        rag_f1_sum = 0.0
        full_f1_sum = 0.0
        ingest_times: list[float] = []
        query_times: list[float] = []
        llm_times: list[float] = []
        per_item: list[dict] = []
        by_category: dict[str, dict] = {}

        for idx, item in enumerate(items):
            sample_id = item.get("sample_id", f"sample_{idx}")
            conv = item.get("conversation", {})
            qa_pairs = item.get("qa", [])
            if not qa_pairs or not conv:
                continue

            if len(qa_pairs) > qa_per_item:
                item_qa_pairs = rng.sample(qa_pairs, qa_per_item)
            else:
                item_qa_pairs = qa_pairs

            # 1. Ingest the conversation
            text = session_to_text(conv)
            t0 = time.monotonic()
            try:
                ingest_resp = await agent.rag.ingest_text(
                    text, document_id=sample_id,
                )
                ingest_time = time.monotonic() - t0
                ingest_times.append(ingest_time)
                print(f"[{idx+1}/{len(items)}] {sample_id}: "
                      f"ingested {ingest_resp.chunk_count} chunks "
                      f"({ingest_time:.1f}s)")
            except Exception as e:
                print(f"[{idx+1}/{len(items)}] {sample_id}: ingest failed — {e}")
                continue

            # 2. Run QA
            item_rag_correct = 0
            item_full_correct = 0
            item_rag_f1 = 0.0
            item_full_f1 = 0.0
            item_qa = 0

            for qa in item_qa_pairs:
                question = qa.get("question", "")
                answer = str(qa.get("answer", ""))
                category = int(qa.get("category", 4))
                if not question or not answer:
                    continue

                # 2a. RAG retrieval
                t0 = time.monotonic()
                try:
                    results = await agent.rag.query(question, top_k=top_k)
                    query_time = time.monotonic() - t0
                    query_times.append(query_time)
                except Exception as e:
                    print(f"  query failed: {e}")
                    continue

                # 2b. LLM answers from RAG chunks
                rag_context = _build_context_from_chunks(results)
                t0 = time.monotonic()
                try:
                    rag_answer = await llm_answer(
                        llm_client, llm_base_url, llm_model,
                        rag_context, question, category,
                    )
                    llm_time = time.monotonic() - t0
                    llm_times.append(llm_time)
                except Exception as e:
                    print(f"  LLM (RAG) failed: {e}")
                    continue

                rag_score = score_answer(rag_answer, answer, category)
                rag_f1 = _f1_score(rag_answer, answer)

                # 2c. Oracle: LLM answers from full conversation
                full_score = 0.0
                full_f1 = 0.0
                if not no_full_context:
                    try:
                        full_answer = await llm_answer(
                            llm_client, llm_base_url, llm_model,
                            text, question, category,
                        )
                        full_score = score_answer(full_answer, answer, category)
                        full_f1 = _f1_score(full_answer, answer)
                    except Exception as e:
                        print(f"  LLM (full) failed: {e}")

                item_qa += 1
                total_qa += 1
                rag_correct += rag_score
                full_correct += full_score
                rag_f1_sum += rag_f1
                full_f1_sum += full_f1
                item_rag_correct += rag_score
                item_full_correct += full_score
                item_rag_f1 += rag_f1
                item_full_f1 += full_f1

                # per-category tracking
                cat_name = LOCOMO_CATEGORY_MAP.get(category, f"cat_{category}")
                if cat_name not in by_category:
                    by_category[cat_name] = {
                        "qa": 0,
                        "rag_correct": 0, "rag_f1": 0.0,
                        "full_correct": 0, "full_f1": 0.0,
                    }
                by_category[cat_name]["qa"] += 1
                by_category[cat_name]["rag_correct"] += rag_score
                by_category[cat_name]["rag_f1"] += rag_f1
                by_category[cat_name]["full_correct"] += full_score
                by_category[cat_name]["full_f1"] += full_f1

            # 3. Delete the document
            try:
                await agent.rag.delete_document(sample_id)
            except Exception:
                pass

            if item_qa > 0:
                item_rag_acc = item_rag_correct / item_qa
                item_full_acc = item_full_correct / item_qa
                per_item.append({
                    "item_id": sample_id,
                    "qa_count": item_qa,
                    "rag_accuracy": round(item_rag_acc, 3),
                    "rag_avg_f1": round(item_rag_f1 / item_qa, 3),
                    "full_accuracy": round(item_full_acc, 3),
                    "full_avg_f1": round(item_full_f1 / item_qa, 3),
                })
                print(f"  {sample_id}: RAG_acc={item_rag_acc:.3f}, "
                      f"RAG_F1={item_rag_f1/item_qa:.3f}, "
                      f"Full_acc={item_full_acc:.3f}, "
                      f"Full_F1={item_full_f1/item_qa:.3f}")

        # ── Aggregate metrics ──
        if total_qa == 0:
            return {"error": "No QA pairs evaluated"}

        # Per-category breakdown
        cat_breakdown = {}
        for cat_name, stats in sorted(by_category.items()):
            if stats["qa"] > 0:
                n = stats["qa"]
                cat_breakdown[cat_name] = {
                    "qa_count": n,
                    "rag_accuracy": round(stats["rag_correct"] / n, 4),
                    "rag_avg_f1": round(stats["rag_f1"] / n, 4),
                    "full_accuracy": round(stats["full_correct"] / n, 4) if not no_full_context else None,
                    "full_avg_f1": round(stats["full_f1"] / n, 4) if not no_full_context else None,
                }

        results = {
            "num_sessions": len(items),
            "num_qa_pairs": total_qa,
            "top_k": top_k,
            "rag_accuracy": round(rag_correct / total_qa, 4),
            "rag_avg_f1": round(rag_f1_sum / total_qa, 4),
            "full_accuracy": round(full_correct / total_qa, 4) if not no_full_context else None,
            "full_avg_f1": round(full_f1_sum / total_qa, 4) if not no_full_context else None,
            "by_category": cat_breakdown,
            "avg_ingest_time_s": (
                round(sum(ingest_times) / len(ingest_times), 2)
                if ingest_times else 0
            ),
            "avg_query_time_s": (
                round(sum(query_times) / len(query_times), 2)
                if query_times else 0
            ),
            "avg_llm_time_s": (
                round(sum(llm_times) / len(llm_times), 2)
                if llm_times else 0
            ),
            "per_item": per_item,
        }

        return results


def main() -> None:
    parser = argparse.ArgumentParser(description="RAG retrieval eval (LLM-based QA)")
    parser.add_argument("--server", default="http://localhost:3000", help="Amadeus server URL")
    parser.add_argument("--num-items", type=int, default=0, help="Number of sessions (0=all)")
    parser.add_argument("--qa-per-item", type=int, default=30, help="QA pairs per session")
    parser.add_argument("--quick", action="store_true", help="Use at most 10 sessions")
    parser.add_argument("--dataset-path", type=Path, default=None)
    parser.add_argument("--top-k", type=int, default=5, help="Top-k chunks for RAG")
    parser.add_argument("--output", type=Path, default=None, help="Save results to JSON")
    parser.add_argument("--seed", type=int, default=42)
    parser.add_argument("--llm-base-url", default="http://118.31.102.225:1112/v1")
    parser.add_argument("--llm-model", default="gemma-4-26b-a4b-it-fp8")
    parser.add_argument("--no-full-context", action="store_true",
                        help="Skip full-context oracle (faster)")
    args = parser.parse_args()

    results = asyncio.run(
        run_rag_eval(
            server_url=args.server,
            num_items=args.num_items,
            dataset_path=args.dataset_path,
            top_k=args.top_k,
            qa_per_item=args.qa_per_item,
            quick=args.quick,
            seed=args.seed,
            llm_base_url=args.llm_base_url,
            llm_model=args.llm_model,
            no_full_context=args.no_full_context,
        )
    )

    print("\n" + "=" * 60)
    print("RAG Evaluation Results (LLM-based QA from retrieved chunks)")
    print("=" * 60)
    if "error" in results:
        print(f"ERROR: {results['error']}")
        return

    print(f"Sessions evaluated: {results['num_sessions']}")
    print(f"QA pairs:           {results['num_qa_pairs']}")
    print(f"Top-k:              {results['top_k']}")
    print()
    print(f"RAG accuracy:       {results['rag_accuracy']:.4f}")
    print(f"RAG avg F1:         {results['rag_avg_f1']:.4f}")
    if results.get("full_accuracy") is not None:
        print(f"Full-context acc:   {results['full_accuracy']:.4f}")
        print(f"Full-context F1:    {results['full_avg_f1']:.4f}")
        retention = (results['rag_accuracy'] / results['full_accuracy']
                     if results['full_accuracy'] > 0 else 0)
        print(f"Retrieval retention: {retention:.1%}")
    print(f"Avg ingest time:    {results['avg_ingest_time_s']}s")
    print(f"Avg query time:     {results['avg_query_time_s']}s")
    print(f"Avg LLM time:       {results['avg_llm_time_s']}s")

    if results.get("by_category"):
        print("\n─── By Category ───")
        for cat_name, stats in results["by_category"].items():
            line = (f"  {cat_name:15s}  n={stats['qa_count']:3d}  "
                    f"RAG_acc={stats['rag_accuracy']:.3f}  "
                    f"RAG_F1={stats['rag_avg_f1']:.3f}")
            if stats.get("full_accuracy") is not None:
                line += f"  Full_acc={stats['full_accuracy']:.3f}"
            print(line)

    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        with open(args.output, "w") as f:
            json.dump(results, f, indent=2)
        print(f"\nResults saved to {args.output}")


if __name__ == "__main__":
    main()
