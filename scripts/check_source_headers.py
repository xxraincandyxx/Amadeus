#!/usr/bin/env python3
# @amadeus-header
# summary: Validates mandatory source-file headers across in-scope repository code.
# layer: script
# status: active
# feature_flags:
# - full
# provides:
# - cmd: scripts/check_source_headers.py
# uses:
# - cmd: python3
# - artifact: source file headers
# invariants:
# - Validation scope stays aligned with docs/SOURCE_FILE_HEADERS.md.
# - Required header fields and interface-kind rules stay enforced consistently.
# side_effects:
# - Reads filesystem state.
# - Writes output to stdout or stderr.
# tests:
# - cmd: python3 scripts/check_source_headers.py
# @end-amadeus-header

from __future__ import annotations

import re
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parent.parent
REQUIRED_FIELDS = [
    "summary",
    "layer",
    "status",
    "feature_flags",
    "provides",
    "uses",
    "invariants",
    "side_effects",
    "tests",
]
LIST_FIELDS = {"feature_flags", "provides", "uses", "invariants", "side_effects", "tests"}
INTERFACE_KINDS = {
    "module",
    "type",
    "trait",
    "fn",
    "const",
    "tool",
    "route",
    "event",
    "cmd",
    "format",
    "artifact",
    "env",
    "protocol",
    "runtime",
}
BANNED_PATTERN = re.compile(r"\b(TBD|TODO|etc|misc)\b")
IN_SCOPE_DIRS = ("src", "tests", "examples", "scripts")
EXTRA_FILES = ("verify.sh", "count-code.sh")
EXCLUDED_PREFIXES = ("tests/fixtures/", "refs/")


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


def read_lines(path: Path) -> list[str]:
  return path.read_text().splitlines()


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


def parse_header(path: Path, lines: list[str], prefix: str, start: int, end: int) -> tuple[dict[str, str | list[str]], list[str]]:
  problems: list[str] = []
  data: dict[str, str | list[str]] = {}
  order: list[str] = []
  idx = start + 1
  while idx < end:
    line = lines[idx]
    if not line.startswith(f"{prefix} "):
      problems.append(f"{path}: header line {idx + 1} has wrong comment prefix")
      idx += 1
      continue
    payload = line[len(prefix) + 1 :]
    if payload.startswith("- "):
      problems.append(f"{path}: unexpected list item without field at line {idx + 1}")
      idx += 1
      continue
    if ":" not in payload:
      problems.append(f"{path}: malformed header field at line {idx + 1}")
      idx += 1
      continue
    field, value = payload.split(":", 1)
    field = field.strip()
    value = value.strip()
    order.append(field)
    if field in LIST_FIELDS and value == "":
      items: list[str] = []
      idx += 1
      while idx < end:
        next_line = lines[idx]
        if next_line.startswith(f"{prefix} - "):
          items.append(next_line[len(prefix) + 3 :].strip())
          idx += 1
          continue
        break
      data[field] = items
      continue
    data[field] = value
    idx += 1
  if order != REQUIRED_FIELDS:
    problems.append(f"{path}: header fields must appear in canonical order")
  return data, problems


def validate_field_value(path: Path, field: str, value: str | list[str], problems: list[str]) -> None:
  if field not in REQUIRED_FIELDS:
    problems.append(f"{path}: unexpected header field '{field}'")
    return
  if isinstance(value, str):
    if BANNED_PATTERN.search(value):
      problems.append(f"{path}: field '{field}' contains banned placeholder text")
    if field in LIST_FIELDS and value != "none":
      problems.append(f"{path}: field '{field}' must be a list or 'none'")
    if field == "summary" and len(value.split()) > 24:
      problems.append(f"{path}: summary exceeds 24 words")
    return
  if not value:
    problems.append(f"{path}: field '{field}' cannot be an empty list")
    return
  for item in value:
    if BANNED_PATTERN.search(item):
      problems.append(f"{path}: field '{field}' contains banned placeholder text")
    if field in {"provides", "uses"}:
      if ": " not in item:
        problems.append(f"{path}: interface entry '{item}' in '{field}' is missing a kind prefix")
        continue
      kind, _ = item.split(": ", 1)
      if kind not in INTERFACE_KINDS:
        problems.append(f"{path}: interface kind '{kind}' in '{field}' is not allowed")


def validate_path(path: Path) -> list[str]:
  problems: list[str] = []
  lines = read_lines(path)
  prefix = comment_prefix(path)
  block = header_block(lines, prefix)
  if block is None:
    problems.append(f"{path}: missing header block at top of file")
    return problems
  start, end = block
  data, parse_problems = parse_header(path, lines, prefix, start, end)
  problems.extend(parse_problems)
  for field in REQUIRED_FIELDS:
    if field not in data:
      problems.append(f"{path}: missing required field '{field}'")
  for field, value in data.items():
    validate_field_value(path, field, value, problems)
  if end + 1 < len(lines) and lines[end + 1] != "":
    problems.append(f"{path}: expected a blank line after header block")
  return problems


def main() -> int:
  problems: list[str] = []
  for path in iter_in_scope_files():
    problems.extend(validate_path(path))
  if problems:
    for problem in problems:
      print(problem, file=sys.stderr)
    print(f"header validation failed with {len(problems)} problem(s)", file=sys.stderr)
    return 1
  print("source file headers: ok")
  return 0


if __name__ == "__main__":
    raise SystemExit(main())
