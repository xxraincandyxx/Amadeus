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
// - Completed live reasoning remains attached to its assistant turn after history refresh.
// - Responses without reasoning do not create duplicate placeholder entries.
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
  thinkingStartedAt: null,
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
  assert.equal(completed.timeline[0].durationSeconds, 1);
  assert.equal(completed.thinkingStartedAt, null);
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

test("missing model reasoning does not create a placeholder timeline entry", () => {
  const withText = reduceEvent(runtime, "text", { content: "A direct answer." });
  const completed = reduceEvent(withText, "done", {});

  assert.deepEqual(completed.timeline.map(({ kind, text }) => ({ kind, text })), [
    { kind: "assistant", text: "A direct answer." },
  ]);
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

test("two prompts keep reasoning attached to their corresponding answers", () => {
  const hydrated = [
    { id: "user-1", kind: "user", text: "First prompt" },
    { id: "answer-1", kind: "assistant", text: "First answer" },
    { id: "user-2", kind: "user", text: "Second prompt" },
    { id: "answer-2", kind: "assistant", text: "Second answer" },
  ];
  const firstThinking = { id: "thinking-1", kind: "thinking", text: "First reasoning", available: true };
  const secondThinking = { id: "thinking-2", kind: "thinking", text: "Second reasoning", available: true };
  const current = [
    hydrated[0],
    firstThinking,
    hydrated[1],
    hydrated[2],
    secondThinking,
    hydrated[3],
  ];

  assert.deepEqual(preserveThinkingTimeline(hydrated, current), [
    hydrated[0],
    firstThinking,
    hydrated[1],
    hydrated[2],
    secondThinking,
    hydrated[3],
  ]);
});

test("history refresh drops legacy unavailable-reasoning placeholders", () => {
  const hydrated = [
    { id: "user-1", kind: "user", text: "First prompt" },
    { id: "answer-1", kind: "assistant", text: "First answer" },
    { id: "user-2", kind: "user", text: "Second prompt" },
    { id: "answer-2", kind: "assistant", text: "Second answer" },
  ];
  const unavailable = {
    id: "thinking-unavailable",
    kind: "thinking",
    text: "The current model did not expose a separate reasoning channel for this response.",
    available: false,
  };

  assert.deepEqual(preserveThinkingTimeline(hydrated, [unavailable, ...hydrated, unavailable]), hydrated);
});
