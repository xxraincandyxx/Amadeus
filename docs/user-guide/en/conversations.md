# Conversations and context

The conversation is the working record for one agent session. User requests, final answers, reasoning disclosures, tool activity, and approval requests appear in chronological order.

## Thinking disclosures

When a provider exposes a separate reasoning stream, Amadeus shows a **Thought** row. Completed thinking is collapsed by default. Select the row to inspect it. Models that do not expose reasoning simply omit the disclosure.

## Markdown answers

Answers support headings, emphasis, links, lists, task lists, tables, blockquotes, inline code, and fenced code blocks. Code blocks include a copy action. Raw HTML is escaped.

## Context usage

Use `/context` to inspect message counts, pending approvals, token usage, and context percentage. Use `/compact` when a long conversation needs more space. Compaction summarizes older context while retaining recent work.

## Export a record

Use `/export markdown` for a readable manuscript or `/export json` for structured session data. Exporting does not modify the conversation.
