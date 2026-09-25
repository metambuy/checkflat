// Minimal screen state (no router): projects list, one project, dev screen.
export type Screen = { name: "projects" } | { name: "project"; id: string } | { name: "dev" };

const nav = $state<{ screen: Screen }>({ screen: { name: "projects" } });

export function screen(): Screen {
  return nav.screen;
}
export function go(s: Screen): void {
  nav.screen = s;
}
