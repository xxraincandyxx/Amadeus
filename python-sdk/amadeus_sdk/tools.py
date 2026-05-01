"""Tool catalog inspection."""

from __future__ import annotations

from typing import Optional

from .client import AmadeusClient
from .types import ToolCatalogEntry


class ToolRegistry:
    """Inspect available tools via the API.

    Usage::

        registry = ToolRegistry(client)
        tools = await registry.list_tools()
        bash_tool = await registry.get_tool("bash")
    """

    def __init__(self, client: AmadeusClient) -> None:
        self._client = client

    async def list_tools(self) -> list[ToolCatalogEntry]:
        """Return all tools in the catalog."""
        resp = await self._client.get_tool_catalog()
        return resp.tools

    async def get_tool(self, name: str) -> Optional[ToolCatalogEntry]:
        """Find a tool by name."""
        tools = await self.list_tools()
        for t in tools:
            if t.name == name:
                return t
        return None

    async def tool_names(self) -> list[str]:
        """Return all tool names."""
        tools = await self.list_tools()
        return [t.name for t in tools]

    async def tools_by_permission(self, mode: str) -> list[ToolCatalogEntry]:
        """Filter tools by permission mode (auto/ask/strict)."""
        tools = await self.list_tools()
        return [t for t in tools if t.permission_mode == mode]
