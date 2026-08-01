// @amadeus-header
// summary: Renders safe GitHub-flavored Markdown for agent responses.
// layer: ui
// status: active
// feature_flags: none
// provides:
// - fn: MarkdownContent
// uses:
// - library: react-markdown
// - library: remark-gfm
// invariants:
// - Raw HTML remains escaped by default.
// - External links open without granting opener access.
// side_effects:
// - Copies fenced code content through the browser clipboard API.
// tests:
// - cmd: npm run build
// @end-amadeus-header

import { useState } from "react";
import { Check, Copy } from "@phosphor-icons/react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";

function MarkdownCode({ className, children }) {
  const [copied, setCopied] = useState(false);
  const text = String(children).replace(/\n$/, "");
  const language = className?.replace("language-", "") || "";
  const block = Boolean(className) || text.includes("\n");

  if (!block) return <code className="markdown-inline-code">{children}</code>;

  const copy = async () => {
    await navigator.clipboard.writeText(text);
    setCopied(true);
    setTimeout(() => setCopied(false), 1200);
  };

  return (
    <div className="markdown-code-block">
      <div className="markdown-code-header">
        <span>{language || "code"}</span>
        <button type="button" onClick={copy} aria-label="Copy code">
          {copied ? <Check /> : <Copy />}
        </button>
      </div>
      <pre><code className={className}>{text}</code></pre>
    </div>
  );
}

export function MarkdownContent({ text }) {
  return (
    <div className="markdown-content">
      <ReactMarkdown
        remarkPlugins={[remarkGfm]}
        components={{
          a: ({ children, ...props }) => <a {...props} target="_blank" rel="noreferrer">{children}</a>,
          code: MarkdownCode,
          pre: ({ children }) => children,
        }}
      >
        {text}
      </ReactMarkdown>
    </div>
  );
}
