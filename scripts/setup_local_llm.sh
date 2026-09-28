#!/usr/bin/env bash
# @amadeus-header
# summary: Bootstrap a local OpenAI-compatible 0.5B model server for development.
# layer: script
# status: active
# feature_flags: none
# provides:
# - cmd: scripts/setup_local_llm.sh
# uses:
# - cmd: curl
# - cmd: llama-server
# invariants:
# - Script command flow remains non-interactive and order-dependent.
# - The served endpoint stays OpenAI-compatible on 127.0.0.1:8123 by default.
# side_effects:
# - Downloads model files into .amadeus/models.
# - Runs external commands or subprocesses.
# - Writes output to stdout or stderr.
# tests:
# - cmd: bash -n scripts/setup_local_llm.sh
# @end-amadeus-header

set -euo pipefail

# Downloads Qwen2.5-0.5B-Instruct (GGUF) and serves it through llama.cpp's
# OpenAI-compatible server so Amadeus runs without any cloud API key.

MODEL_NAME="${AMADEUS_LOCAL_MODEL_NAME:-qwen2.5-0.5b-instruct}"
MODEL_URL="${AMADEUS_LOCAL_MODEL_URL:-https://huggingface.co/Qwen/Qwen2.5-0.5B-Instruct-GGUF/resolve/main/qwen2.5-0.5b-instruct-q4_k_m.gguf}"
MODEL_FILE="${AMADEUS_LOCAL_MODEL_FILE:-$MODEL_NAME-q4_k_m.gguf}"
HOST="${AMADEUS_LOCAL_HOST:-127.0.0.1}"
PORT="${AMADEUS_LOCAL_PORT:-8123}"
CTX="${AMADEUS_LOCAL_CTX:-8192}"
LLAMA_SERVER_BIN="${LLAMA_SERVER:-llama-server}"

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MODEL_DIR="$REPO_ROOT/.amadeus/models"
MODEL_PATH="$MODEL_DIR/$MODEL_FILE"

usage() {
  cat <<'EOF'
Usage: scripts/setup_local_llm.sh [download|serve|all]

Commands:
  download   Fetch the model file (resumable, skipped when present)
  serve      Start the OpenAI-compatible server
  all        Download (if needed), then serve (default)

Environment overrides:
  AMADEUS_LOCAL_MODEL_NAME   Model id reported to clients (default qwen2.5-0.5b-instruct)
  AMADEUS_LOCAL_MODEL_URL    Download URL for the GGUF file
  AMADEUS_LOCAL_MODEL_FILE   File name under .amadeus/models
  AMADEUS_LOCAL_HOST         Bind address (default 127.0.0.1)
  AMADEUS_LOCAL_PORT         Port (default 8123)
  AMADEUS_LOCAL_CTX          Context size (default 8192)
  LLAMA_SERVER               Path to the llama-server binary
EOF
}

require_llama_server() {
  if command -v "$LLAMA_SERVER_BIN" >/dev/null 2>&1; then
    return
  fi
  echo "error: llama-server not found (looked for: $LLAMA_SERVER_BIN)." >&2
  echo "  macOS:           brew install llama.cpp" >&2
  echo "  Linux/other:     build llama.cpp and put llama-server on PATH," >&2
  echo "                   or point LLAMA_SERVER at the binary" >&2
  exit 1
}

download_model() {
  if [ -f "$MODEL_PATH" ] && [ -s "$MODEL_PATH" ]; then
    echo "Model already present: $MODEL_PATH"
    return
  fi
  mkdir -p "$MODEL_DIR"
  echo "Downloading $MODEL_URL"
  echo "  -> $MODEL_PATH"
  curl -L --fail --retry 3 --continue-at - -o "$MODEL_PATH.part" "$MODEL_URL"
  mv "$MODEL_PATH.part" "$MODEL_PATH"
  echo "Downloaded $(du -h "$MODEL_PATH" | cut -f1 | tr -d ' ') -> $MODEL_PATH"
}

serve_model() {
  require_llama_server
  if [ ! -f "$MODEL_PATH" ]; then
    echo "error: model file missing: $MODEL_PATH (run 'download' first)" >&2
    exit 1
  fi

  cat <<EOF

Starting $MODEL_NAME on http://$HOST:$PORT (Ctrl-C to stop)

Point Amadeus at it via .amadeus/settings.json:

  {
    "provider": "openai",
    "api_key": "local",
    "base_url": "http://$HOST:$PORT/v1",
    "model": "$MODEL_NAME"
  }

EOF

  exec "$LLAMA_SERVER_BIN" \
    -m "$MODEL_PATH" \
    --alias "$MODEL_NAME" \
    --host "$HOST" \
    --port "$PORT" \
    --ctx-size "$CTX"
}

case "${1:-all}" in
  download) download_model ;;
  serve) serve_model ;;
  all)
    download_model
    serve_model
    ;;
  -h | --help | help)
    usage
    ;;
  *)
    usage >&2
    exit 1
    ;;
esac
