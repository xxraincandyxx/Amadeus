#!/usr/bin/env python3
# @amadeus-header
# summary: Generates a static web UI from source-file header metadata.
# layer: script
# status: active
# feature_flags:
# - full
# provides:
# - cmd: scripts/generate_header_webui.py
# - artifact: docs/header_map.html
# - format: header metadata JSON
# uses:
# - cmd: python3
# - artifact: source file headers
# invariants:
# - The rendered UI stays derived from committed source-file headers.
# - Missing or malformed headers warn by default and fail only in strict mode.
# side_effects:
# - Reads filesystem state.
# - Writes output to stdout or generated files.
# tests:
# - cmd: python3 scripts/generate_header_webui.py --stdout | head
# @end-amadeus-header

from __future__ import annotations

import argparse
import json
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
DEFAULT_OUTPUT = ROOT / "docs/header_map.html"


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


def list_value(value: str | list[str] | None) -> list[str]:
  if value is None or value == "none":
    return []
  if isinstance(value, list):
    return value
  return [value]


def build_payload() -> tuple[dict[str, object], list[str]]:
  warnings: list[str] = []
  records: list[dict[str, object]] = []

  for path in iter_in_scope_files():
    data, problems = parse_header(path)
    warnings.extend(problems)
    if data is None:
      continue
    rel = path.relative_to(ROOT).as_posix()
    provides = list_value(data.get("provides"))
    uses = list_value(data.get("uses"))
    records.append(
        {
            "path": rel,
            "name": Path(rel).name,
            "layer": data["layer"],
            "status": data["status"],
            "summary": data["summary"],
            "feature_flags": list_value(data.get("feature_flags")),
            "provides": provides,
            "uses": uses,
            "invariants": list_value(data.get("invariants")),
            "side_effects": list_value(data.get("side_effects")),
            "tests": list_value(data.get("tests")),
            "score": len(provides) + len(uses),
        }
    )

  layers = sorted({str(record["layer"]) for record in records})
  statuses = sorted({str(record["status"]) for record in records})
  feature_flags = sorted(
      {
          flag
          for record in records
          for flag in record["feature_flags"]
          if isinstance(flag, str)
      }
  )

  payload = {
      "generated_from": "source-file headers",
      "file_count": len(records),
      "layers": layers,
      "statuses": statuses,
      "feature_flags": feature_flags,
      "records": sorted(records, key=lambda record: str(record["path"])),
  }
  return payload, warnings


