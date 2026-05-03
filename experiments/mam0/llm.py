"""OpenAI-compatible LLM client with Anthropic SDK interface.

Replaces the anthropic.Anthropic client so agent.py can work with any
OpenAI-compatible API (vLLM, OpenRouter, etc.) without code changes.
"""

from __future__ import annotations

import json
from typing import Optional, Union

import httpx


# ---------------------------------------------------------------------------
# Response types (mimic anthropic SDK shapes)
# ---------------------------------------------------------------------------

class _TextBlock:
    """Text content block — only has .type and .text (so hasattr(block, 'text') works)."""

    __slots__ = ("type", "text")

    def __init__(self, text: str):
        self.type = "text"
        self.text = text

    def __repr__(self) -> str:
        return f"TextBlock(text={self.text!r})"


class _ToolUseBlock:
    """Tool-use content block — has .id, .name, .input but NOT .text."""

    __slots__ = ("type", "id", "name", "input")

    def __init__(self, id: str, name: str, input: dict):
        self.type = "tool_use"
        self.id = id
        self.name = name
        self.input = input

    def __repr__(self) -> str:
        return f"ToolUseBlock(id={self.id!r}, name={self.name!r})"


# Union type for type hints
ContentBlock = Union[_TextBlock, _ToolUseBlock]


class Response:
    """Mimics anthropic.types.Message."""

    __slots__ = ("content", "stop_reason")

    def __init__(self, content: list[ContentBlock], stop_reason: str):
        self.content = content
        self.stop_reason = stop_reason


# ---------------------------------------------------------------------------
# Format conversion helpers
# ---------------------------------------------------------------------------

def _convert_tools(tools: list[dict]) -> list[dict]:
    """Convert Anthropic tool definitions to OpenAI function format.

    Anthropic: {name, description, input_schema}
    OpenAI:    {type: "function", function: {name, description, parameters}}
    """
    converted = []
    for tool in tools:
        params = tool.get("parameters") or tool.get("input_schema") or {}
        converted.append({
            "type": "function",
            "function": {
                "name": tool["name"],
                "description": tool.get("description", ""),
                "parameters": params,
            },
        })
    return converted


def _convert_messages(system: str, messages: list[dict]) -> list[dict]:
    """Convert Anthropic-format messages to OpenAI chat format.

    Key differences:
      - System prompt is a message with role="system"
      - Tool results are separate messages with role="tool"
      - Tool calls go in a tool_calls array on assistant messages
    """
    converted: list[dict] = []

    if system:
        converted.append({"role": "system", "content": system})

    for msg in messages:
        role = msg.get("role", "user")
        content = msg.get("content")

        # Content can be a string (simple text) or a list of content blocks
        if isinstance(content, str):
            converted.append({"role": role, "content": content})
            continue

        if not isinstance(content, list):
            converted.append({"role": role, "content": str(content)})
            continue

        # Separate tool_results from text/tool_use blocks
        tool_results: list[dict] = []
        text_parts: list[str] = []
        tool_uses: list[dict] = []

        for block in content:
            if isinstance(block, dict):
                block_type = block.get("type", "")
            elif isinstance(block, (_TextBlock, _ToolUseBlock)):
                block_type = block.type
            else:
                text_parts.append(str(block))
                continue

            if block_type == "tool_result":
                tid = block.get("tool_use_id", "") if isinstance(block, dict) else block.id
                c = block.get("content", "") if isinstance(block, dict) else block.content
                tool_results.append({"tool_call_id": tid, "content": c})
            elif block_type == "tool_use":
                if isinstance(block, dict):
                    inp = block.get("input", {})
                    if not isinstance(inp, str):
                        inp = json.dumps(inp, ensure_ascii=False)
                    tool_uses.append({
                        "id": block.get("id", ""),
                        "type": "function",
                        "function": {
                            "name": block.get("name", ""),
                            "arguments": inp,
                        },
                    })
                else:
                    inp = block.input or {}
                    if not isinstance(inp, str):
                        inp = json.dumps(inp, ensure_ascii=False)
                    tool_uses.append({
                        "id": block.id,
                        "type": "function",
                        "function": {
                            "name": block.name,
                            "arguments": inp,
                        },
                    })
            elif block_type == "text":
                t = block.get("text", "") if isinstance(block, dict) else block.text
                if t:
                    text_parts.append(t)

        # Emit tool_result messages first (OpenAI requires tool before assistant)
        for tr in tool_results:
            converted.append({"role": "tool", "tool_call_id": tr["tool_call_id"], "content": tr["content"]})

        # Emit the message itself
        if role == "assistant" and tool_uses:
            converted.append({
                "role": "assistant",
                "content": "".join(text_parts) or None,
                "tool_calls": tool_uses,
            })
        else:
            converted.append({"role": role, "content": "".join(text_parts)})

    return converted


