// What Back (Android system back, Escape) does on the plan screen, one step per press: close the
// delete dialog, and nothing else; else leave add-pin mode or discard the draft; else clear the
// selection; else (null) go up one level. Pure, unit-tested.
export type BackStep = "dialog" | "draft" | "selection" | null;

export function backStep(s: { dialog: boolean; adding: boolean; draft: boolean; saving: boolean; selected: boolean }): BackStep {
  if (s.dialog) return "dialog";
  if ((s.adding || s.draft) && !s.saving) return "draft";
  if (s.selected) return "selection";
  return null;
}
