#!/usr/bin/env bash
# Run the LoCoMo baseline via openbench-quick.
#
# This measures raw model performance: the full conversation is fed as one
# prompt, and the model answers without any memory system.
#
# Usage:
#   ./run_baseline.sh
#
# Prerequisites:
#   - openbench project at ~/Dev/openbench with .venv set up
#   - scripts/openbench-quick symlinked or on PATH

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

# Default model config (override via env vars)
BASE_URL="${LOCOMO_BASE_URL:-http://118.31.102.225:1112/v1}"
MODEL="${LOCOMO_MODEL:-gemma-4-26b-a4b-it-fp8}"
API_KEY="${LOCOMO_API_KEY:-EMPTY}"
JUDGE_BASE_URL="${LOCOMO_JUDGE_BASE_URL:-}"
JUDGE_MODEL="${LOCOMO_JUDGE_MODEL:-}"
OUTPUT_DIR="${LOCOMO_OUTPUT_DIR:-$SCRIPT_DIR/results}"
mkdir -p "$OUTPUT_DIR"

TIMESTAMP=$(date +%Y%m%dT%H%M%S)
OUTPUT_FILE="$OUTPUT_DIR/baseline_${TIMESTAMP}.json"

echo "=== LoCoMo Baseline ==="
echo "  Endpoint: $BASE_URL"
echo "  Model:    $MODEL"
echo "  Output:   $OUTPUT_FILE"
echo ""

# Build judge flags if judge backend is configured
JUDGE_FLAGS=()
if [[ -n "$JUDGE_BASE_URL" && -n "$JUDGE_MODEL" ]]; then
    JUDGE_FLAGS=(--judge-base-url "$JUDGE_BASE_URL" --judge-model "$JUDGE_MODEL")
    if [[ -n "${LOCOMO_JUDGE_API_KEY:-}" ]]; then
        JUDGE_FLAGS+=(--judge-api-key "$LOCOMO_JUDGE_API_KEY")
    fi
fi

"$PROJECT_ROOT/scripts/openbench-quick" \
    --base-url "$BASE_URL" \
    --model "$MODEL" \
    --api-key "$API_KEY" \
    --benchmarks "mini_locomo" \
    --verbose \
    --output "$OUTPUT_FILE" \
    --no-persist \
    "${JUDGE_FLAGS[@]}"

echo ""
echo "Baseline complete. Results saved to $OUTPUT_FILE"

# Print quick summary
if [[ -f "$OUTPUT_FILE" ]]; then
    # Try to extract key metrics if python is available
    if command -v python3 &>/dev/null; then
        python3 -c "
import json
with open('$OUTPUT_FILE') as f:
    data = json.load(f)
# openbench writes an array of run results
for run in (data if isinstance(data, list) else [data]):
    m = run.get('metrics', {})
    print(f'  Accuracy:     {m.get(\"accuracy\", \"N/A\"):.3f}' if isinstance(m.get('accuracy'), float) else f'  Accuracy:     {m.get(\"accuracy\", \"N/A\")}')
    print(f'  LoCoMo Core:  {m.get(\"locomo_core_accuracy\", \"N/A\"):.3f}' if isinstance(m.get('locomo_core_accuracy'), float) else f'  LoCoMo Core:  {m.get(\"locomo_core_accuracy\", \"N/A\")}')
    print(f'  Correct/Total: {m.get(\"correct\", \"?\")}/{m.get(\"attempted\", \"?\")}')
    if 'category_breakdown' in m:
        print(f'  Categories:')
        for cat, stats in m['category_breakdown'].items():
            acc = stats.get('accuracy', 0)
            print(f'    {cat}: {acc:.3f} ({stats.get(\"correct\", 0)}/{stats.get(\"count\", 0)})')
" 2>/dev/null || true
    fi
fi
