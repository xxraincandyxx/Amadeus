#!/usr/bin/env bash
# @amadeus-header
# summary: Bootstrap a local OpenAI-compatible GGUF model server for development.
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

# Serves any GGUF already present under .amadeus/models; when none is found,
# downloads Qwen2.5-0.5B-Instruct and serves it through llama.cpp's
# OpenAI-compatible server so Amadeus runs without any cloud API key.

HOST="${AMADEUS_LOCAL_HOST:-127.0.0.1}"
PORT="${AMADEUS_LOCAL_PORT:-8123}"
CTX="${AMADEUS_LOCAL_CTX:-8192}"
LLAMA_SERVER_BIN="${LLAMA_SERVER:-llama-server}"
# Empty string disables the flag entirely; "0" disables thinking; -1 (default)
# lets reasoning models think freely so the client can show the live process.
REASONING_BUDGET="${AMADEUS_LOCAL_REASONING_BUDGET:--1}"

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MODEL_DIR="$REPO_ROOT/.amadeus/models"

DEFAULT_MODEL_URL="https://huggingface.co/Qwen/Qwen2.5-0.5B-Instruct-GGUF/resolve/main/qwen2.5-0.5b-instruct-q4_k_m.gguf"
DEFAULT_MODEL_FILE="qwen2.5-0.5b-instruct-q4_k_m.gguf"

MODEL_FILE="${AMADEUS_LOCAL_MODEL_FILE:-}"
MODEL_URL="${AMADEUS_LOCAL_MODEL_URL:-$DEFAULT_MODEL_URL}"
MODEL_NAME="${AMADEUS_LOCAL_MODEL_NAME:-}"
MODEL_PATH=""

usage() {
  cat <<'EOF'
Usage: scripts/setup_local_llm.sh [download|serve|all]

Commands:
  download   Fetch the default model file (resumable, skipped when present)
  serve      Start the OpenAI-compatible server
  all        Download (if needed), then serve (default)

The first *.gguf found in .amadeus/models is served as-is; the download
default only kicks in when the directory holds none.

Environment overrides:
  AMADEUS_LOCAL_MODEL_FILE   Explicit model file name inside .amadeus/models
  AMADEUS_LOCAL_MODEL_NAME   Model id reported to clients (default: file stem)
  AMADEUS_LOCAL_MODEL_URL    Download URL for the default GGUF file
  AMADEUS_LOCAL_HOST         Bind address (default 127.0.0.1)
  AMADEUS_LOCAL_PORT         Port (default 8123)
  AMADEUS_LOCAL_CTX          Context size (default 8192)
  LLAMA_SERVER               Path to the llama-server binary
  AMADEUS_LOCAL_REASONING_BUDGET  llama-server --reasoning-budget value
                             (default -1, thinking enabled; 0 disables it)
  HF_ENDPOINT                Hugging Face endpoint for the mirror fallback
                             (default https://hf-mirror.com)
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

resolve_model() {
  if [ -n "$MODEL_FILE" ]; then
    MODEL_PATH="$MODEL_DIR/$MODEL_FILE"
  else
    local found
    found="$(find "$MODEL_DIR" -maxdepth 1 -name '*.gguf' 2>/dev/null | sort | head -1 || true)"
    if [ -n "$found" ]; then
      MODEL_PATH="$found"
    else
      MODEL_PATH="$MODEL_DIR/$DEFAULT_MODEL_FILE"
    fi
  fi
  if [ -z "$MODEL_NAME" ]; then
    MODEL_NAME="$(basename "$MODEL_PATH")"
    MODEL_NAME="${MODEL_NAME%.gguf}"
  fi
}

has_model_file() {
  [ -f "$MODEL_PATH" ] && [ -s "$MODEL_PATH" ]
}

download_file() {
  curl -L --fail --retry 3 --continue-at - -o "$MODEL_PATH.part" "$1"
}

download_model() {
  resolve_model
  if has_model_file; then
    echo "Model already present: $MODEL_PATH"
    return
  fi
  if [ "$MODEL_PATH" != "$MODEL_DIR/$DEFAULT_MODEL_FILE" ]; then
    echo "error: model file missing: $MODEL_PATH" >&2
    exit 1
  fi
  mkdir -p "$MODEL_DIR"
  echo "Downloading $MODEL_URL"
  echo "  -> $MODEL_PATH"
  if ! download_file "$MODEL_URL"; then
    # huggingface.co is unreachable from some networks; retry once via mirror.
    local mirror_url="${MODEL_URL/https:\/\/huggingface.co/${HF_ENDPOINT:-https://hf-mirror.com}}"
    if [ "$mirror_url" = "$MODEL_URL" ]; then
      echo "error: download failed (custom model URL, no mirror fallback)" >&2
      exit 1
    fi
    echo "Primary download failed; retrying via mirror: $mirror_url" >&2
    download_file "$mirror_url"
  fi
  mv "$MODEL_PATH.part" "$MODEL_PATH"
  echo "Downloaded $(du -h "$MODEL_PATH" | cut -f1 | tr -d ' ') -> $MODEL_PATH"
}

serve_model() {
  require_llama_server
  resolve_model
  if ! has_model_file; then
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
    --ctx-size "$CTX" \
    ${REASONING_BUDGET:+--reasoning-budget "$REASONING_BUDGET"}
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
