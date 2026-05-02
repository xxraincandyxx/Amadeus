"""Memory management — list providers and load entries."""

from __future__ import annotations

from .client import AmadeusClient
from .types import MemoryEntryInfo, MemoryProviderInfo


class MemoryManager:
    """Manage memory providers and entries via the API.

    Usage::

        mem = MemoryManager(client)
        providers = await mem.list_providers()
        entries = await mem.load_entries()
    """

    def __init__(self, client: AmadeusClient) -> None:
        self._client = client

    async def list_providers(self) -> list[MemoryProviderInfo]:
        """List all registered memory providers."""
        resp = await self._client.list_memory_providers()
        return resp.providers

    async def load_entries(self) -> list[MemoryEntryInfo]:
        """Load all memory entries from all providers."""
        resp = await self._client.load_memory_entries()
        return resp.entries

    async def get_entries_by_source(self, source: str) -> list[MemoryEntryInfo]:
        """Filter entries by source (file, session, etc.)."""
        entries = await self.load_entries()
        return [e for e in entries if e.source == source]

    async def store_entry(self, key: str, content: str, source: str = "user") -> dict:
        """Store a memory entry."""
        return await self._client.store_memory_entry(key, content, source)

    async def delete_entry(self, key: str) -> dict:
        """Delete a memory entry by key."""
        return await self._client.delete_memory_entry(key)

    async def build_context_block(self) -> str:
        """Build a context block from all memory entries for inclusion in prompts."""
        entries = await self.load_entries()
        if not entries:
            return ""
        lines = ["## Project Context", ""]
        for entry in entries:
            lines.append(f"### {entry.key}")
            lines.append(entry.content)
            lines.append("")
        return "\n".join(lines)
