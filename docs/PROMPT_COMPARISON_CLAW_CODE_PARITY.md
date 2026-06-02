# Prompt Surface Review: Amadeus vs `claw-code-parity`

This document compares the prompt-bearing surfaces in Amadeus and the `claw-code-parity` reference snapshot under [refs/claw-code-parity](/Users/raincandy_u/Dev/amadeus/refs/claw-code-parity).

The goal is not to compare branding or UX copy. The focus is the instruction surface seen by the model or by hook/tool subprocesses:
- system prompts
- profile prompts
- compaction and summarization prompts
- tool descriptions and schemas
- hook payload contracts
- sub-agent prompts
- task or assessment prompts

## Scope And Method

This comparison is based on prompt-bearing source files that are actually present in this workspace.

Amadeus sources reviewed:
- [crates/prompts/src/lib.rs](/Users/raincandy_u/Dev/amadeus/crates/prompts/src/lib.rs)
- [crates/config/src/lib.rs](/Users/raincandy_u/Dev/amadeus/crates/config/src/lib.rs)
- [crates/core/src/agent/compaction.rs](/Users/raincandy_u/Dev/amadeus/crates/core/src/agent/compaction.rs)
- [crates/profiles/src/lib.rs](/Users/raincandy_u/Dev/amadeus/crates/profiles/src/lib.rs)
- [crates/context/src/lib.rs](/Users/raincandy_u/Dev/amadeus/crates/context/src/lib.rs)
- [crates/core/src/tools/schema.rs](/Users/raincandy_u/Dev/amadeus/crates/core/src/tools/schema.rs)
- [crates/core/src/hooks/mod.rs](/Users/raincandy_u/Dev/amadeus/crates/core/src/hooks/mod.rs)
- [crates/core/src/hooks/shell.rs](/Users/raincandy_u/Dev/amadeus/crates/core/src/hooks/shell.rs)
- [crates/core/src/assessment/mod.rs](/Users/raincandy_u/Dev/amadeus/crates/core/src/assessment/mod.rs)

`claw-code-parity` sources reviewed:
- [refs/claw-code-parity/rust/crates/runtime/src/prompt.rs](/Users/raincandy_u/Dev/amadeus/refs/claw-code-parity/rust/crates/runtime/src/prompt.rs)
- [refs/claw-code-parity/rust/crates/runtime/src/compact.rs](/Users/raincandy_u/Dev/amadeus/refs/claw-code-parity/rust/crates/runtime/src/compact.rs)
- [refs/claw-code-parity/rust/crates/runtime/src/hooks.rs](/Users/raincandy_u/Dev/amadeus/refs/claw-code-parity/rust/crates/runtime/src/hooks.rs)
- [refs/claw-code-parity/rust/crates/tools/src/lib.rs](/Users/raincandy_u/Dev/amadeus/refs/claw-code-parity/rust/crates/tools/src/lib.rs)

Important limitation:
- The parity snapshot references many archived `tools/*/prompt.ts` files in JSON snapshots, but those prompt files are not present in this repo snapshot. Tool-level comparison therefore uses the actual registered `ToolSpec.description` and schema surface in [tools/src/lib.rs](/Users/raincandy_u/Dev/amadeus/refs/claw-code-parity/rust/crates/tools/src/lib.rs), not the missing archived files.

## Executive Summary

Amadeus currently has a smaller and flatter prompt architecture:
- one compact system prompt
- one explicit LLM compaction prompt
- a few optional role/profile prompts
- simple tool descriptions
- simple hook payloads

`claw-code-parity` has a broader, more layered prompt architecture:
- a composed system prompt with environment, repo instructions, config, and optional output style
- deterministic compaction formatting instead of an LLM summarizer prompt
- much richer tool-description surface
- stronger hook-to-runtime instruction contracts
- more explicit sub-agent prompt shaping

