// Plan viewer geometry. "World" px are px of the top tile level; a Transform maps world px to
// element (css) px: screen = t.{x,y} + world * t.scale. Pins are stored normalised (0–1).
export interface Transform {
  x: number;
  y: number;
  scale: number;
}
export interface Size {
  w: number;
  h: number;
}
export interface Point {
  x: number;
  y: number;
}

/** Zoom limits relative to "fit" (D-014): 8× fit, but the top level is never upscaled beyond
 * MAX_UPSCALE device px per source px; zoom out to MIN_ZOOM × fit. */
export const MAX_ZOOM = 8;
export const MAX_UPSCALE = 1.5;
export const MIN_ZOOM = 0.75;

export const clamp = (v: number, lo: number, hi: number) => Math.min(hi, Math.max(lo, v));
export const clamp01 = (v: number) => clamp(v, 0, 1);

export function fitTransform(view: Size, world: Size): Transform {
  const scale = Math.min(view.w / world.w, view.h / world.h);
  return { scale, x: (view.w - world.w * scale) / 2, y: (view.h - world.h * scale) / 2 };
}

export function fitScale(view: Size, world: Size): number {
  return Math.min(view.w / world.w, view.h / world.h);
}

/** Max css scale: min(8 × fit, 1.5 / dpr), never below fit. */
export function maxScale(fit: number, dpr: number): number {
  return Math.max(fit, Math.min(fit * MAX_ZOOM, MAX_UPSCALE / dpr));
}

export function toNorm(t: Transform, world: Size, px: number, py: number): Point {
  return { x: (px - t.x) / t.scale / world.w, y: (py - t.y) / t.scale / world.h };
}

export function toScreen(t: Transform, world: Size, nx: number, ny: number): Point {
  return { x: t.x + nx * world.w * t.scale, y: t.y + ny * world.h * t.scale };
}

export const insidePlan = (p: Point) => p.x >= 0 && p.x <= 1 && p.y >= 0 && p.y <= 1;

/** Keep the view's centre over the plan so it can never be panned out of sight. */
export function clampPan(t: Transform, view: Size, world: Size): Transform {
  const w = world.w * t.scale, h = world.h * t.scale;
  return { scale: t.scale, x: clamp(t.x, view.w / 2 - w, view.w / 2), y: clamp(t.y, view.h / 2 - h, view.h / 2) };
}

/** Put plan point (nx, ny) at the centre of the view at the given scale. */
export function centreOn(view: Size, world: Size, nx: number, ny: number, scale: number): Transform {
  return { scale, x: view.w / 2 - nx * world.w * scale, y: view.h / 2 - ny * world.h * scale };
}

/** After a resize/rotation: keep the plan point at the centre and the zoom relative to fit. */
export function refit(t: Transform, oldView: Size, newView: Size, world: Size): Transform {
  const c = toNorm(t, world, oldView.w / 2, oldView.h / 2);
  const rel = t.scale / fitScale(oldView, world);
  return centreOn(newView, world, clamp01(c.x), clamp01(c.y), rel * fitScale(newView, world));
}

/** Index of the smallest level with ≥ 1 source px per device px, else the largest available.
 * `sizes`: available levels' long sides, ascending; `worldTop`: long side of world px (the top
 * level, even while it is still being generated); `scale`: css px per world px. */
export function chooseLevel(sizes: number[], worldTop: number, scale: number, dpr: number): number {
  const need = scale * dpr;
  const i = sizes.findIndex((s) => s / worldTop >= need);
  return i === -1 ? sizes.length - 1 : i;
}
