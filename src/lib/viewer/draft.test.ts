import { test } from "node:test";
import assert from "node:assert/strict";
import { cancel, confirm, moveDraft, noDraft, sheetAfter, showsCreateSheet, startAdding, startMoving, tap, type DraftState, type SheetMode } from "./draft.ts";
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
  assert.deepEqual(s, { adding: true, draft: null, saving: false, moving: null });
  s = tap(s, { x: 0.1, y: 0.2 });
  assert.deepEqual(s, { adding: false, draft: { x: 0.1, y: 0.2 }, saving: false, moving: null });
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
  assert.deepEqual(st.get(), { adding: false, draft: { x: 0.2, y: 0.2 }, saving: false, moving: null });
  assert.equal(await confirm(store().get, store().set, async () => 1), null, "no draft: nothing to confirm");
  const adding = store(startAdding(noDraft));
  assert.equal(await confirm(adding.get, adding.set, async () => 1), null, "mode without a draft: nothing to confirm");
});

const moving = (id = "obs-1", at = { x: 0.4, y: 0.6 }) => startMoving(noDraft, id, at);

test("Move turns the selected pin into the draft at its place and remembers which pin it is", () => {
  assert.deepEqual(moving(), { adding: false, draft: { x: 0.4, y: 0.6 }, saving: false, moving: "obs-1" });
});

test("move mode is exclusive with add-pin mode, another draft and saving", () => {
  assert.deepEqual(startMoving(startAdding(noDraft), "obs-1", { x: 0, y: 0 }), startAdding(noDraft), "+ mode is not replaced");
  assert.deepEqual(startMoving(placed({ x: 0.1, y: 0.1 }), "obs-1", { x: 0, y: 0 }), placed({ x: 0.1, y: 0.1 }), "a new draft waits");
  assert.deepEqual(startMoving(moving("a"), "b", { x: 0, y: 0 }), moving("a"), "one move at a time");
  const saving: DraftState = { ...moving(), saving: true };
  assert.deepEqual(startMoving(saving, "b", { x: 0, y: 0 }), saving);
  assert.deepEqual(startAdding(moving()), moving(), "+ is ignored while a pin is being moved");
});

test("in move mode a tap on the plan puts the draft there and the draft can be dragged; the pin stays the same", () => {
  let s = tap(moving(), { x: 0.7, y: 0.2 });
  assert.deepEqual(s, { adding: false, draft: { x: 0.7, y: 0.2 }, saving: false, moving: "obs-1" });
  s = moveDraft(s, { x: 0.71, y: 0.25 });
  assert.deepEqual(s, { adding: false, draft: { x: 0.71, y: 0.25 }, saving: false, moving: "obs-1" });
  assert.deepEqual(tap({ ...s, saving: true }, { x: 0.1, y: 0.1 }), { ...s, saving: true }, "ignored while saving");
});

test("cancel (Cancel, Back) ends the move without a call and leaves no draft", () => {
  assert.deepEqual(cancel(tap(moving(), { x: 0.9, y: 0.9 })), noDraft);
});

test("confirm saves the new place once, keeps `moving` while it is in flight and ends it on success", async () => {
  const st = store(tap(moving("obs-7"), { x: 0.25, y: 0.75 }));
  const calls: { x: number; y: number }[] = [];
  let release!: () => void;
  const move = (p: { x: number; y: number }) => new Promise<string>((r) => { calls.push(p); release = () => r("saved"); });
  const first = confirm(st.get, st.set, move);
  assert.deepEqual(st.get(), { adding: false, draft: { x: 0.25, y: 0.75 }, saving: true, moving: "obs-7" }, "still that pin's place, not a new pin");
  assert.equal(await confirm(st.get, st.set, move), null, "a second Confirm is ignored");
  assert.deepEqual(cancel(st.get()), st.get(), "Cancel is ignored while saving");
  release();
  assert.equal(await first, "saved");
  assert.deepEqual(calls, [{ x: 0.25, y: 0.75 }], "one call");
  assert.deepEqual(st.get(), noDraft);
});

