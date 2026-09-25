import { test } from "node:test";
import assert from "node:assert/strict";
import { interpolate } from "./interpolate.ts";

test("substitutes each placeholder once from the original params", () => {
  assert.equal(interpolate("Delete {name} and its {count} plan(s)?", { name: "X", count: 2 }), "Delete X and its 2 plan(s)?");
});

test("a value containing another placeholder is not re-substituted", () => {
  const out = interpolate("{name} · {count}", { name: "weird {count} name", count: 3 });
  assert.equal(out, "weird {count} name · 3");
});

test("unknown placeholders are left as-is, no params is a no-op", () => {
  assert.equal(interpolate("{missing} stays", { other: 1 }), "{missing} stays");
  assert.equal(interpolate("plain"), "plain");
});
