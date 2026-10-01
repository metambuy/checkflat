// Draft pin (Sprint 2, D-018): the "+" button enters add-pin mode; the next tap on the plan places
// a draft that exists only here and leaves the mode. Outside the mode a tap on the plan does
// nothing. Confirm calls the backend once (create_pin takes the ref number); cancel discards the
// draft, so no number is consumed.
import type { Point } from "./coords";

export interface DraftState {
  /** Add-pin mode: the next tap on the plan places the draft. */
  adding: boolean;
  draft: Point | null;
  saving: boolean;
}

export const noDraft: DraftState = { adding: false, draft: null, saving: false };

/** Enter add-pin mode ("+"). Ignored while a draft is waiting for Confirm/Cancel. */
export function startAdding(s: DraftState): DraftState {
  return s.draft || s.saving ? s : { adding: true, draft: null, saving: false };
}

/** Tap on the empty plan: in add-pin mode it places the draft and leaves the mode; otherwise
 * nothing happens. */
export function tap(s: DraftState, p: Point): DraftState {
  return s.adding && !s.saving ? { adding: false, draft: { x: p.x, y: p.y }, saving: false } : s;
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
 * failure the draft stays so the user can retry or cancel. */
export async function confirm<T>(
  get: () => DraftState,
  set: (s: DraftState) => void,
  create: (p: Point) => Promise<T>,
): Promise<T | null> {
  const s = get();
  if (!s.draft || s.saving) return null;
  set({ adding: false, draft: s.draft, saving: true });
  try {
    const created = await create(s.draft);
    set(noDraft);
    return created;
  } catch (e) {
    set({ adding: false, draft: s.draft, saving: false });
    throw e;
  }
}
