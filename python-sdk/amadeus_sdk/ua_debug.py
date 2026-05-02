"""UA (User-Assistant) Debug JSON file system for recording and replaying conversations."""

from __future__ import annotations

import json
from datetime import datetime, timezone
from pathlib import Path
from typing import Optional

from .types import (
    MemoryEntryInfo,
    UADebugAssistant,
    UADebugLog,
    UADebugTurn,
)


class UADebugRecorder:
    """Records and replays User-Assistant dialogue in a readable JSON format.

    Two use cases:
    1. Record — after a conversation, dump the full UA dialogue for inspection
    2. Load — create/load a file to mimic conversation history for debug/test
    """

    def __init__(self) -> None:
        self._turns: list[UADebugTurn] = []
        self._model: str = ""
        self._system_prompt: str = ""
        self._memory_snapshot: list[dict] = []

    @property
    def turns(self) -> list[UADebugTurn]:
        return list(self._turns)

    @property
    def model(self) -> str:
        return self._model

    @model.setter
    def model(self, value: str) -> None:
        self._model = value

    @property
    def system_prompt(self) -> str:
        return self._system_prompt

    @system_prompt.setter
    def system_prompt(self, value: str) -> None:
        self._system_prompt = value

    def record_turn(
        self,
        user_msg: str,
        assistant_text: str,
        tool_calls: list[dict],
        stop_reason: str = "end_turn",
    ) -> None:
        """Record one complete turn."""
        from .types import UADebugAssistant, UADebugToolCall

        tc_objs = [UADebugToolCall(**tc) for tc in tool_calls]
        turn = UADebugTurn(
            index=len(self._turns),
            user=user_msg,
            assistant=UADebugAssistant(
                text=assistant_text,
                tool_calls=tc_objs,
                stop_reason=stop_reason,
            ),
        )
        self._turns.append(turn)

    def capture_memory_snapshot(self, entries: list[MemoryEntryInfo]) -> None:
        """Snapshot current memory state for the debug log."""
        self._memory_snapshot = [
            {"key": e.key, "content": e.content, "source": e.source}
            for e in entries
        ]

    def save(self, path: str) -> None:
        """Write the full UA debug log to a JSON file."""
        log = UADebugLog(
            version="1",
            timestamp=datetime.now(timezone.utc).isoformat(),
            model=self._model,
            system_prompt=self._system_prompt,
            memory_snapshot=list(self._memory_snapshot),
            turns=list(self._turns),
            stats={
                "turns": len(self._turns),
                "tool_calls": sum(
                    len(t.assistant.tool_calls) for t in self._turns
                ),
            },
        )

        def serialize(obj: object) -> dict:
            """Convert a dataclass to a JSON-safe dict, dropping None values."""
            if hasattr(obj, "__dataclass_fields__"):
                result = {}
                for f in obj.__dataclass_fields__:
                    val = getattr(obj, f)
                    if val is None:
                        continue
                    if isinstance(val, list):
                        result[f] = [serialize(v) if hasattr(v, "__dataclass_fields__") else v for v in val]
                    elif hasattr(val, "__dataclass_fields__"):
                        result[f] = serialize(val)
                    else:
                        result[f] = val
                return result
            return obj  # type: ignore[return-value]

        Path(path).write_text(json.dumps(serialize(log), indent=2, ensure_ascii=False))

    @staticmethod
    def load(path: str) -> UADebugLog:
        """Load a UA debug JSON file. Returns a structured log for inspection."""
        data = json.loads(Path(path).read_text())

        from .types import UADebugToolCall, UADebugAssistant

        turns = []
        for t in data.get("turns", []):
            tc_objs = [UADebugToolCall(**tc) for tc in t["assistant"].get("tool_calls", [])]
            turns.append(UADebugTurn(
                index=t["index"],
                user=t["user"],
                assistant=UADebugAssistant(
                    text=t["assistant"]["text"],
                    tool_calls=tc_objs,
                    stop_reason=t["assistant"].get("stop_reason", "end_turn"),
                ),
            ))

        return UADebugLog(
            version=data.get("version", "1"),
            timestamp=data.get("timestamp", ""),
            model=data.get("model", ""),
            system_prompt=data.get("system_prompt", ""),
            memory_snapshot=data.get("memory_snapshot", []),
            turns=turns,
            stats=data.get("stats", {}),
        )

    @staticmethod
    def load_as_messages(path: str) -> list[dict]:
        """Load a UA debug JSON file and extract user/assistant messages.

        Returns a list of {"role": "user"|"assistant", "content": str} dicts
        suitable for seeding conversation history or test fixtures.
        """
        data = json.loads(Path(path).read_text())
        messages: list[dict] = []
        for t in data.get("turns", []):
            messages.append({"role": "user", "content": t["user"]})
            messages.append({"role": "assistant", "content": t["assistant"]["text"]})
        return messages
