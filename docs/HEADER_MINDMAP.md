# Header Mindmap

`scripts/generate_header_mindmap.py` builds a Mermaid mindmap from the mandatory source-file headers.

The script is intentionally metadata-driven:
- it reads the committed `@amadeus-header` blocks
- groups files by `layer`
- includes each file’s `summary`, `status`, selected `feature_flags`, `provides`, and `uses`
- tolerates malformed or missing headers by warning instead of crashing unless `--strict` is set

## Usage

Generate the default Mermaid output file:

```bash
python3 scripts/generate_header_mindmap.py
```

Write the mindmap to stdout:

```bash
python3 scripts/generate_header_mindmap.py --stdout
```

Write to a custom file:

```bash
python3 scripts/generate_header_mindmap.py --output docs/my_header_map.mmd
```

Fail on any missing or malformed header:

```bash
python3 scripts/generate_header_mindmap.py --strict
```

Limit how many metadata items are shown per file:

```bash
python3 scripts/generate_header_mindmap.py --max-items 3
```

## Output

The script emits Mermaid `mindmap` syntax. A typical downstream workflow is:

1. Generate the `.mmd` file.
2. Paste it into a Mermaid renderer, Markdown viewer, or docs page that supports Mermaid.
3. Regenerate it whenever header metadata changes materially.

## Maintenance Notes

- The mindmap is only as good as the headers. If a header is stale, the mindmap is stale.
- The generator is visualization-oriented, not a validator. Use `scripts/check_source_headers.py` for policy enforcement.
- If the header schema changes in `docs/SOURCE_FILE_HEADERS.md`, update this script to keep the rendered structure aligned with the schema.
