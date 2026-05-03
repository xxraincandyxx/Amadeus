#!/usr/bin/env python3
"""LoCoMo evaluation with MemoryAgent.

Feeds LoCoMo conversations incrementally through the MemoryAgent, letting the
LLM store important facts as memories, then answers QA pairs.

Contrast with the baseline (run_baseline.sh) which dumps the entire conversation
in a single prompt.

Usage:
    python memory_agent_runner.py --server http://localhost:3000 --num-items 10
    python memory_agent_runner.py --server http://localhost:3000 --num-items 0  # all items
"""

from __future__ import annotations

import argparse
import asyncio
import json
import re
import string
import sys
import time
from pathlib import Path
from typing import Any, Optional

# Add openbench to path for dataset loading
_OPENBENCH_SRC = Path("~/Dev/openbench/src").expanduser()
if _OPENBENCH_SRC.exists():
    sys.path.insert(0, str(_OPENBENCH_SRC))

# --- paths -------------------------------------------------------------------
PROJECT_ROOT = Path(__file__).resolve().parents[2]
SDK_PATH = PROJECT_ROOT / "python-sdk"
sys.path.insert(0, str(SDK_PATH))

from amadeus_sdk import MemoryAgent  # noqa: E402


# ── LoCoMo scoring (mirrors openbench) ──────────────────────────────────────

def normalize_answer(text: str) -> str:
    lowered = text.lower().replace(",", " ")
    punct = set(string.punctuation)
    no_punct = "".join(ch for ch in lowered if ch not in punct)
    tokens = [tok for tok in no_punct.split() if tok not in {"a", "an", "the", "and"}]
    return " ".join(tokens)


def f1_score(predicted: str, answer: str) -> float:
    pred_tokens = normalize_answer(predicted).split()
    ans_tokens = normalize_answer(answer).split()
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


def multi_answer_f1(predicted: str, answer: str) -> float:
    preds = [p.strip() for p in predicted.split(",") if p.strip()]
    answers = [a.strip() for a in answer.split(",") if a.strip()]
    if not preds or not answers:
        return 0.0
    scores = [max(f1_score(candidate, expected) for candidate in preds) for expected in answers]
    return sum(scores) / len(scores)


CATEGORY_MAP = {1: "Multi-Hop", 2: "Temporal", 3: "Open Domain", 4: "Single-Hop", 5: "Adversarial"}


def check_answer(predicted: str, item: dict) -> bool:
    category = int(item.get("category", 4))
    answer = str(item.get("answer", ""))
    if category == 5:
        lowered = predicted.lower()
        if "no information available" in lowered or "not mentioned" in lowered:
            return True
        return f1_score(predicted, answer) >= 0.5
    if category in (2, 3, 4):
        return f1_score(predicted, answer) >= 0.5
    if category == 1:
        return multi_answer_f1(predicted, answer) >= 0.5
    return False


# ── Conversation helpers ────────────────────────────────────────────────────

def get_session_ids(conversation: dict) -> list[int]:
    session_keys = [k for k in conversation if k.startswith("session_") and not k.endswith("_date_time")]
    return sorted(int(k.split("_")[-1]) for k in session_keys)


def format_session(conversation: dict, session_id: int) -> str:
    date_key = f"session_{session_id}_date_time"
    convo_key = f"session_{session_id}"
    date = conversation.get(date_key, "")
    dialogs = conversation.get(convo_key, [])

    lines = [f"DATE: {date}", "CONVERSATION:"]
    for dialog in dialogs:
        speaker = dialog.get("speaker", "Speaker")
        text = dialog.get("text", "")
        caption = dialog.get("blip_caption")
        line = f'{speaker} said, "{text}"'
        if caption:
            line += f" and shared {caption}."
        lines.append(line)
    return "\n".join(lines)


# ── MemoryAgent runner ──────────────────────────────────────────────────────

async def evaluate_item(
    agent: MemoryAgent,
    item: dict,
    item_index: int,
    total: int,
) -> dict:
    """Feed conversation incrementally via MemoryAgent, then ask the question."""
    conversation = item.get("conversation", {})
    session_ids = get_session_ids(conversation)
    question = item["question"]
    expected = str(item.get("answer", ""))
    category = int(item.get("category", 4))
    category_name = CATEGORY_MAP.get(category, "Unknown")
    item_id = item.get("id", f"item-{item_index}")

    print(f"  [{item_index + 1}/{total}] {item_id} ({category_name})", end="", flush=True)

    # Step 1: Feed each session to the agent, asking it to remember key facts
    for sid in session_ids:
        session_text = format_session(conversation, sid)
        prompt = (
            f"Read this conversation session and use the memory tool to store any "
            f"important facts, events, dates, names, and details that might be asked "
            f"about later. Focus on specific information.\n\n{session_text}"
        )
        try:
            await agent.ask(prompt, timeout_secs=120)
        except Exception as e:
            print(f" [session {sid} error: {e}]", end="", flush=True)
            continue

    # Step 2: Ask the actual question
    category_hints = {
        1: "This may require combining multiple facts. List all relevant items.",
        2: "Use calendar dates from the conversation to answer with an approximate date.",
        3: "Answer with a short phrase from the conversation.",
        4: "Answer with exact words from the conversation.",
        5: "Answer with the exact fact from the conversation, or say 'not mentioned' if it isn't there.",
    }
    hint = category_hints.get(category, "")
    answer_prompt = (
        f"Based on the conversations you've read and the memories you've stored, "
        f"answer this question. Use the memory recall tool if needed. "
        f"{hint}\n\nQuestion: {question}\n\nAnswer:"
    )

    try:
        turn = await agent.ask(answer_prompt, timeout_secs=120)
        response = turn.text.strip()
    except Exception as e:
        response = f"ERROR: {e}"

    # Step 3: Score the answer
    is_correct = check_answer(response, item)
    score = 1.0 if is_correct else 0.0

    status = "correct" if is_correct else "incorrect"
    print(f" -> {status} (expected: {expected[:60]})")

    return {
        "item_id": item_id,
        "question": question,
        "expected": expected,
        "predicted": response,
        "category": category,
        "category_name": category_name,
        "score": score,
        "status": status,
    }


