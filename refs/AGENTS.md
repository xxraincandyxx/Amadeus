# AGENTS.md

This repository contains educational AI agent implementations (v0-v4). This document guides agentic coding agents working in this codebase.

## Build / Lint / Test Commands

### Install Dependencies
```bash
pip install -r requirements.txt
```

### Configuration
```bash
cp .env.example .env
# Edit .env with your ANTHROPIC_API_KEY
```

### Run Tests
```bash
# Unit tests (no API calls required)
python tests/test_unit.py

# Integration tests (requires TEST_API_KEY env var)
python tests/test_agent.py

# To run a single unit test:
python -m pytest tests/test_unit.py::test_imports -v
# Or edit tests/test_unit.py and run only one test function
```

### Run Agents
```bash
python v0_bash_agent.py       # Minimal bash-only agent (~50 lines)
python v1_basic_agent.py      # 4 core tools (~200 lines)
python v2_todo_agent.py       # + Todo planning (~300 lines)
python v3_subagent.py         # + Subagents (~450 lines)
python v4_skills_agent.py     # + Skills (~550 lines)
```

## Code Style Guidelines

### Imports
- Standard library imports first, then third-party
- Group with blank lines between sections
- Use `load_dotenv(override=True)` for environment loading

### Naming Conventions
- **Functions/Variables**: `snake_case` (e.g., `run_bash`, `safe_path`, `workdir`)
- **Constants**: `SCREAMING_SNAKE_CASE` at module top (e.g., `WORKDIR`, `MODEL`, `SYSTEM`, `TOOLS`, `AGENT_TYPES`)
- **Classes**: `PascalCase` (e.g., `TodoManager`, `SkillLoader`)
- **Tools**: lowercase JSON names matching function names (e.g., `"name": "bash"`, `"read_file"`)

### File Organization
- Each agent version is a single self-contained file (v0-v4)
- Section dividers use comment blocks:
  ```python
  # =============================================================================
  # Section Name
  # =============================================================================
  ```
- Standard sections:
  1. Configuration (constants, env vars)
  2. Classes (Manager patterns)
  3. Tool Definitions (JSON schemas)
  4. Tool Implementations (functions that execute tools)
  5. Main Agent Loop (the while loop)
  6. Entry Point (main() function)

### Type Hints
- **NOT used** in this codebase for simplicity/educational purposes
- Keep new code consistent with this style

### Docstrings
- Module-level docstrings at the top of each file (triple quotes)
- Function docstrings only for complex functions
- Keep them concise and practical

### Error Handling
- Use try/except for subprocess, file I/O, and API calls
- Return error messages as strings prefixed with "Error: " (e.g., `return f"Error: {e}"`)
- Validate input constraints and raise ValueError for invalid data (e.g., TodoManager validates item count, status values)

### Tools Implementation Pattern
- Each tool has: (1) JSON schema definition, (2) implementation function
- Tool functions named `run_<tool_name>` (e.g., `run_bash`, `run_read`)
- Dispatcher function `execute_tool(name, args)` routes calls
- All tool output truncated to ~50KB to prevent context bloat

### The Agent Loop Pattern
```python
def agent_loop(messages: list) -> list:
    while True:
        response = client.messages.create(model, system, messages, tools)
        if response.stop_reason != "tool_use":
            return messages
        results = []
        for tc in response.tool_calls:
            output = execute_tool(tc.name, tc.input)
            results.append({"type": "tool_result", "tool_use_id": tc.id, "content": output})
        messages.append({"role": "assistant", "content": response.content})
        messages.append({"role": "user", "content": results})
```

### System Prompts
- F-strings with interpolated variables (e.g., `f"""You are at {WORKDIR}"""`)
- Keep them concise (2-5 lines for v1-v2, longer for v3-v4 with skill/agent lists)
- Define as module-level constants (SYSTEM)

### Path Security
- All file operations must go through `safe_path(path)` to prevent directory traversal
- `safe_path` resolves relative to `WORKDIR` and checks `is_relative_to()`
- Raises `ValueError` if path escapes workspace

### Skills System (v4)
- Skills are directories with `SKILL.md` file
- SKILL.md format: YAML frontmatter (---) with name/description, then markdown body
- SkillLoader parses at startup, loads body on-demand (cache-friendly)
- Skill content injected via tool_result wrapped in `<skill-loaded>` tags
- Structure: `skills/<skill-name>/SKILL.md` with optional `scripts/`, `references/`, `assets/`

### Agent Types (v3)
- AGENT_TYPES registry maps type names to configs (description, tools, prompt)
- `get_tools_for_agent(type)` filters tools per agent
- Subagents use limited tool sets (e.g., "explore" has read-only tools)

### Todo Tracking (v2)
- TodoManager enforces: max 20 items, exactly one in_progress, required fields
- Valid statuses: "pending", "in_progress", "completed"
- Required fields: content (string), status (enum), activeForm (string)

### Testing Philosophy
- Unit tests: no API calls, verify structure/logic (test_unit.py)
- Integration tests: real agent tasks with API (test_agent.py)
- Test functions named `test_<subject>_<scenario>`
- Use assert statements for simplicity

## Key Design Principles
- The model is the decision-maker; code just provides tools and runs the loop
- Prefer tools over explanations - let the model act
- Keep context clean (truncation, isolation, cache-friendly injections)
- Progressive complexity: v0 (16 lines) → v4 (550 lines), each adding one concept
