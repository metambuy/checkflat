// What Back (Android system back, Escape) does on the plan screen, one step per press: close the
// delete dialog, and nothing else; else, while a save is in flight, nothing at all ("busy": Back is
// swallowed, so the sheet, the selection and the screen stay put until the save has finished); else
// close the observation sheet (the draft stays on the plan);
// else leave add-pin mode or discard the draft; else clear the selection; else (null) go up one
// level. Pure, unit-tested.
export type BackStep = "dialog" | "busy" | "sheet" | "draft" | "selection" | null;

export function backStep(s: { dialog: boolean; sheet: boolean; adding: boolean; draft: boolean; saving: boolean; selected: boolean }): BackStep {
  if (s.dialog) return "dialog";
  if (s.saving) return "busy";
  if (s.sheet) return "sheet";
  if (s.adding || s.draft) return "draft";
  if (s.selected) return "selection";
  return null;
}