def html_template(payload_json: str) -> str:
  return f"""<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>Amadeus Header Map</title>
  <style>
    :root {{
      --bg: #f3efe3;
      --bg-soft: #fffaf0;
      --ink: #16211f;
      --muted: #5e6f6a;
      --line: rgba(22, 33, 31, 0.12);
      --card: rgba(255, 250, 240, 0.82);
      --shadow: 0 24px 70px rgba(48, 39, 20, 0.16);
      --core: #0f766e;
      --agent: #ef4444;
      --client: #2563eb;
      --tools: #f97316;
      --policy: #7c3aed;
      --ui: #0f766e;
      --api: #0284c7;
      --benchmark: #ca8a04;
      --test: #64748b;
      --example: #db2777;
      --script: #166534;
      --infra: #4b5563;
      --active: #166534;
      --experimental: #b45309;
      --deprecated: #b91c1c;
      --test-only: #475569;
      --generated: #6d28d9;
    }}

    * {{
      box-sizing: border-box;
    }}

    html, body {{
      margin: 0;
      min-height: 100%;
      font-family: "Iowan Old Style", "Palatino Linotype", "Book Antiqua", Palatino, serif;
      color: var(--ink);
      background:
        radial-gradient(circle at top left, rgba(15, 118, 110, 0.16), transparent 32%),
        radial-gradient(circle at top right, rgba(239, 68, 68, 0.12), transparent 28%),
        linear-gradient(180deg, #fcf7ea 0%, #efe7d4 100%);
    }}

    body {{
      padding: 28px;
    }}

    .shell {{
      max-width: 1480px;
      margin: 0 auto;
      display: grid;
      grid-template-columns: 340px minmax(0, 1fr);
      gap: 22px;
    }}

    .panel {{
      background: var(--card);
      backdrop-filter: blur(16px);
      border: 1px solid rgba(255, 255, 255, 0.55);
      border-radius: 28px;
      box-shadow: var(--shadow);
    }}

    .controls {{
      padding: 26px 22px;
      position: sticky;
      top: 24px;
      align-self: start;
    }}

    .eyebrow {{
      font-size: 12px;
      letter-spacing: 0.18em;
      text-transform: uppercase;
      color: var(--muted);
      margin: 0 0 10px;
    }}

    h1 {{
      margin: 0;
      font-size: 42px;
      line-height: 0.95;
      font-weight: 700;
    }}

    .lede {{
      margin: 14px 0 24px;
      color: var(--muted);
      font-size: 15px;
      line-height: 1.5;
    }}

    .stat-grid {{
      display: grid;
      grid-template-columns: repeat(2, minmax(0, 1fr));
      gap: 10px;
      margin-bottom: 22px;
    }}

    .stat {{
      background: rgba(255, 255, 255, 0.58);
      border: 1px solid rgba(22, 33, 31, 0.08);
      border-radius: 18px;
      padding: 14px 14px 12px;
    }}

    .stat strong {{
      display: block;
      font-size: 26px;
      margin-bottom: 4px;
    }}

    .stat span {{
      font-size: 12px;
      color: var(--muted);
      text-transform: uppercase;
      letter-spacing: 0.12em;
    }}

    .field {{
      margin-bottom: 18px;
    }}

    .field label {{
      display: block;
      font-size: 12px;
      font-weight: 700;
      letter-spacing: 0.12em;
      text-transform: uppercase;
      color: var(--muted);
      margin-bottom: 8px;
    }}

    .field input {{
      width: 100%;
      border: 1px solid rgba(22, 33, 31, 0.12);
      border-radius: 14px;
      padding: 12px 14px;
      font: inherit;
      background: rgba(255, 255, 255, 0.76);
      color: var(--ink);
      outline: none;
    }}

    .field input:focus {{
      border-color: rgba(15, 118, 110, 0.45);
      box-shadow: 0 0 0 4px rgba(15, 118, 110, 0.1);
    }}

    .chip-group {{
      display: flex;
      flex-wrap: wrap;
      gap: 8px;
    }}

    .chip {{
      border: 1px solid rgba(22, 33, 31, 0.1);
      background: rgba(255, 255, 255, 0.68);
      color: var(--ink);
      border-radius: 999px;
      padding: 7px 12px;
      font: inherit;
      font-size: 13px;
      cursor: pointer;
      transition: 140ms ease;
    }}

    .chip:hover {{
      transform: translateY(-1px);
      border-color: rgba(22, 33, 31, 0.22);
    }}

    .chip.active {{
      background: var(--ink);
      color: #fffaf0;
      border-color: var(--ink);
    }}

    .actions {{
      display: flex;
      gap: 10px;
      margin-top: 18px;
    }}

    .button {{
      border: 0;
      border-radius: 14px;
      padding: 11px 14px;
      font: inherit;
      cursor: pointer;
    }}

    .button.primary {{
      background: var(--ink);
      color: #fffaf0;
    }}

    .button.secondary {{
      background: rgba(255, 255, 255, 0.7);
      color: var(--ink);
      border: 1px solid rgba(22, 33, 31, 0.1);
    }}

    .stage {{
      padding: 22px;
      overflow: hidden;
      min-height: 860px;
      position: relative;
    }}

    .stage-header {{
      display: flex;
      justify-content: space-between;
      align-items: end;
      gap: 12px;
      margin-bottom: 18px;
    }}

    .stage-header h2 {{
      margin: 0;
      font-size: 28px;
    }}

    .stage-header p {{
      margin: 6px 0 0;
      color: var(--muted);
      max-width: 740px;
      line-height: 1.45;
    }}

    .legend {{
      display: flex;
      flex-wrap: wrap;
      gap: 10px;
      justify-content: end;
    }}

    .legend-item {{
      display: inline-flex;
      align-items: center;
      gap: 8px;
      font-size: 12px;
      color: var(--muted);
    }}

    .swatch {{
      width: 12px;
      height: 12px;
      border-radius: 999px;
    }}

    .map {{
      position: relative;
      min-height: 760px;
      border-radius: 24px;
      background:
        linear-gradient(180deg, rgba(255,255,255,0.62), rgba(255,255,255,0.34)),
        radial-gradient(circle at center, rgba(15, 118, 110, 0.07), transparent 48%);
      border: 1px solid rgba(22, 33, 31, 0.08);
      overflow: auto;
      padding: 26px 26px 30px;
    }}

    .empty {{
      padding: 48px 18px;
      text-align: center;
      color: var(--muted);
    }}

    .mind-root {{
      display: flex;
      flex-direction: column;
      gap: 28px;
      min-width: 920px;
    }}

    .root-card {{
      align-self: center;
      text-align: center;
      background: linear-gradient(135deg, rgba(22,33,31,0.96), rgba(15,118,110,0.84));
      color: #fffaf0;
      border-radius: 26px;
      padding: 22px 28px;
      box-shadow: 0 16px 50px rgba(15, 118, 110, 0.22);
      min-width: 320px;
    }}

    .root-card strong {{
      display: block;
      font-size: 32px;
      margin-bottom: 6px;
    }}

    .root-card span {{
      color: rgba(255, 250, 240, 0.76);
      font-size: 14px;
    }}

    .layer-grid {{
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(290px, 1fr));
      gap: 18px;
      align-items: start;
    }}

    .layer {{
      border-radius: 24px;
      border: 1px solid rgba(22, 33, 31, 0.08);
      background: rgba(255, 255, 255, 0.7);
      overflow: hidden;
      position: relative;
    }}

    .layer::before {{
      content: "";
      position: absolute;
      inset: 0 0 auto;
      height: 5px;
      background: var(--layer-color, var(--ink));
    }}

    .layer-head {{
      padding: 18px 18px 14px;
      border-bottom: 1px solid rgba(22, 33, 31, 0.06);
    }}

    .layer-head h3 {{
      margin: 0;
      font-size: 22px;
      text-transform: capitalize;
    }}

    .layer-head p {{
      margin: 6px 0 0;
      color: var(--muted);
      font-size: 13px;
    }}

    .file-stack {{
      display: flex;
      flex-direction: column;
      gap: 12px;
      padding: 16px;
    }}

    .node {{
      position: relative;
      padding: 16px;
      border-radius: 18px;
      background: rgba(252, 247, 234, 0.92);
      border: 1px solid rgba(22, 33, 31, 0.08);
      box-shadow: 0 12px 24px rgba(22, 33, 31, 0.06);
    }}

    .node::before {{
      content: "";
      position: absolute;
      top: -12px;
      left: 22px;
      width: 1px;
      height: 12px;
      background: rgba(22, 33, 31, 0.14);
    }}

    .file-stack .node:first-child::before {{
      display: none;
    }}

    .node-head {{
      display: flex;
      justify-content: space-between;
      gap: 10px;
      align-items: start;
      margin-bottom: 10px;
    }}

    .path {{
      font-size: 13px;
      color: var(--muted);
      word-break: break-all;
    }}

    .summary {{
      margin: 6px 0 0;
      font-size: 15px;
      line-height: 1.38;
    }}

    .badge {{
      flex: 0 0 auto;
      padding: 6px 10px;
      border-radius: 999px;
      font-size: 11px;
      text-transform: uppercase;
      letter-spacing: 0.12em;
      color: #fffaf0;
      background: var(--status-color, var(--ink));
    }}

    .meta-row {{
      display: flex;
      flex-wrap: wrap;
      gap: 8px;
      margin: 10px 0 12px;
    }}

    .tag {{
      font-size: 11px;
      letter-spacing: 0.04em;
      text-transform: uppercase;
      border-radius: 999px;
      padding: 5px 9px;
      background: rgba(22, 33, 31, 0.06);
      color: var(--muted);
    }}

    .metric-grid {{
      display: grid;
      grid-template-columns: repeat(3, minmax(0, 1fr));
      gap: 10px;
      margin-bottom: 12px;
    }}

    .metric {{
      background: rgba(255, 255, 255, 0.68);
      border-radius: 14px;
      padding: 10px 10px 8px;
      border: 1px solid rgba(22, 33, 31, 0.06);
    }}

    .metric strong {{
      display: block;
      font-size: 20px;
      margin-bottom: 2px;
    }}

    .metric span {{
      font-size: 11px;
      text-transform: uppercase;
      letter-spacing: 0.12em;
      color: var(--muted);
    }}

    .section {{
      margin-top: 12px;
    }}

    .section h4 {{
      margin: 0 0 8px;
      font-size: 12px;
      text-transform: uppercase;
      letter-spacing: 0.14em;
      color: var(--muted);
    }}

    .mini-list {{
      display: flex;
      flex-wrap: wrap;
      gap: 7px;
    }}

    .mini-pill {{
      display: inline-flex;
      align-items: center;
      max-width: 100%;
      padding: 6px 9px;
      border-radius: 12px;
      background: rgba(22, 33, 31, 0.055);
      font-size: 12px;
      line-height: 1.25;
    }}

    @media (max-width: 1100px) {{
      .shell {{
        grid-template-columns: 1fr;
      }}

      .controls {{
        position: static;
      }}
    }}

    @media (max-width: 720px) {{
      body {{
        padding: 16px;
      }}

      h1 {{
        font-size: 34px;
      }}

      .stage {{
        padding: 18px;
      }}

      .map {{
        padding: 18px;
      }}

      .mind-root {{
        min-width: 0;
      }}

      .metric-grid {{
        grid-template-columns: 1fr;
      }}
    }}
  </style>
</head>
<body>
  <div class="shell">
    <aside class="panel controls">
      <p class="eyebrow">Header Graph</p>
      <h1>Source Mindspace</h1>
      <p class="lede">
        An interactive map of the repository built from the source-file headers.
        Search by file, summary, interface, or feature flag and narrow the view by layer or status.
      </p>

      <div class="stat-grid">
        <div class="stat"><strong id="visible-count">0</strong><span>Visible Files</span></div>
        <div class="stat"><strong id="layer-count">0</strong><span>Visible Layers</span></div>
        <div class="stat"><strong id="provide-count">0</strong><span>Provides</span></div>
        <div class="stat"><strong id="use-count">0</strong><span>Uses</span></div>
      </div>

      <div class="field">
        <label for="search">Search</label>
        <input id="search" type="search" placeholder="path, summary, interface, feature..." />
      </div>

      <div class="field">
        <label>Layer</label>
        <div id="layer-filters" class="chip-group"></div>
      </div>

      <div class="field">
        <label>Status</label>
        <div id="status-filters" class="chip-group"></div>
      </div>

      <div class="field">
        <label>Feature Flag</label>
        <div id="feature-filters" class="chip-group"></div>
      </div>

      <div class="actions">
        <button id="reset" class="button secondary" type="button">Reset Filters</button>
        <button id="expand" class="button primary" type="button">Show Full Map</button>
      </div>
    </aside>

    <main class="panel stage">
      <div class="stage-header">
        <div>
          <p class="eyebrow">Generated View</p>
          <h2>Layered Repository Mindmap</h2>
          <p>
            Files are grouped by architectural layer and visualized as node clusters.
            Each node shows status, interface density, and its most important provided and consumed boundaries.
          </p>
        </div>
        <div id="legend" class="legend"></div>
      </div>
      <section id="map" class="map"></section>
    </main>
  </div>

  <script>
    const DATA = {payload_json};

    const state = {{
      query: "",
      layer: "all",
      status: "all",
      feature: "all",
      expanded: false,
    }};

    const layerColors = {{
      core: "var(--core)",
      agent: "var(--agent)",
      client: "var(--client)",
      tools: "var(--tools)",
      policy: "var(--policy)",
      ui: "var(--ui)",
      api: "var(--api)",
      benchmark: "var(--benchmark)",
      test: "var(--test)",
      example: "var(--example)",
      script: "var(--script)",
      infra: "var(--infra)",
    }};

    const statusColors = {{
      active: "var(--active)",
      experimental: "var(--experimental)",
      deprecated: "var(--deprecated)",
      "test-only": "var(--test)",
      generated: "var(--generated)",
    }};

    const searchEl = document.getElementById("search");
    const layerFiltersEl = document.getElementById("layer-filters");
    const statusFiltersEl = document.getElementById("status-filters");
    const featureFiltersEl = document.getElementById("feature-filters");
    const visibleCountEl = document.getElementById("visible-count");
    const layerCountEl = document.getElementById("layer-count");
    const provideCountEl = document.getElementById("provide-count");
    const useCountEl = document.getElementById("use-count");
    const mapEl = document.getElementById("map");
    const legendEl = document.getElementById("legend");

    function compactLabel(label) {{
      return label.replace(/^.*?:\\s*/, "");
    }}

    function makeChip(label, value, kind) {{
      const button = document.createElement("button");
      button.type = "button";
      button.className = "chip";
      button.textContent = label;
      button.dataset.kind = kind;
      button.dataset.value = value;
      button.addEventListener("click", () => {{
        state[kind] = state[kind] === value ? "all" : value;
        render();
      }});
      return button;
    }}

    function populateFilters() {{
      layerFiltersEl.appendChild(makeChip("All", "all", "layer"));
      DATA.layers.forEach((layer) => layerFiltersEl.appendChild(makeChip(layer, layer, "layer")));

      statusFiltersEl.appendChild(makeChip("All", "all", "status"));
      DATA.statuses.forEach((status) => statusFiltersEl.appendChild(makeChip(status, status, "status")));

      featureFiltersEl.appendChild(makeChip("All", "all", "feature"));
      DATA.feature_flags.forEach((flag) => featureFiltersEl.appendChild(makeChip(flag, flag, "feature")));
    }}

    function renderLegend() {{
      legendEl.innerHTML = "";
      DATA.layers.forEach((layer) => {{
        const item = document.createElement("div");
        item.className = "legend-item";
        const swatch = document.createElement("span");
        swatch.className = "swatch";
        swatch.style.background = layerColors[layer] || "var(--ink)";
        const label = document.createElement("span");
        label.textContent = layer;
        item.append(swatch, label);
        legendEl.appendChild(item);
      }});
    }}

    function matches(record) {{
      if (state.layer !== "all" && record.layer !== state.layer) return false;
      if (state.status !== "all" && record.status !== state.status) return false;
      if (state.feature !== "all" && !record.feature_flags.includes(state.feature)) return false;
      if (!state.query) return true;
      const haystack = [
        record.path,
        record.summary,
        record.layer,
        record.status,
        ...record.feature_flags,
        ...record.provides,
        ...record.uses,
        ...record.invariants,
      ].join("\\n").toLowerCase();
      return haystack.includes(state.query);
    }}

    function filteredRecords() {{
      return DATA.records.filter(matches);
    }}

    function renderStats(records) {{
      visibleCountEl.textContent = records.length;
      layerCountEl.textContent = new Set(records.map((record) => record.layer)).size;
      provideCountEl.textContent = records.reduce((sum, record) => sum + record.provides.length, 0);
      useCountEl.textContent = records.reduce((sum, record) => sum + record.uses.length, 0);
    }}

    function updateChips() {{
      document.querySelectorAll(".chip").forEach((chip) => {{
        const kind = chip.dataset.kind;
        const value = chip.dataset.value;
        chip.classList.toggle("active", state[kind] === value || (state[kind] === "all" && value === "all"));
      }});
    }}

    function node(record) {{
      const article = document.createElement("article");
      article.className = "node";

      const head = document.createElement("div");
      head.className = "node-head";

      const titleWrap = document.createElement("div");
      const path = document.createElement("div");
      path.className = "path";
      path.textContent = record.path;
      const summary = document.createElement("p");
      summary.className = "summary";
      summary.textContent = record.summary;
      titleWrap.append(path, summary);

      const badge = document.createElement("span");
      badge.className = "badge";
      badge.textContent = record.status;
      badge.style.background = statusColors[record.status] || "var(--ink)";
      head.append(titleWrap, badge);

      const metaRow = document.createElement("div");
      metaRow.className = "meta-row";
      [record.layer, ...record.feature_flags.slice(0, state.expanded ? 6 : 3)].forEach((item) => {{
        const tag = document.createElement("span");
        tag.className = "tag";
        tag.textContent = item;
        metaRow.appendChild(tag);
      }});

      const metricGrid = document.createElement("div");
      metricGrid.className = "metric-grid";
      [
        ["provides", record.provides.length],
        ["uses", record.uses.length],
        ["signal", record.score],
      ].forEach(([label, value]) => {{
        const metric = document.createElement("div");
        metric.className = "metric";
        const strong = document.createElement("strong");
        strong.textContent = String(value);
        const span = document.createElement("span");
        span.textContent = label;
        metric.append(strong, span);
        metricGrid.appendChild(metric);
      }});

      const sections = [
        ["provides", record.provides],
        ["uses", record.uses],
      ];

      sections.forEach(([title, values]) => {{
        if (!values.length) return;
        const section = document.createElement("section");
        section.className = "section";
        const heading = document.createElement("h4");
        heading.textContent = title;
        const list = document.createElement("div");
        list.className = "mini-list";
        values.slice(0, state.expanded ? 8 : 4).forEach((value) => {{
          const pill = document.createElement("span");
          pill.className = "mini-pill";
          pill.textContent = compactLabel(value);
          list.appendChild(pill);
        }});
        section.append(heading, list);
        article.appendChild(section);
      }});

      article.prepend(metricGrid);
      article.prepend(metaRow);
      article.prepend(head);
      return article;
    }}

    function renderMap(records) {{
      mapEl.innerHTML = "";
      if (!records.length) {{
        const empty = document.createElement("div");
        empty.className = "empty";
        empty.innerHTML = "<h3>No files match the current filters.</h3><p>Reset filters or broaden the search query.</p>";
        mapEl.appendChild(empty);
        return;
      }}

      const root = document.createElement("div");
      root.className = "mind-root";

      const rootCard = document.createElement("div");
      rootCard.className = "root-card";
      const title = document.createElement("strong");
      title.textContent = "Amadeus";
      const caption = document.createElement("span");
      caption.textContent = `${{records.length}} visible files mapped from header metadata`;
      rootCard.append(title, caption);
      root.appendChild(rootCard);

      const layerGrid = document.createElement("div");
      layerGrid.className = "layer-grid";

      const grouped = new Map();
      records.forEach((record) => {{
        if (!grouped.has(record.layer)) grouped.set(record.layer, []);
        grouped.get(record.layer).push(record);
      }});

      [...grouped.entries()]
        .sort((a, b) => a[0].localeCompare(b[0]))
        .forEach(([layer, layerRecords]) => {{
          const block = document.createElement("section");
          block.className = "layer";
          block.style.setProperty("--layer-color", layerColors[layer] || "var(--ink)");

          const head = document.createElement("div");
          head.className = "layer-head";
          const h3 = document.createElement("h3");
          h3.textContent = layer;
          const p = document.createElement("p");
          p.textContent = `${{layerRecords.length}} file${{layerRecords.length === 1 ? "" : "s"}}`;
          head.append(h3, p);

          const stack = document.createElement("div");
          stack.className = "file-stack";
          layerRecords
            .sort((a, b) => b.score - a.score || a.path.localeCompare(b.path))
            .forEach((record) => stack.appendChild(node(record)));

          block.append(head, stack);
          layerGrid.appendChild(block);
        }});

      root.appendChild(layerGrid);
      mapEl.appendChild(root);
    }}

    function render() {{
      state.query = searchEl.value.trim().toLowerCase();
      const records = filteredRecords();
      renderStats(records);
      updateChips();
      renderMap(records);
    }}

    document.getElementById("reset").addEventListener("click", () => {{
      state.query = "";
      state.layer = "all";
      state.status = "all";
      state.feature = "all";
      searchEl.value = "";
      render();
    }});

    document.getElementById("expand").addEventListener("click", (event) => {{
      state.expanded = !state.expanded;
      event.currentTarget.textContent = state.expanded ? "Show Compact Map" : "Show Full Map";
      render();
    }});

    searchEl.addEventListener("input", render);

    populateFilters();
    renderLegend();
    render();
  </script>
</body>
</html>
"""


