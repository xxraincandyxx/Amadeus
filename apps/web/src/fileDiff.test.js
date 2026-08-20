// @amadeus-header
// summary: Verifies web file diff parsing, change classification, and line numbering.
// layer: test
// status: test-only
// feature_flags: none
// provides:
// - cmd: node --test src/fileDiff.test.js
// uses:
// - fn: buildFileDiff
// invariants:
// - File tool payload coverage includes objects and streamed JSON strings.
// side_effects: none
// tests:
// - cmd: npm test
// @end-amadeus-header

import assert from "node:assert/strict";
import test from "node:test";

import { buildFileDiff } from "./fileDiff.js";

test("write_file renders every content line as an addition", () => {
  const diff = buildFileDiff("write_file", {
    path: "src/new.rs",
    content: "fn main() {\n    println!(\"hello\");\n}\n",
  });

  assert.equal(diff.path, "src/new.rs");
  assert.equal(diff.additions, 3);
  assert.equal(diff.deletions, 0);
  assert.deepEqual(diff.lines.map((line) => line.status), ["added", "added", "added"]);
  assert.deepEqual(diff.lines.map((line) => line.newNumber), [1, 2, 3]);
  assert.deepEqual(diff.lines.map((line) => line.oldNumber), [null, null, null]);
});

test("edit_file marks replacements red and green", () => {
  const diff = buildFileDiff("edit_file", {
    path: "src/lib.rs",
    old_text: "alpha\nbeta\ngamma\n",
    new_text: "alpha\nupdated\ngamma\n",
  });

  assert.equal(diff.additions, 1);
  assert.equal(diff.deletions, 1);
  assert.deepEqual(diff.lines.map((line) => [line.status, line.content]), [
    ["unchanged", "alpha"],
    ["removed", "beta"],
    ["added", "updated"],
    ["unchanged", "gamma"],
  ]);
});

test("insertions preserve old and new line numbers", () => {
  const diff = buildFileDiff("edit_file", JSON.stringify({
    path: "notes.txt",
    old_text: "alpha\nbeta\n",
    new_text: "alpha\ninserted\nbeta\n",
  }));

  assert.deepEqual(diff.lines.map(({ oldNumber, newNumber }) => [oldNumber, newNumber]), [
    [1, 1],
    [null, 2],
    [2, 3],
  ]);
});

test("CRLF input is normalized without adding phantom lines", () => {
  const diff = buildFileDiff("write_file", JSON.stringify({
    path: "windows.txt",
    content: "first\r\nsecond\r\n",
  }));

  assert.deepEqual(diff.lines.map((line) => line.content), ["first", "second"]);
  assert.equal(diff.additions, 2);
});

test("unsupported, incomplete, and malformed inputs fall back to regular tool output", () => {
  assert.equal(buildFileDiff("bash", { command: "cargo test" }), null);
  assert.equal(buildFileDiff("write_file", { path: "empty.txt" }), null);
  assert.equal(buildFileDiff("edit_file", "{not-json"), null);
  assert.equal(buildFileDiff("edit_file", null), null);
});
