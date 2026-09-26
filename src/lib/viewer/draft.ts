// Draft pin (Sprint 2): a tap places or moves a draft that exists only here. Confirm calls the
// backend once (create_pin takes the ref number); cancel discards it, so no number is consumed.
import type { Point } from "./coords";

export interface DraftState {
  draft: Point | null;
  saving: boolean;
}

export const noDraft: DraftState = { draft: null, saving: false };

/** Place the draft, or move the existing one. Ignored while a confirm is in flight. */
export function place(s: DraftState, p: Point): DraftState {
  return s.saving ? s : { draft: { x: p.x, y: p.y }, saving: false };
}

/** Discard the draft. Ignored while saving (the number may already be taken). */
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
  set({ draft: s.draft, saving: true });
  try {
    const created = await create(s.draft);
    set(noDraft);
    return created;
  } catch (e) {
    set({ draft: s.draft, saving: false });
    throw e;
  }
}
