// @amadeus-header
// summary: Builds line-oriented display diffs from file write and edit tool inputs.
// layer: ui
// status: active
// feature_flags: none
// provides:
// - fn: buildFileDiff
// uses:
// - format: Amadeus file tool input
// - library: diff line comparison
// invariants:
// - Added and removed lines retain their respective file line numbers.
// side_effects: none
// tests:
// - apps/web/src/fileDiff.test.js
// @end-amadeus-header

import { diffLines } from "diff";

function parseInput(input) {
  if (input && typeof input === "object" && !Array.isArray(input)) return input;
  if (typeof input !== "string" || !input.trim()) return null;

  try {
    const parsed = JSON.parse(input);
    return parsed && typeof parsed === "object" && !Array.isArray(parsed) ? parsed : null;
  } catch {
    return null;
  }
}

function chunkLines(value) {
  const normalized = value.replace(/\r\n?/g, "\n");
  const lines = normalized.split("\n");
  if (normalized.endsWith("\n")) lines.pop();
  return lines;
}

export function buildFileDiff(toolName, input) {
  const parsed = parseInput(input);
  if (!parsed || typeof parsed.path !== "string") return null;

  let oldText;
  let newText;
  if (toolName === "write_file" && typeof parsed.content === "string") {
    oldText = "";
    newText = parsed.content;
  } else if (
    toolName === "edit_file"
    && typeof parsed.old_text === "string"
    && typeof parsed.new_text === "string"
  ) {
    oldText = parsed.old_text;
    newText = parsed.new_text;
  } else {
    return null;
  }

  let oldLine = 1;
  let newLine = 1;
  let additions = 0;
  let deletions = 0;
  const lines = [];

  for (const change of diffLines(oldText, newText)) {
    const status = change.added ? "added" : change.removed ? "removed" : "unchanged";
    for (const content of chunkLines(change.value)) {
      const oldNumber = status === "added" ? null : oldLine++;
      const newNumber = status === "removed" ? null : newLine++;
      if (status === "added") additions += 1;
      if (status === "removed") deletions += 1;
      lines.push({ status, oldNumber, newNumber, content });
    }
  }

  return { path: parsed.path, additions, deletions, lines };
}
