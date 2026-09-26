import { test } from "node:test";
import assert from "node:assert/strict";
import { cancel, confirm, noDraft, place, type DraftState } from "./draft.ts";
import { isTap } from "./gestures.ts";

function store(init: DraftState = noDraft) {
  let s = init;
  return { get: () => s, set: (n: DraftState) => (s = n) };
}

test("tap places a draft, a second tap moves it, cancel discards without a call", () => {
  let s = place(noDraft, { x: 0.1, y: 0.2 });
  assert.deepEqual(s.draft, { x: 0.1, y: 0.2 });
  s = place(s, { x: 0.3, y: 0.4 });
  assert.deepEqual(s.draft, { x: 0.3, y: 0.4 }, "only one draft; it moves");
  s = cancel(s);
  assert.equal(s.draft, null);
});

test("confirm calls create exactly once, even on a double tap", async () => {
  const st = store(place(noDraft, { x: 0.5, y: 0.5 }));
  let calls = 0;
  let release!: () => void;
  const create = (p: { x: number; y: number }) =>
    new Promise<number>((r) => { calls++; release = () => r(Math.round(p.x * 10)); });
  const first = confirm(st.get, st.set, create);
  const second = confirm(st.get, st.set, create);
  assert.equal(await second, null, "second confirm ignored while saving");
  assert.equal(st.get().saving, true);
  assert.deepEqual(cancel(st.get()), st.get(), "cancel ignored while saving");
  assert.deepEqual(place(st.get(), { x: 0.9, y: 0.9 }), st.get(), "tap ignored while saving");
  release();
  assert.equal(await first, 5);
  assert.equal(calls, 1);
  assert.deepEqual(st.get(), noDraft);
});

test("failed confirm keeps the draft for retry or cancel", async () => {
  const st = store(place(noDraft, { x: 0.2, y: 0.2 }));
  await assert.rejects(confirm(st.get, st.set, async () => { throw new Error("db"); }));
  assert.deepEqual(st.get(), { draft: { x: 0.2, y: 0.2 }, saving: false });
  assert.equal(await confirm(store().get, store().set, async () => 1), null, "no draft: nothing to confirm");
});

test("tap classifier", () => {
  assert.ok(isTap(120, 3, 1));
  assert.ok(!isTap(120, 12, 1), "moved: pan");
  assert.ok(!isTap(450, 2, 1), "long press");
  assert.ok(!isTap(120, 2, 2), "two fingers: pinch");
});
