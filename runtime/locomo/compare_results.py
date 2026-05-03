#!/usr/bin/env python3
"""Compare baseline vs MemoryAgent LoCoMo results.

Usage:
    python compare_results.py results/baseline_20260503T120000.json results/memory_agent_20260503T130000.json
"""

import json
import sys
from pathlib import Path


def load_openbench_result(path: str) -> dict | None:
    """Load a result file and normalize to a common format."""
    data = json.loads(Path(path).read_text())

    # openbench-quick outputs an array of run results
    if isinstance(data, list):
        data = data[0] if data else {}

    metrics = data.get("metrics", {})
    return {
        "source": Path(path).name,
        "accuracy": metrics.get("accuracy", 0),
        "locomo_core_accuracy": metrics.get("locomo_core_accuracy"),
        "correct": metrics.get("correct", 0),
        "attempted": metrics.get("attempted", 0),
        "category_breakdown": metrics.get("category_breakdown", {}),
    }


def load_memory_agent_result(path: str) -> dict:
    """Load a MemoryAgent result (already in unified format)."""
    data = json.loads(Path(path).read_text())
    return {
        "source": Path(path).name,
        "accuracy": data.get("accuracy", 0),
        "locomo_core_accuracy": data.get("locomo_core_accuracy"),
        "correct": data.get("correct", 0),
        "attempted": data.get("attempted", 0),
        "category_breakdown": data.get("category_breakdown", {}),
    }


def print_comparison(baseline: dict, memory_agent: dict) -> None:
    print("=== LoCoMo Comparison: Baseline vs MemoryAgent ===\n")

    # Header
    print(f"{'Metric':<25} {'Baseline':>12} {'MemoryAgent':>12} {'Delta':>12}")
    print("-" * 61)

    # Accuracy
    b_acc = baseline["accuracy"]
    m_acc = memory_agent["accuracy"]
    delta_acc = m_acc - b_acc
    print(f"{'Accuracy':<25} {b_acc:>12.3f} {m_acc:>12.3f} {delta_acc:>+12.3f}")

    # LoCoMo Core
    b_core = baseline.get("locomo_core_accuracy")
    m_core = memory_agent.get("locomo_core_accuracy")
    if b_core is not None and m_core is not None:
        delta_core = m_core - b_core
        print(f"{'LoCoMo Core':<25} {b_core:>12.3f} {m_core:>12.3f} {delta_core:>+12.3f}")

    # Counts
    b_count = str(baseline["correct"]) + "/" + str(baseline["attempted"])
    m_count = str(memory_agent["correct"]) + "/" + str(memory_agent["attempted"])
    print(f"{'Correct/Attempted':<25} {b_count:>12} {m_count:>12}")

    # Category breakdown
    all_cats = set(baseline.get("category_breakdown", {})) | set(
        memory_agent.get("category_breakdown", {})
    )
    if all_cats:
        print(f"\n{'Category':<25} {'Baseline':>12} {'MemoryAgent':>12} {'Delta':>12}")
        print("-" * 61)
        for cat in sorted(all_cats):
            b_cat = baseline.get("category_breakdown", {}).get(cat, {})
            m_cat = memory_agent.get("category_breakdown", {}).get(cat, {})
            b_a = b_cat.get("accuracy", 0)
            m_a = m_cat.get("accuracy", 0)
            delta_cat = m_a - b_a
            print(f"{cat:<25} {b_a:>12.3f} {m_a:>12.3f} {delta_cat:>+12.3f}")

    # Verdict
    print()
    if delta_acc > 0.02:
        print(f"MemoryAgent improves accuracy by {delta_acc:+.3f}")
    elif delta_acc < -0.02:
        print(f"MemoryAgent reduces accuracy by {delta_acc:+.3f} — investigate memory prompt quality")
    else:
        print("MemoryAgent matches baseline within noise — no significant difference")


def main():
    if len(sys.argv) < 3:
        print("Usage: python compare_results.py <baseline.json> <memory_agent.json>")
        sys.exit(1)

    baseline_path = sys.argv[1]
    ma_path = sys.argv[2]

    # Auto-detect format
    baseline = load_openbench_result(baseline_path)
    memory_agent = load_memory_agent_result(ma_path)

    print_comparison(baseline, memory_agent)


if __name__ == "__main__":
    main()