def _parse_response(json_data: dict) -> Response:
    """Parse an OpenAI chat completion response into ContentBlocks + stop_reason."""
    choices = json_data.get("choices", [])
    if not choices:
        return Response(content=[], stop_reason="end_turn")

    choice = choices[0]
    finish_reason = choice.get("finish_reason", "") or ""

    # Map OpenAI finish_reason to Anthropic stop_reason
    stop_map = {
        "tool_calls": "tool_use",
        "stop": "end_turn",
        "length": "max_tokens",
    }
    stop_reason = stop_map.get(finish_reason, finish_reason or "end_turn")

    message = choice.get("message", {})
    blocks: list[ContentBlock] = []

    # Text content
    text_content = message.get("content")
    if isinstance(text_content, str) and text_content:
        blocks.append(_TextBlock(text=text_content))
    elif isinstance(text_content, list):
        for part in text_content:
            if isinstance(part, dict):
                if part.get("type") == "text":
                    blocks.append(_TextBlock(text=part.get("text", "")))
                elif part.get("type") == "tool_use":
                    blocks.append(_ToolUseBlock(
                        id=part.get("id", ""),
                        name=part.get("name", ""),
                        input=part.get("input", {}),
                    ))

    # Tool calls (older OpenAI format)
    tool_calls = message.get("tool_calls") or []
    for tc in tool_calls:
        func = tc.get("function", {})
        args_str = func.get("arguments", "{}")
        if isinstance(args_str, str):
            try:
                args = json.loads(args_str)
            except (json.JSONDecodeError, TypeError):
                args = {}
        else:
            args = args_str
        blocks.append(_ToolUseBlock(
            id=tc.get("id", ""),
            name=func.get("name", ""),
            input=args,
        ))

    return Response(content=blocks, stop_reason=stop_reason)


# ---------------------------------------------------------------------------
# Client (drop-in replacement for anthropic.Anthropic)
# ---------------------------------------------------------------------------

class Messages:
    """Mimics anthropic.Anthropic().messages — exposes .create()."""

    def __init__(self, client: "LLMClient"):
        self._client = client

    def create(
        self,
        model: str,
        messages: list[dict],
        max_tokens: int = 8000,
        system: str = "",
        tools: Optional[list[dict]] = None,
    ) -> Response:
        return self._client._create_message(
            model=model,
            messages=messages,
            max_tokens=max_tokens,
            system=system,
            tools=tools or [],
        )


class LLMClient:
    """OpenAI-compatible HTTP client with an Anthropic SDK-shaped interface.

    Usage::

        client = LLMClient(
            base_url="http://localhost:8080/v1",
            api_key="EMPTY",
        )
        resp = client.messages.create(
            model="gemma-4-26b-a4b-it-fp8",
            messages=[{"role": "user", "content": "Hello"}],
            max_tokens=1024,
            system="You are helpful.",
            tools=[{"name": "bash", "description": "...", "input_schema": {...}}],
        )
        for block in resp.content:
            print(block.type, block.text)
    """

    def __init__(
        self,
        base_url: str = "",
        api_key: str = "",
        timeout: float = 120.0,
    ):
        self.base_url = (base_url or "http://118.31.102.225:1112/v1").rstrip("/")
        self.api_key = api_key or "EMPTY"
        self.timeout = timeout
        self.messages = Messages(self)
        self._http = httpx.Client(timeout=timeout, trust_env=False)

    @property
    def _chat_url(self) -> str:
        base = self.base_url
        if base.endswith("/chat/completions"):
            return base
        if any(base.endswith(v) for v in ["/v1", "/v2", "/v3", "/v4"]):
            return f"{base}/chat/completions"
        return f"{base}/v1/chat/completions"

    def _create_message(
        self,
        model: str,
        messages: list[dict],
        max_tokens: int = 8000,
        system: str = "",
        tools: Optional[list[dict]] = None,
    ) -> Response:
        openai_messages = _convert_messages(system, messages)
        openai_tools = _convert_tools(tools or [])

        body: dict = {
            "model": model,
            "messages": openai_messages,
            "max_tokens": max_tokens,
        }
        if openai_tools:
            body["tools"] = openai_tools

        headers = {
            "Content-Type": "application/json",
        }
        if self.api_key and self.api_key != "EMPTY":
            headers["Authorization"] = f"Bearer {self.api_key}"

        resp = self._http.post(
            self._chat_url,
            json=body,
            headers=headers,
        )
        resp.raise_for_status()
        return _parse_response(resp.json())
