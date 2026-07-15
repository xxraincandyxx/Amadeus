import React, { useEffect, useMemo, useState } from "react";
import { createRoot } from "react-dom/client";

const e = React.createElement;

const samples = {
  writing:
    "Evaluate this IELTS Writing Task 2 response and give concise band-style feedback: Some people think online learning is better than classroom learning. Discuss both views and give your opinion. Online learning is very popular today because students can study at home and save time. It is also cheaper for many families. However, classroom learning helps students talk with teachers and classmates directly, so they can understand difficult ideas more quickly. In my opinion, online learning is useful, but classroom learning is better for young students because they need discipline and communication.",
  speaking:
    "Start an IELTS Speaking Part 2 practice session. Give me a cue card about a useful skill I learned, then ask one follow-up question.",
  rubric: "Print the IELTS practice scoring rubric you will use.",
};

const modes = [
  { id: "writing", label: "Writing", description: "Task response, cohesion, vocabulary, grammar" },
  { id: "speaking", label: "Speaking", description: "Cue cards, follow-ups, oral fluency" },
  { id: "rubric", label: "Rubric", description: "Practice scoring criteria" },
];

function App() {
  const [mode, setMode] = useState("writing");
  const [prompt, setPrompt] = useState(samples.writing);
  const [maxTurns, setMaxTurns] = useState(2);
  const [model, setModel] = useState("Model pending");
  const [result, setResult] = useState(null);
  const [error, setError] = useState("");
  const [isLoading, setIsLoading] = useState(false);

  const activeMode = useMemo(
    () => modes.find((item) => item.id === mode) || modes[0],
    [mode],
  );

  useEffect(() => {
    let cancelled = false;
    fetch("/api/health")
      .then((response) => response.json())
      .then((health) => {
        if (!cancelled) {
          setModel(health.model || "Model ready");
        }
      })
      .catch(() => {
        if (!cancelled) {
          setModel("Model unavailable");
        }
      });
    return () => {
      cancelled = true;
    };
  }, []);

  function chooseMode(nextMode) {
    setMode(nextMode);
    setPrompt(samples[nextMode] || samples.writing);
    setResult(null);
    setError("");
  }

  async function submitExam(event) {
    event.preventDefault();
    const trimmed = prompt.trim();
    if (!trimmed) {
      setError("Prompt cannot be empty.");
      setResult(null);
      return;
    }

    setIsLoading(true);
    setError("");
    try {
      const response = await fetch("/api/exam", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          mode,
          prompt: trimmed,
          max_turns: Number(maxTurns) || 2,
        }),
      });
      const data = await response.json();
      if (!response.ok) {
        throw new Error(data.error || "The examiner request failed.");
      }
      setResult(data);
      setModel(data.model);
    } catch (requestError) {
      setError(requestError.message);
      setResult(null);
    } finally {
      setIsLoading(false);
    }
  }

  return e(
    "main",
    { className: "page-shell" },
    e(Header, { model }),
    e(
      "section",
      { className: "exam-layout" },
      e(Composer, {
        activeMode,
        error,
        isLoading,
        maxTurns,
        mode,
        prompt,
        setMaxTurns,
        setPrompt,
        chooseMode,
        submitExam,
      }),
      e(ResultPane, { activeMode, error, isLoading, result }),
    ),
  );
}

function Header({ model }) {
  return e(
    "header",
    { className: "hero" },
    e(
      "div",
      { className: "eyebrow-row" },
      e("span", null, "Amadeus example"),
      e("span", null, "Kami-inspired study paper"),
    ),
    e(
      "div",
      { className: "hero-grid" },
      e(
        "div",
        null,
        e("h1", null, "IELTS Examinator"),
        e(
          "p",
          { className: "lede" },
          "A quiet practice desk for speaking prompts, writing feedback, and band-style review.",
        ),
      ),
      e(
        "aside",
        { className: "model-card" },
        e("span", { className: "label" }, "Current model"),
        e("strong", null, model),
      ),
    ),
  );
}

function Composer(props) {
  return e(
    "form",
    { className: "paper-panel composer", onSubmit: props.submitExam },
    e("h2", { className: "section-title" }, "Candidate input"),
    e(
      "div",
      { className: "mode-list", role: "radiogroup", "aria-label": "Exam mode" },
      modes.map((item) =>
        e(
          "button",
          {
            "aria-checked": props.mode === item.id,
            className: props.mode === item.id ? "mode-card active" : "mode-card",
            key: item.id,
            onClick: () => props.chooseMode(item.id),
            role: "radio",
            type: "button",
          },
          e("span", null, item.label),
          e("small", null, item.description),
        ),
      ),
    ),
    e(
      "label",
      { className: "field-block" },
      e("span", null, props.activeMode.label === "Speaking" ? "Examiner instruction" : "Response text"),
      e("textarea", {
        value: props.prompt,
        onChange: (event) => props.setPrompt(event.target.value),
        spellCheck: true,
      }),
    ),
    e(
      "div",
      { className: "action-row" },
      e(
        "label",
        { className: "turns-field" },
        e("span", null, "Turns"),
        e("input", {
          max: "8",
          min: "1",
          onChange: (event) => props.setMaxTurns(event.target.value),
          type: "number",
          value: props.maxTurns,
        }),
      ),
      e(
        "button",
        { className: "primary-action", disabled: props.isLoading, type: "submit" },
        props.isLoading ? "Reading" : props.mode === "speaking" ? "Start" : "Evaluate",
      ),
    ),
    e("p", { className: "form-error", role: "alert" }, props.error),
  );
}

function ResultPane({ activeMode, error, isLoading, result }) {
  let body = e(
    "div",
    { className: "empty-state" },
    e("span", { className: "thin-rule" }),
    e("p", null, "Submit a response to receive a composed examiner note."),
  );

  if (isLoading) {
    body = e(
      "div",
      { className: "loading-state", "aria-label": "Examiner is working" },
      e("span", null),
      e("span", null),
      e("span", null),
      e("span", null),
    );
  } else if (result) {
    body = e(
      React.Fragment,
      null,
      e("pre", { className: "result-text" }, result.text.trim()),
      result.session_log
        ? e("p", { className: "session-log" }, `Session log: ${result.session_log}`)
        : null,
    );
  } else if (error) {
    body = e(
      "div",
      { className: "empty-state error-state" },
      e("span", { className: "thin-rule" }),
      e("p", null, error),
    );
  }

  return e(
    "section",
    { className: "paper-panel result-pane", "aria-live": "polite" },
    e(
      "div",
      { className: "result-head" },
      e("h2", { className: "section-title" }, result ? `${activeMode.label} feedback` : "Examiner note"),
      result
        ? e("span", { className: "meta-chip" }, `${Math.max(1, Math.round(result.duration_ms / 1000))}s`)
        : e("span", { className: "meta-chip" }, activeMode.label),
    ),
    body,
  );
}

createRoot(document.getElementById("root")).render(e(App));
