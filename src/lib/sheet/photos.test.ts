import test from "node:test";
import assert from "node:assert/strict";
import { addStaged, canSave, count, noPhotos, removeExisting, removeStaged, shown, toSave, withExisting } from "./photos.ts";

const stored = (id: string) => ({ id, observationId: "o", path: `/data/projects/p/photos/${id}.jpg`, takenAt: "2026-10-05T12:00:00.000Z" });
const staged = (token: string) => ({ token, path: `/data/tmp/${token}.jpg`, takenAt: "2026-10-06T08:00:00.000Z" });

test("a new observation needs one staged photo", () => {
  assert.equal(canSave(noPhotos, false), false);
  const s = addStaged(noPhotos, staged("t1"));
  assert.equal(canSave(s, false), true);
  assert.equal(canSave(s, true), false, "not while a photo is being processed");
  assert.equal(canSave(removeStaged(s, "t1"), false), false);
});

test("adding the same token twice keeps one", () => {
  const s = addStaged(addStaged(noPhotos, staged("t1")), staged("t1"));
  assert.equal(count(s), 1);
});

test("stored photos show first, staged after, in the order added", () => {
  let s = withExisting(noPhotos, [stored("a"), stored("b")]);
  s = addStaged(addStaged(s, staged("t1")), staged("t2"));
  assert.deepEqual(shown(s).map((p) => p.key), ["a", "b", "t1", "t2"]);
  assert.deepEqual(shown(s).map((p) => p.staged), [false, false, true, true]);
});

test("removing a stored photo hides it and is sent to the save; removing twice or an unknown id is a no-op", () => {
  let s = withExisting(noPhotos, [stored("a"), stored("b")]);
  s = removeExisting(removeExisting(removeExisting(s, "a"), "a"), "zzz");
  assert.deepEqual(shown(s).map((p) => p.key), ["b"]);
  assert.deepEqual(toSave(s), { photos: [], removed: ["a"] });
});

test("an observation migrated without photos cannot be saved until one is added", () => {
  const s = withExisting(noPhotos, []);
  assert.equal(canSave(s, false), false);
  assert.equal(canSave(addStaged(s, staged("t1")), false), true);
});

test("removing the last stored photo needs a replacement", () => {
  const s = removeExisting(withExisting(noPhotos, [stored("a")]), "a");
  assert.equal(canSave(s, false), false);
  const swapped = addStaged(s, staged("t1"));
  assert.equal(canSave(swapped, false), true);
  assert.deepEqual(toSave(swapped), { photos: [{ token: "t1", takenAt: "2026-10-06T08:00:00.000Z" }], removed: ["a"] });
});
