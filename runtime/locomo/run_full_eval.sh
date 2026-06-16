#!/usr/bin/env bash
# Run a complete MemoryAgent evaluation: LoCoMo + MMLU.
#
# Measures both memory recall (LoCoMo) and general knowledge retention (MMLU).
# A good memory system should improve LoCoMo without degrading MMLU.
#
# Usage:
#   ./run_full_eval.sh
#
# Requires:
#   - Amadeus server running (cargo run --features full -- --server 3000)
#   - openbench at ~/Dev/openbench with .venv

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

BASE_URL="${LOCOMO_BASE_URL:-http://localhost:1112/v1}"
MODEL="${LOCOMO_MODEL:-gemma-4-26b-a4b-it-fp8}"
API_KEY="${LOCOMO_API_KEY:-EMPTY}"
SERVER_URL="${AMADEUS_SERVER:-http://localhost:3000}"

OUTPUT_DIR="$SCRIPT_DIR/results"
mkdir -p "$OUTPUT_DIR"

TIMESTAMP=$(date +%Y%m%dT%H%M%S)
BASELINE_OUT="$OUTPUT_DIR/full_baseline_${TIMESTAMP}.json"
MA_OUT="$OUTPUT_DIR/full_memory_agent_${TIMESTAMP}.json"

echo "================================================"
echo "  Full MemoryAgent Evaluation"
echo "  Date: $(date)"
echo "================================================"
echo ""
echo "Phase 1: Baseline (direct model, no memory)"
echo "--------------------------------------------"

# Step 1: Run baseline (both mini_locomo and mini_mmlu via openbench-quick)
"$PROJECT_ROOT/scripts/openbench-quick" \
    --base-url "$BASE_URL" \
    --model "$MODEL" \
    --api-key "$API_KEY" \
    --benchmarks "mini_locomo mini_mmlu" \
    --verbose \
    --output "$BASELINE_OUT" \
    --no-persist

echo ""
echo "Phase 2: MemoryAgent (server: $SERVER_URL)"
echo "--------------------------------------------"

# Step 2: Run MemoryAgent on LoCoMo
cd "$SCRIPT_DIR"
python3 memory_agent_runner.py \
    --server "$SERVER_URL" \
    --num-items 0 \
    --output "$MA_OUT"

echo ""
echo "================================================"
echo "  Evaluation Complete"
echo "================================================"
echo ""
echo "Results:"
echo "  Baseline:      $BASELINE_OUT"
echo "  MemoryAgent:   $MA_OUT"
echo ""

# Print comparison if both files exist
if [[ -f "$BASELINE_OUT" && -f "$MA_OUT" ]]; then
    echo "Comparison:"
    python3 compare_results.py "$BASELINE_OUT" "$MA_OUT"
fi
