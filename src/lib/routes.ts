// The screens and how Back climbs between them. A plain module (nav.svelte.ts uses runes, which
// node --test cannot import), so the Back chain through the observation list is unit-tested.
import type { SheetDraft } from "./sheet/draft";

/**
 * The observation list of a project (M7). `from` is the plan it was opened from (Back goes there;
 * null: the project screen). `filter` (a fraction code, null = all) and `scrollTop` are the list's
 * memory: the route carries them, so Back from a plan that was opened out of the list finds the list
 * as it was left.
 */
export interface ObservationsScreen {
  name: "observations";
  projectId: string;
  from: { planId: string } | null;
  filter: string | null;
  scrollTop: number;
}

export type Screen =
  | { name: "projects" }
  | { name: "project"; id: string }
  | { name: "projectSettings"; id: string }
  /**
   * `recovered`: an unsaved observation sheet restored after the app was killed (sheet/draft.ts).
   * `focusId`: the pin to select and centre on arrival (a tap in the observation list); `from`: that
   * list, which Back returns to.
   */
  | { name: "plan"; projectId: string; planId: string; recovered?: SheetDraft; focusId?: string; from?: ObservationsScreen }
  | ObservationsScreen
  | { name: "dev" };

export function parent(s: Screen): Screen | null {
  switch (s.name) {
    case "projects": return null;
    case "project": return { name: "projects" };
    case "projectSettings": return { name: "project", id: s.id };
    case "plan": return s.from ?? { name: "project", id: s.projectId };
    case "observations": return s.from ? { name: "plan", projectId: s.projectId, planId: s.from.planId } : { name: "project", id: s.projectId };
    case "dev": return { name: "projects" };
  }
}
