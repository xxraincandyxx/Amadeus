// @amadeus-header
// summary: Verifies the bilingual user-guide catalog and manuscript coverage.
// layer: test
// status: test-only
// feature_flags: none
// provides:
// - cmd: npm test
// uses:
// - module: apps/client/src/guideCatalog.js
// - module: docs/user-guide
// invariants:
// - Every catalog chapter has aligned English and Simplified Chinese manuscripts.
// side_effects:
// - Reads guide manuscripts from the workspace.
// tests:
// - cmd: npm test
// @end-amadeus-header

import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

import { GUIDE_CHAPTERS, guideChapterLabel } from "./guideCatalog.js";

const guideRoot = new URL("../../../docs/user-guide/", import.meta.url);

test("guide chapter identifiers are unique and localized", () => {
  assert.equal(new Set(GUIDE_CHAPTERS.map((chapter) => chapter.id)).size, GUIDE_CHAPTERS.length);
  for (const chapter of GUIDE_CHAPTERS) {
    assert.ok(guideChapterLabel(chapter, "en").title);
    assert.ok(guideChapterLabel(chapter, "zh-CN").title);
  }
});

test("every chapter has English and Simplified Chinese manuscripts", async () => {
  for (const chapter of GUIDE_CHAPTERS) {
    const english = await readFile(new URL(`en/${chapter.id}.md`, guideRoot), "utf8");
    const chinese = await readFile(new URL(`zh-CN/${chapter.id}.md`, guideRoot), "utf8");
    assert.match(english, /^# /);
    assert.match(chinese, /^# /);
    assert.ok(english.length > 250);
    assert.ok(chinese.length > 150);
  }
});

test("guide manuscripts avoid em dashes", async () => {
  for (const chapter of GUIDE_CHAPTERS) {
    for (const locale of ["en", "zh-CN"]) {
      const content = await readFile(new URL(`${locale}/${chapter.id}.md`, guideRoot), "utf8");
      assert.equal(content.includes("—"), false);
    }
  }
});
