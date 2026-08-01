// @amadeus-header
// summary: Verifies web session timeline reduction and reasoning separation behavior.
// layer: test
// status: test-only
// feature_flags: none
// provides:
// - cmd: npm test
// uses:
// - fn: historyToTimeline
// - fn: preserveThinkingTimeline
// - fn: reduceEvent
// invariants:
// - Reasoning never merges into final answer text.
// - Completed live reasoning remains available after history refresh.
// side_effects: none
// tests:
// - cmd: npm test
// @end-amadeus-header

import assert from "node:assert/strict";
import test from "node:test";

import { historyToTimeline, preserveThinkingTimeline, reduceEvent, splitTaggedThinking } from "./sessionState.js";

const runtime = {
  timeline: [],
  tools: {},
  streamingText: "",
  thinking: "",
  rawStreamingText: "",
  providerThinking: "",
  status: "running",
  tokenUsage: null,
  approvals: [],
};

test("history keeps reasoning separate from assistant text", () => {
  const timeline = historyToTimeline([
    {
      role: "assistant",
      content: [
        { type: "thinking", thinking: "Compare both values." },
        { type: "text", text: "9.9 is larger." },
      ],
    },
  ]);

  assert.deepEqual(timeline.map(({ kind, text }) => ({ kind, text })), [
    { kind: "thinking", text: "Compare both values." },
    { kind: "assistant", text: "9.9 is larger." },
  ]);
});

test("done commits streamed reasoning before the final answer", () => {
  const withThinking = reduceEvent(runtime, "thinking", { delta: "Inspect the inputs." });
  const withText = reduceEvent(withThinking, "text", { content: "The result is 42." });
  const completed = reduceEvent(withText, "done", {});

  assert.deepEqual(completed.timeline.map(({ kind, text }) => ({ kind, text })), [
    { kind: "thinking", text: "Inspect the inputs." },
    { kind: "assistant", text: "The result is 42." },
  ]);
  assert.equal(completed.thinking, "");
  assert.equal(completed.streamingText, "");
});

test("tagged thinking is separated from streamed answer text", () => {
  const first = reduceEvent(runtime, "text", { content: "<think>Compare " });
  const second = reduceEvent(first, "text", { content: "the terms.</think>Use **unity of opposites**." });
  const completed = reduceEvent(second, "done", {});

  assert.deepEqual(completed.timeline.map(({ kind, text, available }) => ({ kind, text, available })), [
    { kind: "thinking", text: "Compare the terms.", available: true },
    { kind: "assistant", text: "Use **unity of opposites**.", available: undefined },
  ]);
});

test("missing model reasoning is represented explicitly", () => {
  const withText = reduceEvent(runtime, "text", { content: "A direct answer." });
  const completed = reduceEvent(withText, "done", {});

  assert.equal(completed.timeline[0].kind, "thinking");
  assert.equal(completed.timeline[0].available, false);
  assert.match(completed.timeline[0].text, /did not expose a separate reasoning channel/);
});

test("tag parser keeps ordinary markdown untouched", () => {
  assert.deepEqual(splitTaggedThinking("### Heading\n\n**Bold**"), {
    answer: "### Heading\n\n**Bold**",
    thinking: "",
    tagged: false,
  });
});

test("history refresh preserves reasoning captured only by the live stream", () => {
  const hydrated = [{ id: "answer", kind: "assistant", text: "The result is 42." }];
  const current = [{ id: "reasoning", kind: "thinking", text: "Inspect the inputs." }];

  assert.deepEqual(preserveThinkingTimeline(hydrated, current), [current[0], hydrated[0]]);
});
