# Skills Guide

Project skills live under `.amadeus/skills/`.

Current load behavior:

- user skills root: `~/.amadeus/skills`
- project skills root: `.amadeus/skills`
- both roots are loaded, with project skills added on top

Recommended layout:

```text
.amadeus/skills/
  my-skill/
    SKILL.md
```

Current skill format:

```md
---
name: my-skill
description: Short trigger description for the skill.
allowed_tools:
  - read_file
  - grep
  - bash
---

Use this skill when...

Steps:
1. Inspect the relevant files.
2. Make the smallest correct change.
3. Verify with focused tests.

Context:
{context}
```

Notes:

- `name` and `description` are required
- `allowed_tools` is optional
- the runtime renders `{context}` at execution time

Working example:

- [feature-assessment-loop/SKILL.md](/.amadeus/skills/feature-assessment-loop/SKILL.md)
- [code-review/SKILL.example.md](/.amadeus/skills/code-review/SKILL.example.md)