The practical difference is:
- Amadeus is more direct and easier to reason about
- `claw-code-parity` is more complete, more configurable, and better at surfacing runtime state to the model

The largest quality gaps in Amadeus are:
1. the system prompt is too small relative to the runtime behavior it expects
2. tool descriptions are useful but inconsistent in level and naming
3. hook contracts are weaker and less machine-readable
4. profile prompts are disconnected from the main runtime prompt path
5. the compaction prompt is strong, but the rest of the prompt stack is not equally structured

## Inventory By Surface

### 1. Main System Prompt

Amadeus:
- main prompt is a single static template in [crates/prompts/src/lib.rs](/Users/raincandy_u/Dev/amadeus/crates/prompts/src/lib.rs#L25)
- config appends optional project context in [crates/config/src/lib.rs](/Users/raincandy_u/Dev/amadeus/crates/config/src/lib.rs#L329)
- structure is:
  - core loop
  - security
  - context efficiency
  - engineering standards
  - task management
  - tool usage
  - code references
  - output style

`claw-code-parity`:
- system prompt is assembled section-by-section in [runtime/src/prompt.rs](/Users/raincandy_u/Dev/amadeus/refs/claw-code-parity/rust/crates/runtime/src/prompt.rs#L95)
- it includes:
  - intro
  - optional output style
  - system rules
  - task execution rules
  - action-risk rules
  - dynamic boundary marker
  - environment context
  - project context
  - discovered instruction files
  - runtime config

Comparison:
- Amadeus is cleaner and shorter.
- `claw-code-parity` is much closer to a real runtime prompt contract.
- The parity prompt explicitly tells the model about:
  - permission modes
  - hook behavior
  - prompt-injection risk in tool results
  - automatic compression
  - loaded instruction files and config
- Amadeus mentions some of this behavior indirectly, but not enough.

Assessment:
- `claw-code-parity` is stronger here.
- Amadeus should move from one flat static prompt to a builder with explicit sections and dynamic runtime facts.

### 2. Instruction Memory And Project Context

Amadeus:
- project context is a single file lookup in [crates/context/src/lib.rs](/Users/raincandy_u/Dev/amadeus/crates/context/src/lib.rs#L21)
- it injects one `## Project Context` block in [crates/context/src/lib.rs](/Users/raincandy_u/Dev/amadeus/crates/context/src/lib.rs#L53)
- no ancestry search
- no dedupe
- no prompt budget
- no git snapshot

`claw-code-parity`:
- discovers instruction files across ancestor directories in [runtime/src/prompt.rs](/Users/raincandy_u/Dev/amadeus/refs/claw-code-parity/rust/crates/runtime/src/prompt.rs#L192)
- supports `CLAUDE.md`, `CLAUDE.local.md`, `.claw/CLAUDE.md`, `.claw/instructions.md`
- dedupes identical instruction content in [runtime/src/prompt.rs](/Users/raincandy_u/Dev/amadeus/refs/claw-code-parity/rust/crates/runtime/src/prompt.rs#L326)
- truncates instruction content with per-file and total budgets in [runtime/src/prompt.rs](/Users/raincandy_u/Dev/amadeus/refs/claw-code-parity/rust/crates/runtime/src/prompt.rs#L303)
- includes git status and diff snapshots in [runtime/src/prompt.rs](/Users/raincandy_u/Dev/amadeus/refs/claw-code-parity/rust/crates/runtime/src/prompt.rs#L276)

Comparison:
- Amadeus treats context as one optional note.
- `claw-code-parity` treats instruction memory as a first-class prompt subsystem.

Assessment:
- `claw-code-parity` is substantially stronger.
- This is one of the most important deltas because it changes how much runtime and repo state the model actually sees each turn.

### 3. Profile Prompts

Amadeus:
- has standalone profile prompts in [crates/profiles/src/lib.rs](/Users/raincandy_u/Dev/amadeus/crates/profiles/src/lib.rs#L35)
- profiles include:
  - default
  - debug
  - docs
  - code review
  - custom
- these are generic role descriptions, not strongly integrated with the main system prompt

`claw-code-parity`:
- does not use the same kind of named profile prompt enum in the reviewed runtime files
- instead it supports output-style injection and runtime-specific assembled sections in [runtime/src/prompt.rs](/Users/raincandy_u/Dev/amadeus/refs/claw-code-parity/rust/crates/runtime/src/prompt.rs#L102)

Comparison:
- Amadeus has more explicit role variants.
- `claw-code-parity` has a better integrated prompt assembly model.

Assessment:
- Amadeus has the better idea for reusable roles.
- `claw-code-parity` has the better implementation pattern.
- The right direction for Amadeus is to fold profile prompts into the main prompt builder, not leave them as a parallel unused prompt system.

### 4. Context Compaction And Summarization

Amadeus:
- uses an explicit LLM compaction prompt in [crates/core/src/agent/compaction.rs](/Users/raincandy_u/Dev/amadeus/crates/core/src/agent/compaction.rs#L76)
- prompt is structured and strong:
  - explicit prompt-injection warning
  - required XML output
  - required sections like `overall_goal`, `active_constraints`, `artifact_trail`, `task_state`
  - asks for dense state preservation

`claw-code-parity`:
- compaction in [runtime/src/compact.rs](/Users/raincandy_u/Dev/amadeus/refs/claw-code-parity/rust/crates/runtime/src/compact.rs#L1) is deterministic and local
- it does not use an LLM prompt for compaction in the reviewed code
- instead it:
  - summarizes messages into a tagged `<summary>`
  - preserves recent messages
  - merges older compacted summaries
  - injects a continuation instruction telling the assistant to resume directly

Comparison:
- Amadeus has the stronger summarization prompt design.
- `claw-code-parity` has the safer and cheaper runtime design because it avoids another model call.

Assessment:
- On prompt quality alone, Amadeus wins here.
- On operational robustness and cost, `claw-code-parity` wins.
- The best future direction is probably hybrid:
  - deterministic compaction for normal turns
  - optional LLM compaction only for high-complexity sessions

### 5. Tool Descriptions And Tool-Use Guidance

Amadeus:
- tool guidance is split between:
  - system prompt tool bullets in [crates/prompts/src/lib.rs](/Users/raincandy_u/Dev/amadeus/crates/prompts/src/lib.rs#L49)
  - JSON tool descriptions in [crates/core/src/tools/schema.rs](/Users/raincandy_u/Dev/amadeus/crates/core/src/tools/schema.rs#L37)
- strengths:
  - direct language
  - clear basic intent for `bash`, `read_file`, `edit_file`, `todo`
- weaknesses:
  - naming and description style is inconsistent
  - fewer high-level control-plane tools
  - system prompt says “Use tools to accomplish tasks, not to explain” but tool descriptions do not consistently encode planning heuristics or safety hints

`claw-code-parity`:
- tool descriptions are defined centrally in [refs/claw-code-parity/rust/crates/tools/src/lib.rs](/Users/raincandy_u/Dev/amadeus/refs/claw-code-parity/rust/crates/tools/src/lib.rs#L377)
- the tool surface is much broader:
  - primitive file/shell/search tools
  - web fetch/search
  - skill loading
  - worker/team/task tools
  - config/plan-mode tools
  - MCP tools
  - structured output and user interaction tools
- descriptions are usually short, but they encode more product intent than Amadeus’s current schemas

Comparison:
- Amadeus tool descriptions are decent for primitives.
- `claw-code-parity` tool descriptions form a wider operational vocabulary for the model.

Assessment:
- `claw-code-parity` is stronger because the model can reason at multiple abstraction levels.
- Amadeus now has the tool-platform groundwork, but its prompt layer still mostly speaks in primitive tools.

### 6. Hook Prompt / Payload Contract

Amadeus:
- hooks are described as simple pre/post tool callbacks in [crates/core/src/hooks/mod.rs](/Users/raincandy_u/Dev/amadeus/crates/core/src/hooks/mod.rs#L21)
- shell hooks get:
  - stdin JSON payload with `event`, `tool_name`, `tool_input`, `tool_output`, `is_error`, `duration_ms`
  - env vars like `HOOK_EVENT`, `HOOK_TOOL_INPUT`, `HOOK_TOOL_OUTPUT` in [crates/core/src/hooks/shell.rs](/Users/raincandy_u/Dev/amadeus/crates/core/src/hooks/shell.rs#L167)
- outputs are weakly interpreted:
  - exit code `2` blocks
  - non-zero optionally blocks if `block_on_error`
  - stdout/stderr are mostly treated as log text
- no structured permission override or structured updated-input protocol

`claw-code-parity`:
- hooks build richer JSON payloads in [runtime/src/hooks.rs](/Users/raincandy_u/Dev/amadeus/refs/claw-code-parity/rust/crates/runtime/src/hooks.rs#L591)
- hook output parser supports structured fields in [runtime/src/hooks.rs](/Users/raincandy_u/Dev/amadeus/refs/claw-code-parity/rust/crates/runtime/src/hooks.rs#L560):
  - `additionalContext`
  - `permissionDecision`
  - `permissionDecisionReason`
  - `updatedInput`
- this effectively makes hooks part of the instruction/control surface, not only side-effect scripts

Comparison:
- Amadeus hooks are a shell callback system.
- `claw-code-parity` hooks are a structured policy-and-instruction extension surface.

Assessment:
- `claw-code-parity` is much stronger here.
- Amadeus should adopt a structured hook output contract if hooks are expected to shape agent behavior, not merely observe it.

### 7. Sub-Agent Prompting

Amadeus:
- main system prompt conditionally mentions `sub_agent` in [crates/prompts/src/lib.rs](/Users/raincandy_u/Dev/amadeus/crates/prompts/src/lib.rs#L20)
- sub-agent availability is mostly controlled by tool registration, not a specialized sub-agent system prompt

`claw-code-parity`:
- background agents use the assembled system prompt plus an extra sub-agent line in [tools/src/lib.rs](/Users/raincandy_u/Dev/amadeus/refs/claw-code-parity/rust/crates/tools/src/lib.rs#L3005)
- that line explicitly constrains behavior:
  - work only on delegated task
  - use only available tools
  - do not ask user questions
  - finish with concise result
- allowed tools are also narrowed by sub-agent type in [tools/src/lib.rs](/Users/raincandy_u/Dev/amadeus/refs/claw-code-parity/rust/crates/tools/src/lib.rs#L3021)

Comparison:
- Amadeus exposes sub-agents mainly as a capability.
- `claw-code-parity` shapes sub-agent behavior more explicitly at prompt time.

Assessment:
- `claw-code-parity` is stronger here.
- Amadeus should add dedicated sub-agent prompt overlays tied to profile/tool restrictions.

### 8. Assessment / Review Prompting

Amadeus:
- has a concrete assessment default prompt in [crates/core/src/assessment/mod.rs](/Users/raincandy_u/Dev/amadeus/crates/core/src/assessment/mod.rs#L220)
- it is surprisingly specific:
  - references `docs/TMUX_TEST_FLOW.md`
  - references runtime flows and targeted tests
  - requires read-only mode
  - requires confirmed bugs only

`claw-code-parity`:
- no equivalent dedicated assessment prompt was found in the reviewed runtime/tool files
- it relies more on generic runtime/system prompt plus richer tool surfaces

Comparison:
- Amadeus has a stronger specialized evaluation prompt.
- `claw-code-parity` is stronger in general-purpose runtime prompting.

Assessment:
- Amadeus should keep this specialization pattern and apply it more broadly.

## Side-By-Side Findings

### Where Amadeus Is Better

1. Compaction prompt quality
- The XML state snapshot prompt in [compaction.rs](/Users/raincandy_u/Dev/amadeus/crates/core/src/agent/compaction.rs#L76) is more explicit and safer than the average summarization prompt.

2. Specialized assessment prompting
- The assessment prompt in [assessment/mod.rs](/Users/raincandy_u/Dev/amadeus/crates/core/src/assessment/mod.rs#L220) is focused and operationally useful.

3. Simplicity
- The main prompt is easy to read and maintain.

### Where `claw-code-parity` Is Better

1. System prompt assembly
- It carries real runtime state into the prompt builder instead of relying on one static string.

2. Instruction-file handling
- It has ancestry discovery, dedupe, truncation, and config rendering.

3. Tool description breadth
- It gives the model a much richer action vocabulary.

4. Hook contract richness
- Hooks can actually redirect permission and input flow in a structured way.

5. Sub-agent instruction shaping
- Sub-agents get an explicit behavioral overlay, not just a delegated tool.

## Main Structural Gap In Amadeus

Amadeus’s strongest prompt is not its main prompt.

Right now:
- the compaction prompt is highly structured
- the system prompt is comparatively shallow
- profile prompts are separate
- tool descriptions are separate
- hook contracts are simple

That means the prompt stack does not feel designed as one coherent system.

`claw-code-parity`, by contrast, has a more coherent hierarchy:
1. build a system prompt from runtime state
2. give the model a broad and typed tool vocabulary
3. let hooks and config participate in runtime control
4. specialize sub-agents with targeted overlays

## Recommended Changes For Amadeus

### Priority 1: Replace the flat system prompt with a builder

Add a prompt builder that assembles:
- base system rules
- environment context
- project context
- instruction memory files
- permission mode summary
- tool profile summary
- optional profile overlay
- optional sub-agent overlay

This should replace the current single template in [crates/prompts/src/lib.rs](/Users/raincandy_u/Dev/amadeus/crates/prompts/src/lib.rs#L25) as the primary runtime prompt path.

### Priority 2: Unify profile prompts with runtime prompt assembly

The profile prompts in [crates/profiles/src/lib.rs](/Users/raincandy_u/Dev/amadeus/crates/profiles/src/lib.rs#L35) should become composable overlays, not a separate parallel system.

### Priority 3: Strengthen the hook contract

Extend shell hook output parsing so hooks can return structured data like:
- additional context
- permission override
- permission reason
- updated input

That would move hooks closer to parity with [runtime/src/hooks.rs](/Users/raincandy_u/Dev/amadeus/refs/claw-code-parity/rust/crates/runtime/src/hooks.rs#L560).

### Priority 4: Make tool descriptions more intentional

Now that Amadeus has a richer tool platform, the prompt layer should:
- standardize tool naming
- standardize description style
- encode planning heuristics consistently
- expose higher-level tool surfaces where available

### Priority 5: Add explicit sub-agent overlays

When a sub-agent is spawned, append an additional instruction block similar in spirit to [build_agent_system_prompt](/Users/raincandy_u/Dev/amadeus/refs/claw-code-parity/rust/crates/tools/src/lib.rs#L3005), with:
- delegated scope
- no user-question rule
- result-format expectation
- allowed-tool reminder

## Bottom Line

Amadeus currently has one excellent prompt surface, one decent prompt surface, and several underdeveloped ones:
- excellent: compaction
- decent: assessment
- underdeveloped: main system prompt, tool prompt surface, hook contract, sub-agent prompting

`claw-code-parity` is more mature as a prompt system:
- broader
- more stateful
- more configurable
- better aligned with its runtime behavior

If Amadeus wants prompt parity, the next step is not “make the system prompt longer.” The next step is to make the prompt architecture compositional and runtime-aware in the same way the tool platform is now becoming compositional and runtime-aware.
