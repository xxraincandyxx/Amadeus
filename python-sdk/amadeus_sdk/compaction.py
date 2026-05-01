"""Compaction configuration management."""

from __future__ import annotations

from typing import Union

from .client import AmadeusClient
from .types import CompactionConfig, CompactionTriggers


class CompactionManager:
    """Manage compaction configuration via the API.

    Usage::

        cm = CompactionManager(client)
        config = await cm.get_config()
        await cm.set_threshold(80)
        await cm.set_min_messages(6)
    """

    def __init__(self, client: AmadeusClient) -> None:
        self._client = client

    async def get_config(self) -> CompactionConfig:
        """Get the current compaction configuration."""
        return await self._client.get_compaction_config()

    async def update(self, **kwargs: Union[bool, int, str, None]) -> CompactionConfig:
        """Update compaction settings and return the new config."""
        return await self._client.update_compaction_config(**kwargs)

    async def set_threshold(self, percent: int) -> CompactionConfig:
        """Set the compaction trigger threshold (0-100)."""
        return await self._client.update_compaction_config(threshold_percent=percent)

    async def set_target(self, percent: int) -> CompactionConfig:
        """Set the post-compaction target usage percent."""
        return await self._client.update_compaction_config(target_percent=percent)

    async def set_min_messages(self, count: int) -> CompactionConfig:
        """Set the minimum messages before compaction can trigger."""
        return await self._client.update_compaction_config(min_messages=count)

    async def toggle_auto_compact(self, enabled: bool) -> CompactionConfig:
        """Enable or disable automatic compaction."""
        return await self._client.update_compaction_config(auto_compact=enabled)

    async def get_triggers(self) -> CompactionTriggers:
        """List available compaction triggers."""
        return await self._client.get_compaction_triggers()
