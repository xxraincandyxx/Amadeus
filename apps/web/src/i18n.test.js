// @amadeus-header
// summary: Verifies desktop language selection, fallback, and interpolation behavior.
// layer: test
// status: test-only
// feature_flags: none
// provides:
// - cmd: npm test
// uses:
// - fn: normalizeLanguage
// - fn: translate
// invariants:
// - English and Simplified Chinese remain selectable with deterministic fallback behavior.
// side_effects: none
// tests:
// - cmd: npm test
// @end-amadeus-header

import assert from "node:assert/strict";
import test from "node:test";

import { normalizeLanguage, translate } from "./i18n.js";

test("normalizes Chinese locales to Simplified Chinese", () => {
  assert.equal(normalizeLanguage("zh-Hans"), "zh-CN");
  assert.equal(normalizeLanguage("en-US"), "en");
});

test("translates and interpolates desktop copy", () => {
  assert.equal(translate("zh-CN", "Thought for {seconds} seconds", { seconds: 17 }), "思考了 17 秒");
  assert.equal(translate("en", "Thought for {seconds} seconds", { seconds: 17 }), "Thought for 17 seconds");
});

test("falls back to the English source key", () => {
  assert.equal(translate("zh-CN", "Amadeus"), "Amadeus");
});