def parse_args() -> argparse.Namespace:
  parser = argparse.ArgumentParser(
      description="Generate a static web UI from source-file headers."
  )
  parser.add_argument(
      "--output",
      type=Path,
      default=DEFAULT_OUTPUT,
      help=f"Target HTML file. Defaults to {DEFAULT_OUTPUT.relative_to(ROOT)}",
  )
  parser.add_argument(
      "--stdout",
      action="store_true",
      help="Write the generated HTML to stdout instead of a file.",
  )
  parser.add_argument(
      "--strict",
      action="store_true",
      help="Fail if any file header is missing or malformed.",
  )
  parser.add_argument(
      "--json-only",
      action="store_true",
      help="Write only the JSON payload to stdout.",
  )
  return parser.parse_args()


def main() -> int:
  args = parse_args()
  payload, warnings = build_payload()

  if warnings:
    for warning in warnings:
      print(f"warning: {warning}", file=sys.stderr)
    if args.strict:
      print("web UI generation aborted due to header warnings", file=sys.stderr)
      return 1

  payload_json = json.dumps(payload, ensure_ascii=True, separators=(",", ":"))

  if args.json_only:
    sys.stdout.write(payload_json + "\n")
    return 0

  html = html_template(payload_json)
  if args.stdout:
    sys.stdout.write(html)
  else:
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(html)
    print(f"wrote {args.output.relative_to(ROOT)}")

  if warnings:
    print(
        f"generated web UI with {len(warnings)} header warning(s)",
        file=sys.stderr,
    )
  return 0


if __name__ == "__main__":
  raise SystemExit(main())
