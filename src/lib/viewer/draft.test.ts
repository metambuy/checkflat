import { test } from "node:test";
import assert from "node:assert/strict";
import { cancel, confirm, moveDraft, noDraft, startAdding, tap, type DraftState } from "./draft.ts";
import { isTap } from "./gestures.ts";

function store(init: DraftState = noDraft) {
  let s = init;
  return { get: () => s, set: (n: DraftState) => (s = n) };
}

const placed = (p: { x: number; y: number }) => tap(startAdding(noDraft), p);

test("outside add-pin mode a tap on the plan does nothing", () => {
  assert.deepEqual(tap(noDraft, { x: 0.1, y: 0.2 }), noDraft);
  assert.deepEqual(moveDraft(noDraft, { x: 0.1, y: 0.2 }), noDraft, "no draft to drag");
});

test("+ enters the mode, the next tap places the draft and exits the mode", () => {
  let s = startAdding(noDraft);
  assert.deepEqual(s, { adding: true, draft: null, saving: false });
  s = tap(s, { x: 0.1, y: 0.2 });
  assert.deepEqual(s, { adding: false, draft: { x: 0.1, y: 0.2 }, saving: false });
  const again = tap(s, { x: 0.3, y: 0.4 });
  assert.deepEqual(again, s, "mode left: a second tap neither moves the draft nor adds one");
  assert.deepEqual(startAdding(s), s, "+ ignored while a draft waits for Confirm/Cancel");
});

test("the draft can be dragged; cancel discards it without a call", () => {
  let s = moveDraft(placed({ x: 0.1, y: 0.2 }), { x: 0.3, y: 0.4 });
  assert.deepEqual(s.draft, { x: 0.3, y: 0.4 });
  s = cancel(s);
  assert.deepEqual(s, noDraft);
});

test("cancel (Back) leaves add-pin mode before any tap", () => {
  assert.deepEqual(cancel(startAdding(noDraft)), noDraft);
});

test("confirm calls create exactly once, even on a double tap", async () => {
  const st = store(placed({ x: 0.5, y: 0.5 }));
  let calls = 0;
  let release!: () => void;
  const create = (p: { x: number; y: number }) =>
    new Promise<number>((r) => { calls++; release = () => r(Math.round(p.x * 10)); });
  const first = confirm(st.get, st.set, create);
  const second = confirm(st.get, st.set, create);
  assert.equal(await second, null, "second confirm ignored while saving");
  assert.equal(st.get().saving, true);
  assert.deepEqual(cancel(st.get()), st.get(), "cancel ignored while saving");
  assert.deepEqual(moveDraft(st.get(), { x: 0.9, y: 0.9 }), st.get(), "drag ignored while saving");
  assert.deepEqual(startAdding(st.get()), st.get(), "+ ignored while saving");
  release();
  assert.equal(await first, 5);
  assert.equal(calls, 1);
  assert.deepEqual(st.get(), noDraft, "back to idle, not to add-pin mode");
});

test("failed confirm keeps the draft for retry or cancel", async () => {
  const st = store(placed({ x: 0.2, y: 0.2 }));
  await assert.rejects(confirm(st.get, st.set, async () => { throw new Error("db"); }));
  assert.deepEqual(st.get(), { adding: false, draft: { x: 0.2, y: 0.2 }, saving: false });
  assert.equal(await confirm(store().get, store().set, async () => 1), null, "no draft: nothing to confirm");
  const adding = store(startAdding(noDraft));
  assert.equal(await confirm(adding.get, adding.set, async () => 1), null, "mode without a draft: nothing to confirm");
});

test("tap classifier", () => {
  assert.ok(isTap(120, 3, 1));
  assert.ok(!isTap(120, 12, 1), "moved: pan");
  assert.ok(!isTap(450, 2, 1), "long press");
  assert.ok(!isTap(120, 2, 2), "two fingers: pinch");
});
