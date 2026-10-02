import { test } from "node:test";
import assert from "node:assert/strict";
import { backStep } from "./back.ts";

const idle = { dialog: false, adding: false, draft: false, saving: false, selected: false };

test("with the delete dialog open, Back closes the dialog and nothing else", () => {
  // The dialog is opened from a selected pin: the selection must survive the first Back.
  assert.equal(backStep({ ...idle, dialog: true, selected: true }), "dialog");
  assert.equal(backStep({ ...idle, dialog: true, selected: true, adding: true }), "dialog");
  assert.equal(backStep({ ...idle, dialog: false, selected: true }), "selection", "second Back deselects");
});

test("Back order without a dialog: add-pin mode or draft, then selection, then up", () => {
  assert.equal(backStep({ ...idle, adding: true, selected: true }), "draft");
  assert.equal(backStep({ ...idle, draft: true }), "draft");
  assert.equal(backStep({ ...idle, draft: true, saving: true, selected: true }), "selection", "a draft being saved is not discarded");
  assert.equal(backStep({ ...idle, selected: true }), "selection");
  assert.equal(backStep(idle), null);
});
