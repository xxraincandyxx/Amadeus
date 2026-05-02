"""Amadeus Python SDK.

Provides async HTTP client, agent loop, and utilities for the Amadeus AI agent framework.
"""

from .agent import Agent, AgentTurn
from .client import AmadeusClient, AmadeusError
from .compaction import CompactionManager
from .memory import MemoryManager
from .memory_agent import MemoryAgent
from .prompts import PromptBuilder
from .tools import ToolRegistry
from .ua_debug import UADebugRecorder

__all__ = [
    "Agent",
    "AgentTurn",
    "AmadeusClient",
    "AmadeusError",
    "CompactionManager",
    "MemoryAgent",
    "MemoryManager",
    "PromptBuilder",
    "ToolRegistry",
    "UADebugRecorder",
]
