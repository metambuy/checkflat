import { test } from "node:test";
import assert from "node:assert/strict";
import { isTap, pinTap } from "./gestures.ts";

test("a pin tap is movement only: one pointer, under 8 px, however long it was held", () => {
  assert.ok(pinTap(0, 1));
  assert.ok(pinTap(7.9, 1));
  assert.ok(!pinTap(8, 1), "moved: the plan pans");
  assert.ok(!pinTap(2, 2), "two fingers: pinch");
});

test("a plan tap keeps its time limit; a pin tap does not", () => {
  // The same slow, still press: nothing on the empty plan (add-pin mode must not place a draft by
  // accident), but a select on a pin (no long-press action exists there).
  assert.ok(!isTap(600, 1, 1));
  assert.ok(pinTap(1, 1));
  assert.ok(isTap(120, 3, 1));
});
