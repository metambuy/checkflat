// Draft pin (Sprint 2, D-018): the "+" button enters add-pin mode; the next tap on the plan places
// a draft that exists only here and leaves the mode. Outside the mode a tap on the plan does
// nothing. Confirm calls the backend once (create_pin takes the ref number); cancel discards the
// draft, so no number is consumed.
// Move mode (Sprint 4b, D-027): a stored pin moves only after it is selected and Move is tapped.
// The pin then becomes the draft (`moving` = its id): it is dragged or the plan is tapped, Confirm
// saves the new place once, Cancel/Back leaves it where it was. Nothing is stored until Confirm.
import type { Point } from "./coords";

export interface DraftState {
  /** Add-pin mode: the next tap on the plan places the draft. */
  adding: boolean;
  draft: Point | null;
  saving: boolean;
  /** Move mode: the id of the stored pin whose place the draft is; null otherwise. */
  moving: string | null;
}

export const noDraft: DraftState = { adding: false, draft: null, saving: false, moving: null };

/** Enter add-pin mode ("+"). Ignored while a draft is waiting for Confirm/Cancel. */
export function startAdding(s: DraftState): DraftState {
  return s.draft || s.saving ? s : { adding: true, draft: null, saving: false, moving: null };
}

/** Enter move mode for the stored pin `id`, now at `at`. Ignored while adding, while a draft waits
 * (a new one or another move) and while saving. */
export function startMoving(s: DraftState, id: string, at: Point): DraftState {
  return s.adding || s.draft || s.saving ? s : { adding: false, draft: { x: at.x, y: at.y }, saving: false, moving: id };
}

/** Tap on the empty plan: in add-pin mode it places the draft and leaves the mode; in move mode it
 * puts the draft there (the plan is crowded, a tap is easier than a drag); otherwise nothing
 * happens. */
export function tap(s: DraftState, p: Point): DraftState {
  if (s.saving) return s;
  if (s.adding) return { adding: false, draft: { x: p.x, y: p.y }, saving: false, moving: null };
  return s.moving && s.draft ? { ...s, draft: { x: p.x, y: p.y } } : s;
}

/** Drag of the existing draft. Ignored without a draft or while a confirm is in flight. */
export function moveDraft(s: DraftState, p: Point): DraftState {
  return s.draft && !s.saving ? { ...s, draft: { x: p.x, y: p.y } } : s;
}

/** Leave add-pin mode and discard the draft (Cancel, Back, selecting a pin). Ignored while saving
 * (the number may already be taken). */
export function cancel(s: DraftState): DraftState {
  return s.saving ? s : noDraft;
}

/** Confirm: calls `create` exactly once for the current draft; double taps are ignored. On
 * failure the draft stays so the user can retry or cancel. `moving` is kept while the call is in
 * flight and after a failure (the draft is still that pin's new place, not a new pin); only the
 * success ends it. */
export async function confirm<T>(
  get: () => DraftState,
  set: (s: DraftState) => void,
  create: (p: Point) => Promise<T>,
): Promise<T | null> {
  const s = get();
  if (!s.draft || s.saving) return null;
  set({ ...s, adding: false, saving: true });
  try {
    const created = await create(s.draft);
    set(noDraft);
    return created;
  } catch (e) {
    set({ ...s, adding: false, saving: false });
    throw e;
  }
}

/** The plan screen's observation sheet: "create" for the new pin's draft (Next), "edit" for the
 * selected pin, null when closed. */
export type SheetMode = "create" | "edit" | null;

/** The create sheet is the second step of placing a new pin: it shows only over that pin's draft,
 * never without a draft and never over a move's draft (D-027, review 1e). */
export function showsCreateSheet(sheet: SheetMode, s: DraftState): boolean {
  return sheet === "create" && s.draft !== null && s.moving === null;
}

/** The sheet once the draft has become `s`: a create sheet ends with its draft (Cancel from
 * anywhere, Back, a successful Save) and never carries over into a move; the edit sheet does not
 * depend on the draft. */
export function sheetAfter(sheet: SheetMode, s: DraftState): SheetMode {
  return sheet === "create" && !showsCreateSheet(sheet, s) ? null : sheet;
}
