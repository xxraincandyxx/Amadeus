#!/bin/bash

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
