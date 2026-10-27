import { test } from "node:test";
import assert from "node:assert/strict";
import { sheetIsDirty } from "./discard.ts";
import type { SheetState } from "./draft.ts";
import type { StagedPhoto } from "../api";

const untouched: SheetState = { fraction: "", description: "", staged: [], removed: [], annotations: {} };
const photo: StagedPhoto = { token: "t1", path: "/tmp/t1.jpg", takenAt: "2026-10-27T10:00:00.000Z", annotation: null };
const shapes = { v: 1 as const, shapes: [{ t: "ellipse" as const, cx: 0.5, cy: 0.5, rx: 0.2, ry: 0.1 }] };

test("an untouched sheet is not dirty, in create mode and in edit mode", () => {
  assert.equal(sheetIsDirty(untouched), false);
  assert.equal(sheetIsDirty({ ...untouched, description: "Fissura na parede" }, "Fissura na parede"), false, "edit: the stored text");
});

test("whitespace is not input: the save trims it", () => {
  assert.equal(sheetIsDirty({ ...untouched, description: "  \n " }), false);
  assert.equal(sheetIsDirty({ ...untouched, fraction: "   " }), false);
  assert.equal(sheetIsDirty({ ...untouched, description: "Fissura na parede  " }, "Fissura na parede"), false);
});

test("text typed makes the sheet dirty: a description, a fraction, or an edit of the stored text", () => {
  assert.equal(sheetIsDirty({ ...untouched, description: "Fissura" }), true);
  assert.equal(sheetIsDirty({ ...untouched, fraction: "1D" }), true);
  assert.equal(sheetIsDirty({ ...untouched, description: "Fissura grande" }, "Fissura"), true);
  assert.equal(sheetIsDirty({ ...untouched, description: "" }, "Fissura"), true, "the stored text was deleted");
});

test("photos make the sheet dirty: one added (staged), one marked for removal", () => {
  assert.equal(sheetIsDirty({ ...untouched, staged: [photo] }), true, "a camera photo exists only in the cache until Save");
  assert.equal(sheetIsDirty({ ...untouched, description: "x", removed: ["p9"] }, "x"), true, "edit: the text is as stored, one stored photo is marked for removal");
});

test("shapes drawn make the sheet dirty: on a stored photo, and a clear (null) too", () => {
  assert.equal(sheetIsDirty({ ...untouched, annotations: { p1: shapes } }), true);
  assert.equal(sheetIsDirty({ ...untouched, annotations: { p1: null } }), true, "clearing a stored photo's shapes is a change");
  // Shapes drawn on a staged photo travel with the photo, which already makes the sheet dirty.
  assert.equal(sheetIsDirty({ ...untouched, staged: [{ ...photo, annotation: shapes }] }), true);
});
