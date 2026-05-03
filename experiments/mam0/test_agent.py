#!/usr/bin/env python3
"""Functional test for agent.py with OAI-compatible backend (Gemma4).

Validates:
  1. Single-turn chat (no tools)
  2. Tool calling (bash)
  3. File read/write tools
  4. Subagent delegation
  5. Todo workflow
"""

import os
import sys
from pathlib import Path

os.chdir(Path(__file__).resolve().parent)
sys.path.insert(0, ".")


def test_chat():
    """Single-turn chat without tools."""
    from agent import agent_loop

    history = [{"role": "user", "content": "Say 'PASS' and nothing else."}]
    agent_loop(history)
    output = _extract_text(history[-1]["content"])
    assert "PASS" in output.upper(), f"Expected PASS, got: {output}"
    print(f"  [PASS] chat: {output[:100]}")


def test_tool_call():
    """Tool calling via bash."""
    from agent import agent_loop

    history = [{"role": "user", "content": "Run: echo PASS"}]
    agent_loop(history)
    output = _extract_text(history[-1]["content"])
    assert "PASS" in output.upper(), f"Expected PASS, got: {output}"
    print(f"  [PASS] tool_call: {output[:100]}")


def test_file_read():
    """Read a file that exists in the current directory."""
    from agent import agent_loop

    history = [{"role": "user", "content": "Read the file requirements.txt and say what it contains."}]
    agent_loop(history)
    output = _extract_text(history[-1]["content"])
    assert "httpx" in output, f"Expected httpx mention, got: {output}"
    print(f"  [PASS] file_read: {output[:100]}")


def test_file_write_read():
    """Write a file, then verify it exists."""
    from agent import agent_loop
    from agent import run_bash

    history = [{"role": "user", "content": "Write a file test_output.txt containing 'PASS_TEST'."}]
    agent_loop(history)
    result = run_bash("cat test_output.txt 2>/dev/null || echo NOT_FOUND")
    assert "PASS_TEST" in result, f"Expected PASS_TEST in file, got: {result}"
    # Clean up
    Path("test_output.txt").unlink(missing_ok=True)
    print(f"  [PASS] file_write_read: {result[:100]}")


def test_subagent():
    """Subagent delegation (Explore type)."""
    from agent import run_subagent

    result = run_subagent(
        "Use bash tool to run: ls *.py. Then return the exact filenames you see.",
        agent_type="Explore",
    )
    assert "agent.py" in result, f"Expected agent.py in subagent result, got: {result}"
    print(f"  [PASS] subagent: {result[:100]}")


def test_todo():
    """TodoWrite tool updates todos correctly."""
    from agent import agent_loop

    history = [{"role": "user", "content": "Use TodoWrite to track two tasks: 'task A' pending and 'task B' completed. Just set them and say DONE."}]
    agent_loop(history)
    output = _extract_text(history[-1]["content"])
    assert "DONE" in output.upper(), f"Expected DONE, got: {output}"
    print(f"  [PASS] todo: {output[:100]}")


def _extract_text(content) -> str:
    if isinstance(content, str):
        return content
    if isinstance(content, list):
        parts = []
        for b in content:
            if hasattr(b, "text"):
                parts.append(b.text)
            elif isinstance(b, dict) and b.get("type") == "text":
                parts.append(b.get("text", ""))
        return " ".join(parts)
    return str(content)


if __name__ == "__main__":
    print("=== agent.py Functional Tests (OAI/Gemma4) ===")
    tests = [
        ("chat", test_chat),
        ("tool_call", test_tool_call),
        ("file_read", test_file_read),
        ("file_write_read", test_file_write_read),
        ("subagent", test_subagent),
        ("todo", test_todo),
    ]
    passed = 0
    for name, fn in tests:
        try:
            fn()
            passed += 1
        except Exception as e:
            print(f"  [FAIL] {name}: {e}")
    print(f"\n=== {passed}/{len(tests)} tests passed ===")
