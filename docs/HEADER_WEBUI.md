# Header Web UI

`scripts/generate_header_webui.py` builds a static HTML interface from the mandatory source-file headers.

The UI is designed for quick architectural exploration:
- search by path, summary, interface, invariant, or feature flag
- filter by `layer`, `status`, and `feature_flags`
- browse a layered mindmap-style layout of the repository
- inspect each file’s metadata density through `provides`, `uses`, and interface counts

## Usage

Generate the default HTML output:

```bash
python3 scripts/generate_header_webui.py
```

This writes:

```text
docs/header_map.html
```

Write the HTML to stdout:

```bash
python3 scripts/generate_header_webui.py --stdout
```

Emit only the JSON payload:

```bash
python3 scripts/generate_header_webui.py --json-only
```

Fail if any in-scope file is missing a valid header:

```bash
python3 scripts/generate_header_webui.py --strict
```

Generate to a custom file:

```bash
python3 scripts/generate_header_webui.py --output docs/custom_header_map.html
```

## Viewing

Open the generated file directly in a browser:

```bash
open docs/header_map.html
```

Because the HTML embeds the header metadata payload directly, it does not require a separate server or asset pipeline.

## Maintenance Notes

- The generated UI is only as accurate as the source-file headers.
- Use `scripts/check_source_headers.py` to enforce header correctness.
- If the header schema or mindmap expectations change, keep this script aligned with `docs/SOURCE_FILE_HEADERS.md` and `scripts/generate_header_mindmap.py`.
