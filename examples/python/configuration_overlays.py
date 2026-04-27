# @amadeus-header
# summary: Python helper that writes an example Amadeus settings file with prompt and tool overlays.
# layer: example
# status: experimental
# feature_flags:
# - full
# provides:
# - cmd: examples/python/configuration_overlays.py
# uses:
# - artifact: .amadeus/settings.json
# invariants:
# - Example output uses conservative prompt and tool overlays only.
# side_effects:
# - Writes .amadeus/settings.example.json by default.
# tests:
# - cmd: python3 examples/python/configuration_overlays.py --help
# @end-amadeus-header

from __future__ import annotations

import argparse
import json
from pathlib import Path


EXAMPLE_SETTINGS = {
    "prompts": {
        "active_profile": "team",
        "profiles": {
            "team": {
                "mode": "append",
                "include_project_context": True,
                "sections": [
                    {
                        "id": "team-style",
                        "title": "Team Style",
                        "content": "Prefer concise implementation notes and call out security-sensitive changes.",
                    }
                ],
                "files": ["prompts/team.md"],
            }
        },
    },
    "tools": {
        "default_profile": "workspace",
        "subagent_profile": "subagent",
        "profiles": {
            "workspace": {
                "enabled_packs": ["filesystem", "search", "planning", "web", "shell"],
                "disabled_tools": [],
                "allow_aliases": True,
                "include_mcp": True,
                "include_control_plane": True,
                "model_permission_mode": "workspace-write",
            },
            "subagent": {
                "enabled_packs": ["filesystem", "search", "planning"],
                "disabled_tools": ["bash", "write_file", "edit_file"],
                "allow_aliases": True,
                "include_mcp": False,
                "include_control_plane": False,
                "model_permission_mode": "read-only",
            },
        },
        "overrides": {
            "bash": {
                "description": "Run approved shell commands in the workspace.",
                "aliases": ["shell"],
                "prompt_approval": True,
                "visible_in_modes": ["workspace-write", "danger-full-access"],
                "required_permission": "workspace-write",
            }
        },
    },
}


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Write an example .amadeus settings file with prompt/tool overlays."
    )
    parser.add_argument(
        "--output",
        default=".amadeus/settings.example.json",
        help="Path to write; use .amadeus/settings.json to activate it.",
    )
    args = parser.parse_args()

    output = Path(args.output)
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(EXAMPLE_SETTINGS, indent=2) + "\n", encoding="utf-8")
    print(f"Wrote {output}")


if __name__ == "__main__":
    main()
