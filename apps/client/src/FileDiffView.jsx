// @amadeus-header
// summary: Renders accessible red and green line diffs for file tool calls.
// layer: ui
// status: active
// feature_flags: none
// provides:
// - fn: FileDiffView
// uses:
// - format: Amadeus file diff model
// - library: Phosphor icons
// invariants:
// - File changes use conventional green additions and red deletions.
// side_effects: none
// tests:
// - apps/client/src/fileDiff.test.js
// @end-amadeus-header

import { FileCode } from "@phosphor-icons/react";

import "./fileDiff.css";

export function FileDiffView({ diff, labels }) {
  return (
    <section className="file-diff" aria-label={labels.changes}>
      <header className="file-diff-header">
        <span className="file-diff-path"><FileCode aria-hidden="true" /><span>{diff.path}</span></span>
        <span
          className="file-diff-stats"
          aria-label={`${diff.additions} ${labels.additions}, ${diff.deletions} ${labels.deletions}`}
        >
          <span className="file-diff-additions" aria-hidden="true">+{diff.additions}</span>
          <span className="file-diff-deletions" aria-hidden="true">-{diff.deletions}</span>
        </span>
      </header>
      <div className="file-diff-scroll" role="table">
        {diff.lines.map((line, index) => (
          <div className={`file-diff-line ${line.status}`} role="row" key={`${index}-${line.status}`}>
            <span className="file-diff-line-number" role="cell">{line.oldNumber ?? ""}</span>
            <span className="file-diff-line-number" role="cell">{line.newNumber ?? ""}</span>
            <span className="file-diff-marker" role="cell">{line.status === "added" ? "+" : line.status === "removed" ? "-" : " "}</span>
            <code role="cell">{line.content || " "}</code>
          </div>
        ))}
      </div>
    </section>
  );
}
