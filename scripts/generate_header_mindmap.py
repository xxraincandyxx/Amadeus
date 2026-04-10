#!/usr/bin/env python3
# @amadeus-header
# summary: Generates a Mermaid mindmap from source-file header metadata.
# layer: script
# status: active
# feature_flags:
# - full
# provides:
# - cmd: scripts/generate_header_mindmap.py
# - format: Mermaid mindmap
# uses:
# - cmd: python3
# - artifact: source file headers
# invariants:
# - Mindmap grouping stays derived from committed source-file headers.
# - Missing or malformed headers are reported without crashing by default.
# side_effects:
# - Reads filesystem state.
# - Writes output to stdout or a target file.
# tests:
# - cmd: python3 scripts/generate_header_mindmap.py --stdout
# @end-amadeus-header

from __future__ import annotations

import argparse
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parent.parent
IN_SCOPE_DIRS = ("src", "tests", "examples", "scripts")
EXTRA_FILES = ("verify.sh", "count-code.sh")
EXCLUDED_PREFIXES = ("tests/fixtures/", "refs/")
REQUIRED_FIELDS = (
    "summary",
    "layer",
    "status",
    "feature_flags",
    "provides",
    "uses",
    "invariants",
    "side_effects",
    "tests",
)
LIST_FIELDS = {"feature_flags", "provides", "uses", "invariants", "side_effects", "tests"}
DEFAULT_OUTPUT = ROOT / "docs/header_mindmap.mmd"


def iter_in_scope_files() -> list[Path]:
  files: list[Path] = []
  for directory in IN_SCOPE_DIRS:
    base = ROOT / directory
    if not base.exists():
      continue
    for path in base.rglob("*"):
      if not path.is_file():
        continue
      rel = path.relative_to(ROOT).as_posix()
      if any(rel.startswith(prefix) for prefix in EXCLUDED_PREFIXES):
        continue
      if path.suffix not in {".rs", ".sh", ".py"}:
        continue
      files.append(path)
  for extra in EXTRA_FILES:
    path = ROOT / extra
    if path.exists():
      files.append(path)
  return sorted(set(files))


def comment_prefix(path: Path) -> str:
  return "//" if path.suffix == ".rs" else "#"


def header_block(lines: list[str], prefix: str) -> tuple[int, int] | None:
  start = 1 if lines and lines[0].startswith("#!") else 0
  marker = f"{prefix} @amadeus-header"
  end_marker = f"{prefix} @end-amadeus-header"
  if start >= len(lines) or lines[start] != marker:
    return None
  for idx in range(start + 1, len(lines)):
    if lines[idx] == end_marker:
      return start, idx
  return None


def parse_header(path: Path) -> tuple[dict[str, str | list[str]] | None, list[str]]:
  lines = path.read_text().splitlines()
  prefix = comment_prefix(path)
  block = header_block(lines, prefix)
  if block is None:
    return None, [f"{path.relative_to(ROOT)}: missing header block"]

  start, end = block
  data: dict[str, str | list[str]] = {}
  problems: list[str] = []
  idx = start + 1
  while idx < end:
    line = lines[idx]
    if not line.startswith(f"{prefix} "):
      problems.append(f"{path.relative_to(ROOT)}: malformed header line {idx + 1}")
      idx += 1
      continue
    payload = line[len(prefix) + 1 :]
    if ":" not in payload:
      problems.append(f"{path.relative_to(ROOT)}: malformed header field at line {idx + 1}")
      idx += 1
      continue
    field, value = payload.split(":", 1)
    field = field.strip()
    value = value.strip()
    if field in LIST_FIELDS and value == "":
      items: list[str] = []
      idx += 1
      while idx < end and lines[idx].startswith(f"{prefix} - "):
        items.append(lines[idx][len(prefix) + 3 :].strip())
        idx += 1
      data[field] = items
      continue
    data[field] = value
    idx += 1

  missing = [field for field in REQUIRED_FIELDS if field not in data]
  if missing:
    problems.append(
        f"{path.relative_to(ROOT)}: missing required fields {', '.join(missing)}"
    )
  return data, problems


