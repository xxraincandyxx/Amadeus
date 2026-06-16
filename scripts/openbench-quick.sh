#!/usr/bin/env bash
# openbench-quick — fast smoke tests against an OpenAI-compatible LLM API.
#
# Default benchmarks: mini_locomo, mini_mmlu
#
# Usage:
#   openbench-quick --base-url https://api.openai.com/v1 --model gpt-4o --api-key sk-xxx
#   openbench-quick --base-url https://openrouter.ai/api/v1 --model meta-llama/llama-4-maverick --api-key sk-xxx
#   openbench-quick --base-url http://localhost:8080/v1 --model local-model --benchmarks mini_locomo
#
# Install:
#   ln -s "$PWD/scripts/openbench-quick" /usr/local/bin/openbench-quick
#   export OPENBENCH_ROOT="$PWD"  # or set in ~/.zshrc
set -euo pipefail

# ── project root detection ───────────────────────────────────────────
_detect_root() {
    if [[ -n "${OPENBENCH_ROOT:-}" ]]; then
        echo "$OPENBENCH_ROOT"
        return
    fi
    # try the canonical path from this session
    if [[ -d "$HOME/Dev/openbench" ]]; then
        echo "$HOME/Dev/openbench"
        return
    fi
    # try relative to the script's own resolved location
    local dir
    dir="$(cd "$(dirname "$0")/.." 2>/dev/null && pwd)"
    if [[ -f "$dir/pyproject.toml" ]]; then
        echo "$dir"
        return
    fi
    echo ""
}

OPENBENCH_ROOT="$(_detect_root)"
if [[ -z "$OPENBENCH_ROOT" ]]; then
    echo "ERROR: cannot find openbench project root." >&2
    echo "  Set OPENBENCH_ROOT in your environment, or run from inside the repo." >&2
    exit 1
fi

PYTHON_BIN="${OPENBENCH_ROOT}/.venv/bin/python3"
if [[ ! -x "$PYTHON_BIN" ]]; then
    echo "ERROR: venv python not found at $PYTHON_BIN" >&2
    echo "  Create a venv at .venv and install openbench first." >&2
    exit 1
fi

# ── defaults ─────────────────────────────────────────────────────────
BENCHMARKS="mini_locomo mini_mmlu"
QUICK=""
CONCURRENCY=""
TIMEOUT_S=""
MAX_RETRIES=""
VERBOSE=""
JSON_OUT=""
OUTPUT_FILE=""
BENCHMARK_CONFIG=""
JUDGE_BASE_URL=""
JUDGE_MODEL=""
JUDGE_API_KEY=""
NO_PERSIST=""
ANTHROPIC_MODE=""

# ── usage ────────────────────────────────────────────────────────────
usage() {
    cat <<'EOF'
openbench-quick — run fast benchmark smoke tests against an LLM API.

USAGE
  openbench-quick --base-url <URL> --model <ID> [FLAGS]

REQUIRED
  --base-url URL       OpenAI-compatible API endpoint
  --model   ID          Model identifier at the endpoint

AUTH
  --api-key KEY         API key, or env:VAR_NAME, or "EMPTY"
                        Falls back to $OPENAI_API_KEY if unset.
  --anthropic           Use $ANTHROPIC_API_KEY as default api-key

BENCHMARKS
  --benchmarks LIST     Space-separated benchmark names
                        (default: mini_locomo mini_mmlu)

RUNTIME
  --quick               Use each benchmark's quick subset
  --concurrency N       Max concurrent requests  [default: 1]
  --timeout-s S         Per-request timeout in seconds
  --max-retries N       Retries on transient errors  [default: 2]
  --verbose -v          Print per-item progress

JUDGE (optional, for benchmarks like locomo)
  --judge-base-url URL   Judge API endpoint
  --judge-model ID       Judge model identifier
  --judge-api-key KEY    Judge API key

OUTPUT
  --json                Print full run result JSON to stdout
  --output PATH         Write full run result JSON to file
  --no-persist          Skip persisting results to data/openbench.db
  --benchmark-config P  Path to benchmark JSON config override

ENVIRONMENT
  OPENBENCH_ROOT        Path to openbench project root
  OPENAI_API_KEY        Fallback API key
  ANTHROPIC_API_KEY     Fallback API key (with --anthropic)
EOF
}

# ── parse arguments ──────────────────────────────────────────────────
BASE_URL=""
MODEL=""
API_KEY=""

while [[ $# -gt 0 ]]; do
    case "$1" in
        --base-url)        BASE_URL="$2"; shift 2 ;;
        --model)           MODEL="$2"; shift 2 ;;
        --api-key)         API_KEY="$2"; shift 2 ;;
        --anthropic)       ANTHROPIC_MODE="1"; shift ;;
        --benchmarks)      BENCHMARKS="$2"; shift 2 ;;
        --quick)           QUICK="--quick"; shift ;;
        --concurrency)     CONCURRENCY="$2"; shift 2 ;;
        --timeout-s)       TIMEOUT_S="$2"; shift 2 ;;
        --max-retries)     MAX_RETRIES="$2"; shift 2 ;;
        --verbose|-v)      VERBOSE="--verbose"; shift ;;
        --json)            JSON_OUT="--json"; shift ;;
        --output)          OUTPUT_FILE="$2"; shift 2 ;;
        --no-persist)      NO_PERSIST="--no-persist-db"; shift ;;
        --benchmark-config) BENCHMARK_CONFIG="$2"; shift 2 ;;
        --judge-base-url)  JUDGE_BASE_URL="$2"; shift 2 ;;
        --judge-model)     JUDGE_MODEL="$2"; shift 2 ;;
        --judge-api-key)   JUDGE_API_KEY="$2"; shift 2 ;;
        --help|-h)         usage; exit 0 ;;
        *) echo "ERROR: unknown flag: $1" >&2; usage >&2; exit 2 ;;
    esac