async def run_evaluation(
    server_url: str,
    num_items: int = 10,
    quick: bool = False,
) -> dict:
    """Run the LoCoMo evaluation with MemoryAgent."""
    # Load dataset
    from openbench.benchmarks.mini_locomo import MiniLoCoMoBenchmark

    bm = MiniLoCoMoBenchmark()
    all_items = bm.load_dataset()

    if quick:
        # Use quick subset (first N items)
        items = all_items[: min(num_items or bm.quick_size, len(all_items))]
    elif num_items > 0:
        items = all_items[:num_items]
    else:
        items = all_items

    total = len(items)
    print(f"\n=== LoCoMo MemoryAgent Evaluation ===")
    print(f"  Server: {server_url}")
    print(f"  Items:  {total}")
    print(f"")

    t0 = time.monotonic()
    item_results: list[dict] = []

    async with MemoryAgent(server_url, timeout=180.0, debug_log_dir="./debug_logs") as agent:
        # Optional: pre-seed with instruction that this is a memory test
        for i, item in enumerate(items):
            result = await evaluate_item(agent, item, i, total)
            item_results.append(result)
            # Clear history between items (each conversation is independent)
            await agent.clear_history()

    elapsed = time.monotonic() - t0

    # Compute aggregate metrics
    successful = len(item_results)
    correct = sum(1 for r in item_results if r["score"] >= 1.0)
    accuracy = correct / successful if successful > 0 else 0.0

    # Category breakdown
    cat_counts: dict[str, int] = {}
    cat_correct: dict[str, float] = {}
    for r in item_results:
        cat = r["category_name"]
        cat_counts[cat] = cat_counts.get(cat, 0) + 1
        cat_correct[cat] = cat_correct.get(cat, 0) + r["score"]

    category_breakdown = {}
    for cat, count in cat_counts.items():
        corr = cat_correct.get(cat, 0)
        category_breakdown[cat] = {
            "count": count,
            "correct": int(corr),
            "accuracy": corr / count if count > 0 else 0.0,
        }

    # LoCoMo core accuracy (excludes adversarial category 5)
    core_order = ["Single-Hop", "Multi-Hop", "Open Domain", "Temporal"]
    core_count = sum(cat_counts.get(c, 0) for c in core_order)
    core_correct = sum(cat_correct.get(c, 0) for c in core_order)
    core_accuracy = core_correct / core_count if core_count > 0 else 0.0

    result = {
        "benchmark": "mini_locomo",
        "mode": "memory_agent",
        "accuracy": accuracy,
        "locomo_core_accuracy": core_accuracy,
        "correct": correct,
        "successful": successful,
        "attempted": total,
        "elapsed_seconds": elapsed,
        "category_breakdown": category_breakdown,
        "items": item_results,
    }

    return result


# ── main ────────────────────────────────────────────────────────────────────

def main():
    parser = argparse.ArgumentParser(description="LoCoMo MemoryAgent evaluation")
    parser.add_argument("--server", default="http://localhost:3000", help="Amadeus server URL")
    parser.add_argument("--num-items", type=int, default=10, help="Number of items (0=all)")
    parser.add_argument("--quick", action="store_true", help="Use quick subset")
    parser.add_argument("--output", "-o", help="Output JSON file path")
    args = parser.parse_args()

    result = asyncio.run(run_evaluation(args.server, args.num_items, args.quick))

    # Output
    if args.output:
        output_path = Path(args.output)
    else:
        ts = time.strftime("%Y%m%dT%H%M%S")
        outdir = Path(__file__).resolve().parent / "results"
        outdir.mkdir(exist_ok=True)
        output_path = outdir / f"memory_agent_{ts}.json"

    output_path.write_text(json.dumps(result, indent=2, ensure_ascii=False))
    print(f"\nResults saved to {output_path}")

    # Summary
    print(f"\n=== Summary ===")
    print(f"  Accuracy:        {result['accuracy']:.3f}")
    print(f"  LoCoMo Core:     {result['locomo_core_accuracy']:.3f}")
    print(f"  Correct/Total:   {result['correct']}/{result['attempted']}")
    print(f"  Time:            {result['elapsed_seconds']:.1f}s")
    print(f"  Categories:")
    for cat, stats in sorted(result["category_breakdown"].items()):
        print(f"    {cat}: {stats['accuracy']:.3f} ({stats['correct']}/{stats['count']})")


if __name__ == "__main__":
    main()
