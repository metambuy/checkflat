import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { discardMessageKey, sheetIsDirty } from "./discard.ts";
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

const text = (lang: "en" | "pt", key: string) => (JSON.parse(readFileSync(fileURLToPath(new URL(`../../../i18n/${lang}.json`, import.meta.url)), "utf8")) as Record<string, string>)[key];

test("a new observation asks about discarding the observation; an edit asks about discarding the changes", () => {
  assert.equal(discardMessageKey("create"), "sheet.discard_confirm");
  assert.equal(discardMessageKey("edit"), "sheet.discard_changes_confirm");
  assert.equal(text("pt", discardMessageKey("create")), "Descartar esta observação? As fotos e o texto não guardados perdem-se.");
  assert.equal(text("en", discardMessageKey("create")), "Discard this observation? Unsaved photos and text will be lost.");
  assert.equal(text("pt", discardMessageKey("edit")), "Descartar as alterações? As alterações não guardadas perdem-se.");
  assert.equal(text("en", discardMessageKey("edit")), "Discard changes? Unsaved changes will be lost.");
});

test("both questions use the same two buttons: Descartar / Continuar a editar", () => {
  assert.equal(text("pt", "sheet.discard"), "Descartar");
  assert.equal(text("pt", "sheet.keep_editing"), "Continuar a editar");
  assert.equal(text("en", "sheet.discard"), "Discard");
  assert.equal(text("en", "sheet.keep_editing"), "Keep editing");
});
