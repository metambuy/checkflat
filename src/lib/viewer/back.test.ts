import { test } from "node:test";
import assert from "node:assert/strict";
import { backStep } from "./back.ts";

const idle = { dialog: false, discard: false, sheet: false, adding: false, draft: false, saving: false, selected: false };

test("with the delete dialog open, Back closes the dialog and nothing else", () => {
  // The dialog is opened from a selected pin: the selection must survive the first Back.
  assert.equal(backStep({ ...idle, dialog: true, selected: true }), "dialog");
  assert.equal(backStep({ ...idle, dialog: true, selected: true, adding: true }), "dialog");
  assert.equal(backStep({ ...idle, dialog: false, selected: true }), "selection", "second Back deselects");
});

test("Back order without a dialog: add-pin mode or draft, then selection, then up", () => {
  assert.equal(backStep({ ...idle, adding: true, selected: true }), "draft");
  assert.equal(backStep({ ...idle, draft: true }), "draft");
  assert.equal(backStep({ ...idle, draft: true, saving: true, selected: true }), "busy", "a draft being saved is not discarded");
  assert.equal(backStep({ ...idle, selected: true }), "selection");
  assert.equal(backStep(idle), null);
});

test("the observation sheet closes before the draft is discarded; not while saving", () => {
  assert.equal(backStep({ ...idle, sheet: true, draft: true }), "sheet");
  assert.equal(backStep({ ...idle, sheet: true, draft: true, selected: true }), "sheet");
  assert.equal(backStep({ ...idle, sheet: true, draft: true, saving: true, selected: true }), "busy", "a save in flight is not interrupted");
  assert.equal(backStep({ ...idle, dialog: true, sheet: true, draft: true }), "dialog");
});

test("Back during an edit save does nothing: it must not deselect the pin, which would unmount the edit sheet mid-save", () => {
  // Edit mode: the sheet is mounted only while a pin is selected (no draft), so "selection" here
  // used to close the sheet and discard its staged photos while update_observation was running.
  const editSaving = { ...idle, sheet: true, saving: true, selected: true };
  assert.equal(backStep(editSaving), "busy");
  assert.equal(backStep({ ...editSaving, saving: false }), "sheet", "once the save has finished Back closes the sheet");
  assert.equal(backStep({ ...editSaving, dialog: true }), "dialog", "the delete dialog still comes first");
});

test("move mode is a draft to Back: one press leaves it (Cancel), a move being saved is not interrupted", () => {
  // The moved pin is the draft (draft.ts `moving`), so backStep needs no case of its own.
  assert.equal(backStep({ ...idle, draft: true, selected: true }), "draft", "Back cancels the move, the pin stays selected");
  assert.equal(backStep({ ...idle, selected: true }), "selection", "the next Back deselects");
  assert.equal(backStep({ ...idle, draft: true, saving: true, selected: true }), "busy");
});

test("with the \"Discard?\" question open, Back means keep editing: it closes the question and leaves the sheet", () => {
  // The question is asked over an open sheet (create: with a draft; edit: with the pin selected).
  assert.equal(backStep({ ...idle, discard: true, sheet: true, draft: true }), "discard");
  assert.equal(backStep({ ...idle, discard: true, sheet: true, selected: true }), "discard");
  // Once answered "keep editing" the next Back asks again (the sheet step), it never discards by itself.
  assert.equal(backStep({ ...idle, discard: false, sheet: true, draft: true }), "sheet");
  assert.equal(backStep({ ...idle, dialog: true, discard: true, sheet: true }), "dialog", "the delete dialog still comes first");
});
