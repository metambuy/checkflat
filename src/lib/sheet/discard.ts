// Whether an open observation sheet holds input that Cancel, Back or Escape would throw away: photos
// taken or picked (with the shapes drawn on them), shapes drawn on stored photos, a photo marked for
// removal, a fraction or a description typed. The plan screen asks "Discard?" before closing such a
// sheet and closes an untouched one at once (v0.1 review: one Back after the keyboard went down lost
// a camera photo, which exists only in the app's cache). Pure, unit-tested.
import type { SheetState } from "./draft.ts";

/**
 * `baselineDescription` is the text the sheet started with: the stored description when it edits an
 * observation, "" for a new one. Surrounding whitespace is not input (the save trims it).
 */
export function sheetIsDirty(s: SheetState, baselineDescription = ""): boolean {
  return (
    s.fraction.trim() !== "" ||
    s.description.trim() !== baselineDescription.trim() ||
    s.staged.length > 0 ||
    s.removed.length > 0 ||
    Object.keys(s.annotations).length > 0
  );
}
