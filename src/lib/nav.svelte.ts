// Minimal screen state (no router): projects list, one project, its settings, one plan, the
// observation list, dev screen. The screens and `parent()` live in routes.ts (testable without runes).
// Back (Android system back, Escape on desktop) first asks the handlers pushed by whatever sits on
// top of the screen (a full-screen editor), newest first, then the screen's own handler (e.g. the
// plan screen cancels a draft pin); otherwise it goes up one level.
import { parent, type Screen } from "./routes";

export type { Screen } from "./routes";

const nav = $state<{ screen: Screen }>({ screen: { name: "projects" } });
let screenHandler: (() => boolean) | null = null;
let stacked: (() => boolean)[] = [];

export function screen(): Screen {
  return nav.screen;
}
export function go(s: Screen): void {
  screenHandler = null;
  stacked = [];
  nav.screen = s;
}

/** The current screen may consume Back (return true). Cleared on navigation. */
export function setBackHandler(h: (() => boolean) | null): void {
  screenHandler = h;
}

/**
 * Something above the screen (a modal editor) takes Back first; returns the function that removes
 * the handler again (call it when the modal goes away). Newest handler first.
 */
export function pushBackHandler(h: () => boolean): () => void {
  stacked = [...stacked, h];
  return () => {
    stacked = stacked.filter((x) => x !== h);
  };
}

/** Returns false at the root (the caller lets the platform handle it). */
export function back(): boolean {
  for (let i = stacked.length - 1; i >= 0; i--) {
    if (stacked[i]()) return true;
  }
  if (screenHandler?.()) return true;
  const p = parent(nav.screen);
  if (!p) return false;
  go(p);
  return true;
}
