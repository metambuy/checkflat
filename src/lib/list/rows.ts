// Observation list (M7) as pure functions: the fraction filter and its chips, the one-line
// description, and the routes in and out of the list. Unit-tested.
import type { ObservationsScreen, Screen } from "../routes";

/** The list of a project, opened from the project screen (`fromPlanId` null) or from a plan. */
export function listRoute(projectId: string, fromPlanId: string | null = null, filter: string | null = null, scrollTop = 0): ObservationsScreen {
  return { name: "observations", projectId, from: fromPlanId ? { planId: fromPlanId } : null, filter, scrollTop };
}

/**
 * The plan screen for a tapped row: it selects and centres the row's pin, and its Back returns to
 * this list with the filter and scroll position it has now.
 */
export function planFromList(list: ObservationsScreen, row: { id: string; planId: string }, filter: string | null, scrollTop: number): Screen {
  return { name: "plan", projectId: list.projectId, planId: row.planId, focusId: row.id, from: { ...list, filter, scrollTop } };
}

/** The rows of one fraction; `null` = all. A row without a fraction shows under "All" only. */
export function filterRows<T extends { fraction: string }>(rows: T[], fraction: string | null): T[] {
  return fraction === null ? rows : rows.filter((r) => r.fraction === fraction);
}

/** The project's fraction list, then any code the rows carry that the list lacks (a deleted fraction keeps its observations). */
export function fractionChips(listed: string[], rows: { fraction: string }[]): string[] {
  const out = [...listed];
  for (const r of rows) if (r.fraction !== "" && !out.includes(r.fraction)) out.push(r.fraction);
  return out;
}

/**
 * The photos whose list thumbnail still has to be made, in list order (top rows first) and in
 * batches of `size`: the list asks for one batch at a time, so the rows show at once with
 * placeholders and the thumbnails appear as they are made. Rows without a photo or with a
 * thumbnail already are not asked for.
 */
export function thumbBatches(rows: { photoId: string | null; thumbPath: string | null }[], size = 4): string[][] {
  const ids = rows.flatMap((r) => (r.photoId !== null && r.thumbPath === null ? [r.photoId] : []));
  const batches: string[][] = [];
  for (let i = 0; i < ids.length; i += size) batches.push(ids.slice(i, i + size));
  return batches;
}

/** The rows with the thumbnails that arrived (the same array when none did). */
export function withThumbs<T extends { photoId: string | null; thumbPath: string | null }>(rows: T[], ready: { photoId: string; path: string }[]): T[] {
  if (ready.length === 0) return rows;
  const paths = new Map(ready.map((t) => [t.photoId, t.path]));
  return rows.map((r) => (r.photoId !== null && paths.has(r.photoId) ? { ...r, thumbPath: paths.get(r.photoId)! } : r));
}

/** The description on one line: whitespace collapsed, cut after `max` characters with an ellipsis. */
export function descriptionStart(text: string, max = 80): string {
  const flat = text.replace(/\s+/g, " ").trim();
  const chars = Array.from(flat);
  return chars.length <= max ? flat : chars.slice(0, max - 1).join("").trimEnd() + "…";
}