def sanitize(label: str) -> str:
  cleaned = label.replace('"', "'").replace("`", "'")
  return cleaned.strip()


def list_value(value: str | list[str] | None) -> list[str]:
  if value is None or value == "none":
    return []
  if isinstance(value, list):
    return value
  return [value]


def basename(entry: str) -> str:
  if ": " in entry:
    _, raw = entry.split(": ", 1)
  else:
    raw = entry
  return sanitize(raw)


def render_mermaid(
    records: list[tuple[Path, dict[str, str | list[str]]]],
    max_items: int,
) -> str:
  grouped: dict[str, list[tuple[Path, dict[str, str | list[str]]]]] = {}
  for path, data in records:
    layer = str(data["layer"])
    grouped.setdefault(layer, []).append((path, data))

  lines = [
      "---",
      "title: Amadeus Source Header Mindmap",
      "---",
      "mindmap",
      '  root(("Amadeus Source Map"))',
  ]

  for layer in sorted(grouped):
    lines.append(f'    "{sanitize(layer)}"')
    for path, data in sorted(grouped[layer], key=lambda item: item[0].as_posix()):
      rel = path.relative_to(ROOT).as_posix()
      summary = sanitize(str(data["summary"]))
      status = sanitize(str(data["status"]))
      lines.append(f'      "{rel}"')
      lines.append(f'        "summary: {summary}"')
      lines.append(f'        "status: {status}"')

      flags = list_value(data.get("feature_flags"))[:max_items]
      if flags:
        lines.append('        "feature_flags"')
        for flag in flags:
          lines.append(f'          "{sanitize(flag)}"')

      provides = list_value(data.get("provides"))[:max_items]
      if provides:
        lines.append('        "provides"')
        for item in provides:
          lines.append(f'          "{basename(item)}"')

      uses = list_value(data.get("uses"))[:max_items]
      if uses:
        lines.append('        "uses"')
        for item in uses:
          lines.append(f'          "{basename(item)}"')

  return "\n".join(lines) + "\n"


def parse_args() -> argparse.Namespace:
  parser = argparse.ArgumentParser(
      description="Generate a Mermaid mindmap from source-file headers."
  )
  parser.add_argument(
      "--output",
      type=Path,
      default=DEFAULT_OUTPUT,
      help=f"Target Mermaid file. Defaults to {DEFAULT_OUTPUT.relative_to(ROOT)}",
  )
  parser.add_argument(
      "--stdout",
      action="store_true",
      help="Write the Mermaid document to stdout instead of a file.",
  )
  parser.add_argument(
      "--max-items",
      type=int,
      default=4,
      help="Maximum number of provides, uses, and feature flags to show per file.",
  )
  parser.add_argument(
      "--strict",
      action="store_true",
      help="Fail if any file header is missing or malformed.",
  )
  return parser.parse_args()


def main() -> int:
  args = parse_args()
  records: list[tuple[Path, dict[str, str | list[str]]]] = []
  warnings: list[str] = []

  for path in iter_in_scope_files():
    data, problems = parse_header(path)
    warnings.extend(problems)
    if data is not None:
      records.append((path, data))

  if warnings:
    for warning in warnings:
      print(f"warning: {warning}", file=sys.stderr)
    if args.strict:
      print("mindmap generation aborted due to header warnings", file=sys.stderr)
      return 1

  mermaid = render_mermaid(records, max_items=max(args.max_items, 0))

  if args.stdout:
    sys.stdout.write(mermaid)
  else:
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(mermaid)
    print(f"wrote {args.output.relative_to(ROOT)}")

  if warnings:
    print(
        f"generated mindmap with {len(warnings)} header warning(s)",
        file=sys.stderr,
    )
  return 0


if __name__ == "__main__":
  raise SystemExit(main())
