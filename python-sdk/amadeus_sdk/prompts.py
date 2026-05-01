"""Prompt builder for composing system prompts."""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Optional

from .client import AmadeusClient
from .types import BuildPromptResponse, PromptSectionInfo, PromptSectionInput


@dataclass
class PromptBuilder:
    """Build custom system prompts via the API.

    Usage::

        builder = PromptBuilder(client)
        builder.add_section("custom_role", "You are a Python expert.", priority=50)
        prompt = await builder.build()
    """

    _client: AmadeusClient
    _workdir: Optional[str] = None
    _include_sub_agent: bool = True
    _extra_sections: list[PromptSectionInput] = field(default_factory=list)

    def set_workdir(self, workdir: str) -> "PromptBuilder":
        self._workdir = workdir
        return self

    def include_sub_agent_tool(self, include: bool) -> "PromptBuilder":
        self._include_sub_agent = include
        return self

    def add_section(self, id: str, content: str, priority: int = 100) -> "PromptBuilder":
        self._extra_sections.append(PromptSectionInput(id=id, content=content, priority=priority))
        return self

    def clear_extra_sections(self) -> "PromptBuilder":
        self._extra_sections.clear()
        return self

    async def list_current_sections(self) -> list[PromptSectionInfo]:
        """Fetch the default sections currently configured on the server."""
        resp = await self._client.list_prompt_sections()
        return resp.sections

    async def build(self) -> BuildPromptResponse:
        """Build and return the system prompt."""
        return await self._client.build_prompt(
            workdir=self._workdir,
            include_sub_agent=self._include_sub_agent,
            extra_sections=self._extra_sections if self._extra_sections else None,
        )
