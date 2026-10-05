// Minimal screen state (no router): projects list, one project, one plan, dev screen.
// Back (Android system back, Escape on desktop) first asks the current screen's handler, e.g. the
// plan screen cancels a draft pin; otherwise it goes up one level.
export type Screen =
  | { name: "projects" }
  | { name: "project"; id: string }
  | { name: "plan"; projectId: string; planId: string }
  | { name: "dev" };

const nav = $state<{ screen: Screen }>({ screen: { name: "projects" } });
let backHandler: (() => boolean) | null = null;

export function screen(): Screen {
  return nav.screen;
}
export function go(s: Screen): void {
  backHandler = null;
  nav.screen = s;
}

export function parent(s: Screen): Screen | null {
  switch (s.name) {
    case "projects": return null;
    case "project": return { name: "projects" };
    case "plan": return { name: "project", id: s.projectId };
    case "dev": return { name: "projects" };
  }
}

/** The current screen may consume Back (return true). Cleared on navigation. */
export function setBackHandler(h: (() => boolean) | null): void {
  backHandler = h;
}

/** Returns false at the root (the caller lets the platform handle it). */
export function back(): boolean {
  if (backHandler?.()) return true;
  const p = parent(nav.screen);
  if (!p) return false;
  go(p);
  return true;
}
