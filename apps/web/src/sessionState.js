// @amadeus-header
// summary: Reduces live session events and hydrates persisted history for the web timeline.
// layer: ui
// status: active
// feature_flags: none
// provides:
// - fn: contentText
// - fn: historyToTimeline
// - fn: preserveThinkingTimeline
// - fn: reduceEvent
// - fn: splitTaggedThinking
// uses:
// - protocol: Amadeus session SSE events
// - format: serialized message content blocks
// invariants:
// - Reasoning remains visually separate from final assistant text.
// - Completed live reasoning survives authoritative history refreshes for the current runtime.
// side_effects: none
// tests:
// - apps/web/src/sessionState.test.js
// @end-amadeus-header

export function contentText(content = []) {
  return content
    .filter((block) => block.type === "text")
    .map((block) => block.text || "")
    .join("\n")
    .trim();
}

export function historyToTimeline(messages = []) {
  return messages.flatMap((message, messageIndex) => {
    const items = [];
    const text = contentText(message.content);
    const thinking = (message.content || [])
      .filter((block) => block.type === "thinking")
      .map((block) => block.thinking || block.text || "")
      .join("\n")
      .trim();
    if (thinking) {
      items.push({ id: `history-thinking-${messageIndex}`, kind: "thinking", text: thinking, complete: true });
    }
    if (text) {
      items.push({ id: `history-${messageIndex}`, kind: message.role, text });
    }
    for (const [blockIndex, block] of (message.content || []).entries()) {
      if (block.type === "tool_use") {
        items.push({
          id: block.id || `history-tool-${messageIndex}-${blockIndex}`,
          kind: "tool",
          name: block.name,
          input: block.input,
          status: "complete",
        });
      }
      if (block.type === "tool_result") {
        const target = items.find((item) => item.id === block.tool_use_id);
        if (target) target.output = typeof block.content === "string" ? block.content : JSON.stringify(block.content);
      }
    }
    return items;
  });
}

export function preserveThinkingTimeline(hydrated = [], current = []) {
  const preserved = current.filter(
    (item) => item.kind === "thinking" && !hydrated.some((entry) => entry.kind === "thinking" && entry.text === item.text),
  );
  if (!preserved.length) return hydrated;
  const timeline = [...hydrated];
  const lastAssistant = timeline.findLastIndex((item) => item.kind === "assistant");
  timeline.splice(lastAssistant < 0 ? timeline.length : lastAssistant, 0, ...preserved);
  return timeline;
}

export function splitTaggedThinking(text = "") {
  const tagPattern = /<\/?think>/gi;
  let answer = "";
  let thinking = "";
  let cursor = 0;
  let insideThinking = false;
  let tagged = false;

  for (const match of text.matchAll(tagPattern)) {
    const segment = text.slice(cursor, match.index);
    if (insideThinking) thinking += segment;
    else answer += segment;
    insideThinking = !match[0].startsWith("</");
    tagged = true;
    cursor = match.index + match[0].length;
  }

  const remainder = text.slice(cursor);
  if (insideThinking) thinking += remainder;
  else answer += remainder;

  return { answer, thinking, tagged };
}

function joinThinking(providerThinking, taggedThinking) {
  return [providerThinking.trim(), taggedThinking.trim()].filter(Boolean).join("\n\n");
}

export function reduceEvent(state, eventName, payload) {
  const timeline = [...state.timeline];
  const tools = { ...state.tools };
  let streamingText = state.streamingText;
  let thinking = state.thinking;
  let rawStreamingText = state.rawStreamingText || "";
  let providerThinking = state.providerThinking || "";
  let status = state.status;
  let tokenUsage = state.tokenUsage;
  let approvals = state.approvals;

  if (eventName === "session_state") status = payload.status;
  if (eventName === "text") {
    rawStreamingText += payload.content || "";
    const parsed = splitTaggedThinking(rawStreamingText);
    streamingText = parsed.answer;
    thinking = joinThinking(providerThinking, parsed.thinking);
  }
  if (eventName === "thinking") {
    providerThinking += payload.delta || "";
    thinking = joinThinking(providerThinking, splitTaggedThinking(rawStreamingText).thinking);
  }
  if (eventName === "thinking_complete") {
    providerThinking = payload.thinking || providerThinking;
    thinking = joinThinking(providerThinking, splitTaggedThinking(rawStreamingText).thinking);
  }
  if (eventName === "tool_start") {
    tools[payload.id] = { ...payload, kind: "tool", status: "running", output: "", inputText: "" };
  }
  if (eventName === "tool_input" && tools[payload.id]) {
    tools[payload.id] = { ...tools[payload.id], inputText: `${tools[payload.id].inputText || ""}${payload.delta || ""}` };
  }
  if (eventName === "tool_output" && tools[payload.id]) {
    tools[payload.id] = { ...tools[payload.id], output: `${tools[payload.id].output || ""}${payload.delta || ""}` };
  }
  if (eventName === "tool_progress" && tools[payload.id]) {
    tools[payload.id] = { ...tools[payload.id], progress: payload.message, percent: payload.percent };
  }
  if (eventName === "tool_done") {
    const tool = { ...(tools[payload.id] || {}), ...payload, kind: "tool", status: payload.is_error ? "error" : "complete" };
    tools[payload.id] = tool;
    if (!timeline.some((item) => item.id === payload.id)) timeline.push(tool);
  }
  if (eventName === "approval_request") {
    approvals = [...approvals.filter((item) => item.id !== payload.id), payload];
  }
  if (eventName === "token_usage") tokenUsage = payload;
  if (eventName === "done") {
    timeline.push({
      id: `thinking-${Date.now()}`,
      kind: "thinking",
      text: thinking.trim() || "The current model did not expose a separate reasoning channel for this response.",
      complete: true,
      available: Boolean(thinking.trim()),
    });
    if (streamingText.trim()) timeline.push({ id: `assistant-${Date.now()}`, kind: "assistant", text: streamingText });
    streamingText = "";
    thinking = "";
    rawStreamingText = "";
    providerThinking = "";
    status = "completed";
  }
  if (eventName === "error") {
    timeline.push({ id: `error-${Date.now()}`, kind: "error", text: payload.message || payload.error || "The agent reported an unknown error." });
    status = "failed";
  }
  if (eventName === "compaction") {
    timeline.push({ id: `compaction-${Date.now()}`, kind: "notice", text: `Context compacted · ${payload.messages_summarized} messages summarized` });
  }

  return {
    ...state,
    timeline,
    tools,
    streamingText,
    thinking,
    rawStreamingText,
    providerThinking,
    status,
    tokenUsage,
    approvals,
  };
}
