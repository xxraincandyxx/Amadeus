#!/bin/bash
# @amadeus-header
# summary: Runnable example for client rest usage.
# layer: example
# status: experimental
# feature_flags:
# - full
# provides:
# - cmd: examples/client_rest.sh
# uses:
# - cmd: curl
# - cmd: jq
# invariants:
# - Example code remains runnable against the current public API.
# side_effects:
# - Performs network or HTTP operations.
# - Runs external commands or subprocesses.
# - Writes output to stdout or stderr.
# tests:
# - cmd: bash ./examples/client_rest.sh
# @end-amadeus-header


# Amadeus REST API Client Example
# Demonstrates how to use the Amadeus SDK via REST endpoints

API_URL="http://localhost:3000"

echo "🔍 1. Health Check"
curl -s "$API_URL/health" | jq .
echo -e "
"

echo "💬 2. Simple Chat (via Supervisor)"
curl -s -X POST "$API_URL/chat" 
  -H "Content-Type: application/json" 
  -d '{"message": "Say hello world"}' | jq .
echo -e "
"

echo "💻 3. Direct Command Execution"
curl -s -X POST "$API_URL/execute" 
  -H "Content-Type: application/json" 
  -d '{"command": "ls -F"}' | jq .
echo -e "
"

echo "👥 4. Multi-Agent Task Dispatch"
curl -s -X POST "$API_URL/tasks" 
  -H "Content-Type: application/json" 
  -d '{
    "id": "example-task-1",
    "prompt": "Create a temporary file called test.txt with content Hello API",
    "capabilities": ["bash"]
  }' | jq .
echo -e "
"

echo "🌊 5. Event Stream (SSE)"
echo "Streaming starting (press Ctrl+C to stop)..."
curl -N "$API_URL/stream?message=Summarize+this+project"