test("a failed move keeps the draft and the pin's identity, so Confirm can be tried again", async () => {
  const st = store(tap(moving("obs-7"), { x: 0.25, y: 0.75 }));
  await assert.rejects(confirm(st.get, st.set, async () => { throw new Error("db"); }));
  assert.deepEqual(st.get(), { adding: false, draft: { x: 0.25, y: 0.75 }, saving: false, moving: "obs-7" });
  assert.equal(await confirm(st.get, st.set, async () => "saved"), "saved");
  assert.deepEqual(st.get(), noDraft);
});

// Review 1e (D-027): the create sheet belongs to the new pin's draft and ends with it.

test("the create sheet shows over the new pin's draft only: not without a draft, not over a move's draft", () => {
  const draft = placed({ x: 0.3, y: 0.3 });
  assert.ok(showsCreateSheet("create", draft));
  assert.ok(showsCreateSheet("create", { ...draft, saving: true }), "while Save is in flight");
  assert.ok(!showsCreateSheet("create", noDraft), "no draft");
  assert.ok(!showsCreateSheet("create", startAdding(noDraft)), "add-pin mode before the tap");
  assert.ok(!showsCreateSheet("create", moving()), "a move's draft is never a new observation");
  assert.ok(!showsCreateSheet("create", { ...moving(), saving: true }));
  assert.ok(!showsCreateSheet("edit", draft) && !showsCreateSheet(null, draft), "only the create sheet");
});

test("a create sheet closes when its draft is cleared or becomes a move; the edit sheet is not tied to the draft", () => {
  const draft = placed({ x: 0.3, y: 0.3 });
  assert.equal(sheetAfter("create", moveDraft(draft, { x: 0.4, y: 0.4 })), "create", "dragging the draft keeps it");
  assert.equal(sheetAfter("create", cancel(draft)), null, "Cancel, Back and a selected pin all cancel()");
  assert.equal(sheetAfter("create", cancel({ ...draft, saving: true })), "create", "a cancel ignored while saving keeps it");
  assert.equal(sheetAfter("create", moving()), null, "never into a move");
  assert.equal(sheetAfter("edit", noDraft), "edit");
  assert.equal(sheetAfter("edit", moving()), "edit");
  assert.equal(sheetAfter(null, draft), null, "never opens a sheet");
});

test("the draft cancelled behind an open create sheet, then Move: the create sheet does not come back", () => {
  let sheet: SheetMode = "create";
  let s = placed({ x: 0.3, y: 0.3 });
  const set = (n: DraftState) => { s = n; sheet = sheetAfter(sheet, n); };
  set(cancel(s)); // Cancel reached behind the sheet by Tab, or a pin selected in the side list
  assert.equal(sheet, null);
  set(startMoving(s, "obs-1", { x: 0.4, y: 0.6 }));
  assert.ok(!showsCreateSheet(sheet, s), "no create sheet over the move, so its Save cannot create");
  assert.ok(!showsCreateSheet("create", s), "and a create mode left over by any other path would not show over it either");
});

test("Save: the create sheet stays while the call is in flight and after a failure, and closes with the draft on success", async () => {
  let sheet: SheetMode = "create";
  const st = store(placed({ x: 0.5, y: 0.5 }));
  const set = (n: DraftState) => { st.set(n); sheet = sheetAfter(sheet, n); };
  await assert.rejects(confirm(st.get, set, async () => { assert.equal(sheet, "create", "in flight"); throw new Error("db"); }));
  assert.equal(sheet, "create", "a failed Save keeps the sheet with its error");
  assert.ok(showsCreateSheet(sheet, st.get()));
  assert.equal(await confirm(st.get, set, async () => "obs"), "obs");
  assert.equal(sheet, null);
  assert.deepEqual(st.get(), noDraft);
});

test("tap classifier", () => {
  assert.ok(isTap(120, 3, 1));
  assert.ok(!isTap(120, 12, 1), "moved: pan");
  assert.ok(!isTap(450, 2, 1), "long press");
  assert.ok(!isTap(120, 2, 2), "two fingers: pinch");
});
