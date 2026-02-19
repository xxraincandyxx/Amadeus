# Context Compaction System - Technical Design Document

## Table of Contents

1. [Overview](#overview)
2. [Architecture](#architecture)
3. [When Compaction Triggers](#when-compaction-triggers)
4. [Prompt Design](#prompt-design)
5. [Processing Flow](#processing-flow)
6. [Key Algorithms](#key-algorithms)
7. [Configuration](#configuration)
8. [Edge Cases & Error Handling](#edge-cases--error-handling)
9. [Comparison with Other Systems](#comparison-with-other-systems)
10. [Best Practices](#best-practices)

---

## Overview

Context compaction is a mechanism to manage LLM context window limits in long-running sessions. When a conversation grows beyond the model's context capacity, the system automatically summarizes older messages into a structured summary while preserving recent conversation intact.

### Goals

- **Prevent context overflow**: Keep sessions running indefinitely without hitting token limits
- **Preserve critical context**: Retain goals, decisions, progress, and file operations
- **Minimize information loss**: Use structured summarization to capture essential details
- **Support incremental updates**: Build upon previous summaries rather than re-summarizing everything

### Key Metrics

| Metric | Value |
|--------|-------|
| Default reserve tokens | 16,384 |
| Default keep recent tokens | 20,000 |
| Safety margin | 1.2x (20% buffer) |
| Base chunk ratio | 40% of context window |
| Minimum chunk ratio | 15% of context window |

---

## Architecture

### Component Overview

```
┌─────────────────────────────────────────────────────────────┐
│                     Session Manager                          │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────┐ │
│  │ Session History │  │ Token Tracker   │  │ Compaction  │ │
│  │ (JSONL)         │  │                 │  │ Scheduler   │ │
│  └────────┬────────┘  └────────┬────────┘  └──────┬──────┘ │
└───────────┼────────────────────┼─────────────────┼─────────┘
            │                    │                 │
            ▼                    ▼                 ▼
┌─────────────────────────────────────────────────────────────┐
│                    Compaction Engine                         │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐  │
│  │ Cut Point   │  │ Summarizer  │  │ File Operations     │  │
│  │ Detector    │  │ (LLM-based) │  │ Extractor           │  │
│  └─────────────┘  └─────────────┘  └─────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
            │
            ▼
┌─────────────────────────────────────────────────────────────┐
│                    Compaction Result                         │
│  • Structured summary                                        │
│  • File operations (read/modified)                           │
│  • Tool failures log                                         │
│  • First kept entry ID                                       │
└─────────────────────────────────────────────────────────────┘
```

### Data Flow

```
Session Entries (JSONL)
       │
       ▼
┌──────────────────┐
│ Should Compact?  │ ◄─── contextTokens > window - reserve
└────────┬─────────┘
         │ Yes
         ▼
┌──────────────────┐
│ Find Cut Point   │ ◄─── Keep recent 20K tokens
└────────┬─────────┘
         │
         ▼
┌──────────────────┐
│ Split Messages   │ ◄─── toSummarize | toKeep
└────────┬─────────┘
         │
         ▼
┌──────────────────┐     ┌──────────────────┐
│ Oversized?       │────►│ Prune & Summarize│
└────────┬─────────┘     │ Dropped Chunks   │
         │ No            └────────┬─────────┘
         ▼                        │
┌──────────────────┐              │
│ Summarize Stages │◄─────────────┘
└────────┬─────────┘
         │
         ▼
┌──────────────────┐
│ Extract File Ops │
└────────┬─────────┘
         │
         ▼
┌──────────────────┐
│ Write Compaction │
│ Entry to JSONL   │
└──────────────────┘
```

---

## When Compaction Triggers

### Condition Check

```typescript
function shouldCompact(
  contextTokens: number,
  contextWindow: number,
  settings: CompactionSettings
): boolean {
  if (!settings.enabled) return false;
  return contextTokens > contextWindow - settings.reserveTokens;
}
```

### Token Estimation

The system uses a conservative `chars / 4` heuristic for token estimation:

```typescript
function estimateTokens(message: AgentMessage): number {
  let chars = 0;
  
  switch (message.role) {
    case "user":
      chars = countUserContentChars(message.content);
      break;
    case "assistant":
      chars = countAssistantContentChars(message.content);
      // Includes thinking blocks and tool calls
      break;
    case "toolResult":
      chars = countToolResultChars(message.content);
      // Images estimated as 4800 chars (~1200 tokens)
      break;
    // ... other message types
  }
  
  return Math.ceil(chars / 4);
}
```

### Context Token Calculation

```typescript
function calculateContextTokens(usage: Usage): number {
  return usage.totalTokens || 
         usage.input + usage.output + usage.cacheRead + usage.cacheWrite;
}

function estimateContextTokens(messages: AgentMessage[]): ContextTokens {
  const lastUsage = findLastAssistantUsage(messages);
  
  if (!lastUsage) {
    // No usage data, estimate from scratch
    return { tokens: estimateAllMessages(messages) };
  }
  
  // Use actual usage + estimate trailing messages
  const usageTokens = calculateContextTokens(lastUsage.usage);
  const trailingTokens = estimateMessages(messages.slice(lastUsage.index + 1));
  
  return {
    tokens: usageTokens + trailingTokens,
    usageTokens,
    trailingTokens
  };
}
```

---

## Prompt Design

### System Prompt

```
You are a context summarization assistant. Your task is to read a 
conversation between a user and an AI coding assistant, then produce 
a structured summary following the exact format specified.

Do NOT continue the conversation. Do NOT respond to any questions 
in the conversation. ONLY output the structured summary.
```

### Initial Summarization Prompt

```
The messages above are a conversation to summarize. Create a 
structured context checkpoint summary that another LLM will use 
to continue the work.

Use this EXACT format:

## Goal
[What is the user trying to accomplish? Can be multiple items 
if the session covers different tasks.]

## Constraints & Preferences
- [Any constraints, preferences, or requirements mentioned by user]
- [Or "(none)" if none were mentioned]

## Progress
### Done
- [x] [Completed tasks/changes]

### In Progress
- [ ] [Current work]

### Blocked
- [Issues preventing progress, if any]

## Key Decisions
- **[Decision]**: [Brief rationale]

## Next Steps
1. [Ordered list of what should happen next]

## Critical Context
- [Any data, examples, or references needed to continue]
- [Or "(none)" if not applicable]

Keep each section concise. Preserve exact file paths, function 
names, and error messages.
```

### Incremental Update Prompt

```
The messages above are NEW conversation messages to incorporate 
into the existing summary provided in <previous-summary> tags.

Update the existing structured summary with new information. RULES:
- PRESERVE all existing information from the previous summary
- ADD new progress, decisions, and context from the new messages
- UPDATE the Progress section: move items from "In Progress" to 
  "Done" when completed
- UPDATE "Next Steps" based on what was accomplished
- PRESERVE exact file paths, function names, and error messages
- If something is no longer relevant, you may remove it

Use this EXACT format:
[Same format as initial prompt]
```

### Turn Prefix Summarization Prompt

When compaction cuts in the middle of a turn (between a user message and the end of the assistant's response):

```
This is the PREFIX of a turn that was too large to keep. The SUFFIX 
(recent work) is retained.

Summarize the prefix to provide context for the retained suffix:

## Original Request
[What did the user ask for in this turn?]

## Early Progress
- [Key decisions and work done in the prefix]

## Context for Suffix
- [Information needed to understand the retained recent work]

Be concise. Focus on what's needed to understand the kept suffix.
```

### Merge Summaries Prompt

When multiple partial summaries need to be merged:

```
Merge these partial summaries into a single cohesive summary. 
Preserve decisions, TODOs, open questions, and any constraints.
```

### Prompt Construction

```typescript
async function generateSummary(
  messages: AgentMessage[],
  model: Model,
  reserveTokens: number,
  apiKey: string,
  signal: AbortSignal,
  customInstructions?: string,
  previousSummary?: string
): Promise<string> {
  const maxTokens = Math.floor(0.8 * reserveTokens);
  
  // Select appropriate prompt
  let basePrompt = previousSummary 
    ? UPDATE_SUMMARIZATION_PROMPT 
    : SUMMARIZATION_PROMPT;
  
  // Add custom instructions if provided
  if (customInstructions) {
    basePrompt = `${basePrompt}\n\nAdditional focus: ${customInstructions}`;
  }
  
  // Serialize conversation to prevent continuation
  const conversationText = serializeConversation(messages);
  
  // Build prompt with XML tags
  let promptText = `<conversation>\n${conversationText}\n</conversation>\n\n`;
  
  if (previousSummary) {
    promptText += `<previous-summary>\n${previousSummary}\n</previous-summary>\n\n`;
  }
  
  promptText += basePrompt;
  
  // Call LLM with high reasoning
  const response = await completeSimple(
    model,
    { 
      systemPrompt: SUMMARIZATION_SYSTEM_PROMPT, 
      messages: [{ role: "user", content: promptText }] 
    },
    { maxTokens, signal, apiKey, reasoning: "high" }
  );
  
  return extractTextContent(response);
}
```

---

## Processing Flow

### Complete Algorithm

```typescript
async function compact(
  sessionEntries: SessionEntry[],
  model: Model,
  apiKey: string,
  customInstructions?: string,
  signal?: AbortSignal
): Promise<CompactionResult> {
  
  // ========================================
  // PHASE 1: Preparation
  // ========================================
  
  const preparation = prepareCompaction(sessionEntries, settings);
  if (!preparation) return; // No compaction needed
  
  const {
    firstKeptEntryId,
    messagesToSummarize,
    turnPrefixMessages,
    isSplitTurn,
    tokensBefore,
    previousSummary,
    fileOps,
    settings
  } = preparation;
  
  // ========================================
  // PHASE 2: Safeguard Check
  // ========================================
  
  // Check if new content is large enough to need pruning
  const summarizableTokens = estimateTokens(messagesToSummarize) + 
                             estimateTokens(turnPrefixMessages);
  const newContentTokens = tokensBefore - summarizableTokens;
  const maxHistoryTokens = contextWindow * maxHistoryShare * SAFETY_MARGIN;
  
  let droppedSummary: string | undefined;
  let currentMessages = messagesToSummarize;
  
  if (newContentTokens > maxHistoryTokens) {
    // Need to prune older messages
    const pruned = pruneHistoryForContextShare({
      messages: messagesToSummarize,
      maxContextTokens: contextWindow,
      maxHistoryShare: 0.5,
      parts: 2
    });
    
    if (pruned.droppedChunks > 0) {
      console.warn(`Dropped ${pruned.droppedChunks} older chunk(s) ` +
                   `(${pruned.droppedMessages} messages)`);
      
      currentMessages = pruned.messages;
      
      // Summarize dropped messages
      if (pruned.droppedMessagesList.length > 0) {
        droppedSummary = await summarizeInStages({
          messages: pruned.droppedMessagesList,
          model,
          apiKey,
          signal,
          // ... other params
        });
      }
    }
  }
  
  // ========================================
  // PHASE 3: Multi-Stage Summarization
  // ========================================
  
  const historySummary = await summarizeInStages({
    messages: currentMessages,
    model,
    apiKey,
    signal,
    reserveTokens: settings.reserveTokens,
    maxChunkTokens: contextWindow * computeAdaptiveChunkRatio(currentMessages),
    contextWindow,
    customInstructions,
    previousSummary: droppedSummary ?? previousSummary
  });
  
  // ========================================
  // PHASE 4: Handle Split Turn
  // ========================================
  
  let summary = historySummary;
  
  if (isSplitTurn && turnPrefixMessages.length > 0) {
    const turnPrefixSummary = await summarizeInStages({
      messages: turnPrefixMessages,
      model,
      apiKey,
      signal,
      customInstructions: TURN_PREFIX_INSTRUCTIONS
    });
    
    summary = `${historySummary}\n\n---\n\n` +
              `**Turn Context (split turn):**\n\n${turnPrefixSummary}`;
  }
  
  // ========================================
  // PHASE 5: Extract & Append File Operations
  // ========================================
  
  const { readFiles, modifiedFiles } = computeFileLists(fileOps);
  
  summary += formatToolFailuresSection(toolFailures);
  summary += formatFileOperations(readFiles, modifiedFiles);
  
  // ========================================
  // PHASE 6: Return Result
  // ========================================
  
  return {
    summary,
    firstKeptEntryId,
    tokensBefore,
    details: { readFiles, modifiedFiles }
  };
}
```

### Message Serialization

Messages are serialized to text format to prevent the model from continuing the conversation:

```typescript
function serializeConversation(messages: LlmMessage[]): string {
  const parts: string[] = [];
  
  for (const msg of messages) {
    if (msg.role === "user") {
      parts.push(`[User]: ${extractText(msg.content)}`);
    } 
    else if (msg.role === "assistant") {
      // Handle thinking blocks
      const thinking = extractThinking(msg.content);
      if (thinking) parts.push(`[Assistant thinking]: ${thinking}`);
      
      // Handle text content
      const text = extractText(msg.content);
      if (text) parts.push(`[Assistant]: ${text}`);
      
      // Handle tool calls
      const toolCalls = extractToolCalls(msg.content);
      if (toolCalls.length > 0) {
        const formatted = toolCalls.map(tc => 
          `${tc.name}(${formatArgs(tc.arguments)})`
        ).join("; ");
        parts.push(`[Assistant tool calls]: ${formatted}`);
      }
    }
    else if (msg.role === "toolResult") {
      parts.push(`[Tool result]: ${extractText(msg.content)}`);
    }
  }
  
  return parts.join("\n\n");
}
```

---

## Key Algorithms

### 1. Cut Point Detection

```typescript
function findCutPoint(
  entries: SessionEntry[],
  startIndex: number,
  endIndex: number,
  keepRecentTokens: number
): CutPointResult {
  // Find all valid cut points (user/assistant, never toolResult)
  const cutPoints = findValidCutPoints(entries, startIndex, endIndex);
  
  if (cutPoints.length === 0) {
    return { 
      firstKeptEntryIndex: startIndex, 
      turnStartIndex: -1, 
      isSplitTurn: false 
    };
  }
  
  // Walk backwards, accumulating tokens
  let accumulatedTokens = 0;
  let cutIndex = cutPoints[0];
  
  for (let i = endIndex - 1; i >= startIndex; i--) {
    const entry = entries[i];
    if (entry.type !== "message") continue;
    
    accumulatedTokens += estimateTokens(entry.message);
    
    if (accumulatedTokens >= keepRecentTokens) {
      // Find closest valid cut point at or after this entry
      cutIndex = cutPoints.find(cp => cp >= i) ?? cutPoints[0];
      break;
    }
  }
  
  // Determine if this is a split turn
  const cutEntry = entries[cutIndex];
  const isUserMessage = cutEntry.type === "message" && 
                        cutEntry.message.role === "user";
  
  const turnStartIndex = isUserMessage 
    ? -1 
    : findTurnStartIndex(entries, cutIndex, startIndex);
  
  return {
    firstKeptEntryIndex: cutIndex,
    turnStartIndex,
    isSplitTurn: !isUserMessage && turnStartIndex !== -1
  };
}

function findValidCutPoints(
  entries: SessionEntry[],
  startIndex: number,
  endIndex: number
): number[] {
  const cutPoints: number[] = [];
  
  for (let i = startIndex; i < endIndex; i++) {
    const entry = entries[i];
    
    if (entry.type === "message") {
      const role = entry.message.role;
      // Valid cut points: user, assistant, custom, bashExecution
      // Invalid: toolResult (must follow its tool call)
      if (["user", "assistant", "custom", "bashExecution", 
            "branchSummary", "compactionSummary"].includes(role)) {
        cutPoints.push(i);
      }
    }
    else if (entry.type === "branch_summary" || entry.type === "custom_message") {
      cutPoints.push(i);
    }
  }
  
  return cutPoints;
}
```

### 2. Adaptive Chunk Ratio

```typescript
const BASE_CHUNK_RATIO = 0.4;   // 40% of context window
const MIN_CHUNK_RATIO = 0.15;   // 15% minimum
const SAFETY_MARGIN = 1.2;      // 20% buffer

function computeAdaptiveChunkRatio(
  messages: AgentMessage[],
  contextWindow: number
): number {
  if (messages.length === 0) return BASE_CHUNK_RATIO;
  
  const totalTokens = estimateMessagesTokens(messages);
  const avgTokens = totalTokens / messages.length;
  
  // Apply safety margin
  const safeAvgTokens = avgTokens * SAFETY_MARGIN;
  const avgRatio = safeAvgTokens / contextWindow;
  
  // If average message > 10% of context, reduce chunk ratio
  if (avgRatio > 0.1) {
    const reduction = Math.min(
      avgRatio * 2, 
      BASE_CHUNK_RATIO - MIN_CHUNK_RATIO
    );
    return Math.max(MIN_CHUNK_RATIO, BASE_CHUNK_RATIO - reduction);
  }
  
  return BASE_CHUNK_RATIO;
}
```

### 3. Pruning for Context Share

```typescript
function pruneHistoryForContextShare(params: {
  messages: AgentMessage[];
  maxContextTokens: number;
  maxHistoryShare?: number;
  parts?: number;
}): PruneResult {
  const maxHistoryShare = params.maxHistoryShare ?? 0.5;
  const budgetTokens = Math.floor(params.maxContextTokens * maxHistoryShare);
  
  let keptMessages = params.messages;
  const allDroppedMessages: AgentMessage[] = [];
  let droppedChunks = 0;
  let droppedMessages = 0;
  let droppedTokens = 0;
  
  const parts = params.parts ?? 2;
  
  // Keep dropping oldest chunks until within budget
  while (keptMessages.length > 0 && 
         estimateMessagesTokens(keptMessages) > budgetTokens) {
    
    const chunks = splitMessagesByTokenShare(keptMessages, parts);
    if (chunks.length <= 1) break;
    
    // Drop the oldest chunk
    const [dropped, ...rest] = chunks;
    const flatRest = rest.flat();
    
    // Repair tool_use/tool_result pairing
    // (remove orphaned tool_results whose tool_use was dropped)
    const repairReport = repairToolUseResultPairing(flatRest);
    const repairedKept = repairReport.messages;
    
    droppedChunks += 1;
    droppedMessages += dropped.length + repairReport.droppedOrphanCount;
    droppedTokens += estimateMessagesTokens(dropped);
    
    allDroppedMessages.push(...dropped);
    keptMessages = repairedKept;
  }
  
  return {
    messages: keptMessages,
    droppedMessagesList: allDroppedMessages,
    droppedChunks,
    droppedMessages,
    droppedTokens,
    keptTokens: estimateMessagesTokens(keptMessages),
    budgetTokens
  };
}
```

### 4. Tool Use/Result Pairing Repair

```typescript
function repairToolUseResultPairing(
  messages: AgentMessage[]
): { messages: AgentMessage[]; droppedOrphanCount: number } {
  // Collect all tool_use IDs in kept messages
  const toolUseIds = new Set<string>();
  
  for (const msg of messages) {
    if (msg.role === "assistant" && Array.isArray(msg.content)) {
      for (const block of msg.content) {
        if (block.type === "toolCall") {
          toolUseIds.add(block.id);
        }
      }
    }
  }
  
  // Remove orphaned tool_results
  const repaired: AgentMessage[] = [];
  let orphanCount = 0;
  
  for (const msg of messages) {
    if (msg.role === "toolResult") {
      if (toolUseIds.has(msg.toolCallId)) {
        repaired.push(msg);
      } else {
        orphanCount++;
        // Orphaned - don't include
      }
    } else {
      repaired.push(msg);
    }
  }
  
  return { messages: repaired, droppedOrphanCount: orphanCount };
}
```

### 5. Multi-Stage Summarization

```typescript
async function summarizeInStages(params: {
  messages: AgentMessage[];
  model: Model;
  apiKey: string;
  signal: AbortSignal;
  reserveTokens: number;
  maxChunkTokens: number;
  contextWindow: number;
  customInstructions?: string;
  previousSummary?: string;
  parts?: number;
  minMessagesForSplit?: number;
}): Promise<string> {
  
  const { messages } = params;
  if (messages.length === 0) {
    return params.previousSummary ?? "No prior history.";
  }
  
  const minMessagesForSplit = params.minMessagesForSplit ?? 4;
  const totalTokens = estimateMessagesTokens(messages);
  
  // Check if we can summarize in one pass
  if (messages.length < minMessagesForSplit || 
      totalTokens <= params.maxChunkTokens) {
    return summarizeWithFallback(params);
  }
  
  // Split into chunks
  const splits = splitMessagesByTokenShare(messages, params.parts ?? 2)
    .filter(chunk => chunk.length > 0);
  
  if (splits.length <= 1) {
    return summarizeWithFallback(params);
  }
  
  // Summarize each chunk
  const partialSummaries: string[] = [];
  for (const chunk of splits) {
    const summary = await summarizeWithFallback({
      ...params,
      messages: chunk,
      previousSummary: undefined
    });
    partialSummaries.push(summary);
  }
  
  if (partialSummaries.length === 1) {
    return partialSummaries[0];
  }
  
  // Merge partial summaries
  const summaryMessages = partialSummaries.map(summary => ({
    role: "user",
    content: summary,
    timestamp: Date.now()
  }));
  
  const mergeInstructions = params.customInstructions
    ? `${MERGE_SUMMARIES_INSTRUCTIONS}\n\nAdditional focus:\n${params.customInstructions}`
    : MERGE_SUMMARIES_INSTRUCTIONS;
  
  return summarizeWithFallback({
    ...params,
    messages: summaryMessages,
    customInstructions: mergeInstructions
  });
}
```

### 6. Summarization with Fallback

```typescript
async function summarizeWithFallback(params: {
  messages: AgentMessage[];
  model: Model;
  apiKey: string;
  signal: AbortSignal;
  reserveTokens: number;
  maxChunkTokens: number;
  contextWindow: number;
  customInstructions?: string;
  previousSummary?: string;
}): Promise<string> {
  
  const { messages, contextWindow } = params;
  
  if (messages.length === 0) {
    return params.previousSummary ?? "No prior history.";
  }
  
  // Try full summarization
  try {
    return await summarizeChunks(params);
  } catch (fullError) {
    console.warn(`Full summarization failed, trying partial: ${fullError}`);
  }
  
  // Fallback 1: Summarize only small messages
  const smallMessages: AgentMessage[] = [];
  const oversizedNotes: string[] = [];
  
  for (const msg of messages) {
    if (isOversizedForSummary(msg, contextWindow)) {
      const role = msg.role ?? "message";
      const tokens = estimateTokens(msg);
      oversizedNotes.push(
        `[Large ${role} (~${Math.round(tokens / 1000)}K tokens) omitted from summary]`
      );
    } else {
      smallMessages.push(msg);
    }
  }
  
  if (smallMessages.length > 0) {
    try {
      const partialSummary = await summarizeChunks({
        ...params,
        messages: smallMessages
      });
      const notes = oversizedNotes.length > 0 
        ? `\n\n${oversizedNotes.join("\n")}` 
        : "";
      return partialSummary + notes;
    } catch (partialError) {
      console.warn(`Partial summarization also failed: ${partialError}`);
    }
  }
  
  // Final fallback
  return `Context contained ${messages.length} messages ` +
         `(${oversizedNotes.length} oversized). Summary unavailable.`;
}

function isOversizedForSummary(
  msg: AgentMessage, 
  contextWindow: number
): boolean {
  const tokens = estimateTokens(msg) * SAFETY_MARGIN;
  return tokens > contextWindow * 0.5;
}
```

---

## Configuration

### Default Settings

```typescript
const DEFAULT_COMPACTION_SETTINGS = {
  enabled: true,
  reserveTokens: 16384,      // Keep 16K tokens free
  keepRecentTokens: 20000,   // Keep last 20K tokens intact
};
```

### Configuration Schema

```typescript
interface CompactionSettings {
  /** Enable/disable auto-compaction */
  enabled: boolean;
  
  /** Tokens to keep free in context window */
  reserveTokens: number;
  
  /** Recent tokens to keep without summarization */
  keepRecentTokens: number;
  
  /** Maximum share of context for history (default 0.5) */
  maxHistoryShare?: number;
  
  /** Custom instructions for summarization */
  customInstructions?: string;
}
```

### Configuration in OpenClaw

```json
{
  "agents": {
    "defaults": {
      "compaction": {
        "enabled": true,
        "reserveTokens": 16384,
        "keepRecentTokens": 20000
      }
    }
  }
}
```

---

## Edge Cases & Error Handling

### 1. Orphaned Tool Results

**Problem**: When a chunk containing a `tool_use` is dropped, the corresponding `tool_result` in a kept chunk becomes orphaned.

**Solution**: 
```typescript
// After dropping a chunk, scan kept messages for orphaned tool_results
// Remove them to prevent Anthropic API errors
const repairReport = repairToolUseResultPairing(keptMessages);
```

**Example**:
```
Before drop:
  [1] assistant: tool_use(id="call_123")  <-- in dropped chunk
  [2] toolResult: toolCallId="call_123"   <-- becomes orphaned

After repair:
  [2] is removed (orphaned)
```

### 2. Split Turn Handling

**Problem**: Compaction may cut in the middle of a turn (between user message and end of assistant response).

**Solution**: Generate a separate summary for the turn prefix:
```typescript
if (isSplitTurn && turnPrefixMessages.length > 0) {
  const turnPrefixSummary = await generateTurnPrefixSummary(turnPrefixMessages);
  finalSummary = `${historySummary}\n\n---\n\n**Turn Context:**\n\n${turnPrefixSummary}`;
}
```

### 3. Oversized Messages

**Problem**: A single message may be too large to summarize (> 50% of context).

**Solution**: Use fallback summarization:
```typescript
// Skip oversized messages, note their existence
if (isOversizedForSummary(msg, contextWindow)) {
  oversizedNotes.push(`[Large ${role} (~${tokens}K tokens) omitted]`);
}
```

### 4. Summarization Failure

**Problem**: LLM summarization may fail due to API errors or timeout.

**Solution**: Multiple fallback layers:
```typescript
try {
  return await fullSummarization();
} catch {
  try {
    return await partialSummarization(smallMessages);
  } catch {
    return fallbackSummary();  // "Summary unavailable due to size limits"
  }
}
```

### 5. Context Window Estimation Error

**Problem**: Token estimation uses chars/4 heuristic which may be inaccurate.

**Solution**: Apply 20% safety margin:
```typescript
const SAFETY_MARGIN = 1.2;
const safeTokens = estimatedTokens * SAFETY_MARGIN;
```

---

## Comparison with Other Systems

### OpenClaw vs Claude Code

| Feature | OpenClaw | Claude Code |
|---------|----------|-------------|
| Auto-compaction | ✅ | ✅ |
| Manual `/compact` | ✅ | ✅ |
| Partial compaction (rewind) | ❌ | ✅ |
| Memory flush before compact | ✅ | ❌ |
| Tool result pruning | ✅ | ❌ |
| Custom compaction instructions | ❌ | ✅ (CLAUDE.md) |
| Split turn handling | ✅ | ✅ |
| File operations tracking | ✅ | ❌ |
| Tool failures logging | ✅ | ❌ |

### Key Differences

1. **Memory Integration**: OpenClaw flushes important context to MEMORY.md before compaction
2. **Tool Failures**: OpenClaw tracks and includes failed tool calls in summary
3. **File Operations**: OpenClaw explicitly tracks read/modified files
4. **Custom Instructions**: Claude Code allows per-project compaction hints via CLAUDE.md

---

## Best Practices

### 1. Context Hygiene

```
/clear between unrelated tasks
```

Avoid mixing multiple unrelated tasks in one session. Context pollution degrades performance.

### 2. Corrective Action Limit

```
If correction fails twice, /clear and rewrite the prompt
```

After 2 failed corrections, the context is too polluted. Start fresh with a better prompt.

### 3. Use Subagents for Exploration

```
Use a subagent to investigate how auth works
```

Subagents have isolated context. They don't pollute the main session.

### 4. CLAUDE.md / MEMORY.md

Keep these files concise. Bloated instruction files cause the model to ignore rules.

### 5. Verification Always

Provide test commands, linters, or verification scripts. Without verification, the model can't self-correct.

### 6. Custom Compaction Instructions (Future Feature)

```markdown
<!-- In CLAUDE.md -->
When compacting, always preserve:
- Full list of modified files
- All test commands run
- Any TODO comments added
```

---

## Appendix: Data Structures

### Session Entry Types

```typescript
type SessionEntry = 
  | { type: "message"; message: AgentMessage; id: string }
  | { type: "compaction"; summary: string; firstKeptEntryId: string; tokensBefore: number; details?: CompactionDetails }
  | { type: "branch_summary"; summary: string; fromId: string }
  | { type: "custom_message"; customType: string; content: unknown }
  | { type: "model_change"; from: string; to: string }
  | { type: "thinking_level_change"; level: string }
  | { type: "label"; name: string };
```

### Agent Message Types

```typescript
type AgentMessage = 
  | UserMessage
  | AssistantMessage
  | ToolResultMessage
  | BashExecutionMessage
  | CustomMessage
  | BranchSummaryMessage
  | CompactionSummaryMessage;
```

### Compaction Result

```typescript
interface CompactionResult {
  summary: string;
  firstKeptEntryId: string;
  tokensBefore: number;
  details?: {
    readFiles: string[];
    modifiedFiles: string[];
  };
}
```

---

## References

- OpenClaw Source: `/src/agents/compaction.ts`
- Compaction Safeguard: `/src/agents/pi-extensions/compaction-safeguard.ts`
- Test Suite: `/src/agents/compaction.test.ts`
- Claude Code Docs: https://code.claude.com/docs/en/best-practices
