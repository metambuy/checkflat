// Pin list updates for the plan screen: optimistic move and its rollback. Pure, unit-tested.
import type { Point } from "./coords";

interface Positioned {
  id: string;
  xNorm: number;
  yNorm: number;
}

/** The list with pin `id` at `p`; every other pin (and the list's membership) untouched. */
export function withPosition<T extends Positioned>(pins: T[], id: string, p: Point): T[] {
  return pins.map((o) => (o.id === id ? { ...o, xNorm: p.x, yNorm: p.y } : o));
}

/** A move to `attempted` failed: put that pin back at `previous`. Works on the current list, so
 * pins created or deleted while the move was in flight stay as they are; and only if the pin is
 * still where this move put it (a later move of the same pin is not undone). */
export function revertMove<T extends Positioned>(pins: T[], id: string, attempted: Point, previous: Point): T[] {
  return pins.map((o) => (o.id === id && o.xNorm === attempted.x && o.yNorm === attempted.y ? { ...o, xNorm: previous.x, yNorm: previous.y } : o));
}