done

# ── validate ─────────────────────────────────────────────────────────
if [[ -z "$BASE_URL" ]]; then
    echo "ERROR: --base-url is required" >&2
    usage >&2; exit 2
fi
if [[ -z "$MODEL" ]]; then
    echo "ERROR: --model is required" >&2
    usage >&2; exit 2
fi

# -- resolve api key --
_resolve_key() {
    local explicit="$1"
    local env_name="$2"

    if [[ -n "$explicit" ]]; then
        if [[ "$explicit" == env:* ]]; then
            local var="${explicit#env:}"
            echo "${!var:-}"
        elif [[ "$explicit" == "EMPTY" ]]; then
            echo "EMPTY"
        else
            echo "$explicit"
        fi
    elif [[ -n "${!env_name:-}" ]]; then
        echo "${!env_name}"
    else
        echo ""
    fi
}

KEY_ENV="OPENAI_API_KEY"
if [[ -n "$ANTHROPIC_MODE" ]]; then
    KEY_ENV="ANTHROPIC_API_KEY"
fi

RESOLVED_KEY="$(_resolve_key "$API_KEY" "$KEY_ENV")"
if [[ -z "$RESOLVED_KEY" ]]; then
    echo "NOTE: no API key provided (--api-key / $KEY_ENV). Attempting unauthenticated." >&2
fi

# ── build shared flags ───────────────────────────────────────────────
SHARED_FLAGS=(
    --base-url "$BASE_URL"
    --model "$MODEL"
)
if [[ -n "$RESOLVED_KEY" ]]; then
    SHARED_FLAGS+=(--api-key "$RESOLVED_KEY")
fi
[[ -n "$QUICK" ]]             && SHARED_FLAGS+=("$QUICK")
[[ -n "$CONCURRENCY" ]]       && SHARED_FLAGS+=(--concurrency "$CONCURRENCY")
[[ -n "$TIMEOUT_S" ]]         && SHARED_FLAGS+=(--timeout-s "$TIMEOUT_S")
[[ -n "$MAX_RETRIES" ]]       && SHARED_FLAGS+=(--max-retries "$MAX_RETRIES")
[[ -n "$VERBOSE" ]]           && SHARED_FLAGS+=("$VERBOSE")
[[ -n "$JSON_OUT" ]]          && SHARED_FLAGS+=("$JSON_OUT")
[[ -n "$OUTPUT_FILE" ]]       && SHARED_FLAGS+=(-o "$OUTPUT_FILE")
[[ -n "$NO_PERSIST" ]]        && SHARED_FLAGS+=("$NO_PERSIST")
[[ -n "$BENCHMARK_CONFIG" ]]  && SHARED_FLAGS+=(--benchmark-config "$BENCHMARK_CONFIG")

# -- judge flags (only passed if at least base-url + model are given) --
JUDGE_FLAGS=()
if [[ -n "${JUDGE_BASE_URL:-}" && -n "${JUDGE_MODEL:-}" ]]; then
    JUDGE_FLAGS+=(--judge-base-url "$JUDGE_BASE_URL" --judge-model "$JUDGE_MODEL")
    if [[ -n "$JUDGE_API_KEY" ]]; then
        JUDGE_FLAGS+=(--judge-api-key "$JUDGE_API_KEY")
    fi
fi

# ── run ──────────────────────────────────────────────────────────────
cd "$OPENBENCH_ROOT"

PASS=0
FAIL=0

for bench in $BENCHMARKS; do
    echo ""
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    echo "  openbench-quick ▶ $bench"
    echo "  endpoint: $BASE_URL"
    echo "  model:    $MODEL"
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

    # Build cmd array (compatible with bash 3.2 empty-array + set -u)
    CMD=("$PYTHON_BIN" -m openbench.cli run "$bench" "${SHARED_FLAGS[@]}")
    if [[ ${#JUDGE_FLAGS[@]} -gt 0 ]]; then
        CMD+=("${JUDGE_FLAGS[@]}")
    fi

    set +e
    "${CMD[@]}"
    rc=$?
    set -e

    if [[ $rc -eq 0 ]]; then
        PASS=$((PASS + 1))
        echo "  ✔ $bench passed"
    else
        FAIL=$((FAIL + 1))
        echo "  ✖ $bench failed (exit $rc)" >&2
    fi
done

echo ""
echo "─── results ────────────────────────────────────────────────────"
echo "  passed: $PASS  failed: $FAIL  total: $((PASS + FAIL))"
echo ""

if [[ $FAIL -gt 0 ]]; then
    exit 1
fi
