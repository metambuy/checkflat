// Geometry for the annotation editor and the overlays, in image pixels unless stated: fitting the
// picture into the stage, pointer → image coordinates, the arrow head (mirrored in
// crates/checkflat-core/src/annotation.rs), outlines for hit-testing, and the "too small to keep"
// rule. Pure, unit-tested.
import { ARROW_HEAD_ANGLE_DEG, MIN_POINTS, type Shape, type XY } from "./model.ts";

export interface Size {
  w: number;
  h: number;
}
export interface Box {
  x: number;
  y: number;
  w: number;
  h: number;
}

export const clamp = (v: number, lo: number, hi: number): number => Math.min(hi, Math.max(lo, v));
export const dist = (a: XY, b: XY): number => Math.hypot(a[0] - b[0], a[1] - b[1]);

/** The largest box with the picture's aspect that fits the stage, centred (what `object-fit: contain` shows). */
export function fitBox(stage: Size, natural: Size): Box {
  if (stage.w <= 0 || stage.h <= 0 || natural.w <= 0 || natural.h <= 0) return { x: 0, y: 0, w: 0, h: 0 };
  const s = Math.min(stage.w / natural.w, stage.h / natural.h);
  const w = natural.w * s;
  const h = natural.h * s;
  return { x: (stage.w - w) / 2, y: (stage.h - h) / 2, w, h };
}

/** A pointer position (CSS px in the stage) → image pixels, clamped to the picture. */
export function toImage(x: number, y: number, box: Box, natural: Size): XY {
  if (box.w <= 0 || box.h <= 0) return [0, 0];
  return [clamp(((x - box.x) * natural.w) / box.w, 0, natural.w), clamp(((y - box.y) * natural.h) / box.h, 0, natural.h)];
}

/**
 * The ends of the two chevron legs of an arrow head for a shaft `tail` → `tip` and a leg of `len`
 * (same units as the points); null when the arrow has no direction (tail = tip).
 */
export function arrowHead(tail: XY, tip: XY, len: number): [XY, XY] | null {
  const dx = tail[0] - tip[0];
  const dy = tail[1] - tip[1];
  const d = Math.hypot(dx, dy);
  if (!(d > 0)) return null;
  const ux = dx / d;
  const uy = dy / d;
  const a = (ARROW_HEAD_ANGLE_DEG * Math.PI) / 180;
  const sin = Math.sin(a);
  const cos = Math.cos(a);
  const leg = (s: number): XY => [tip[0] + len * (ux * cos - uy * s * sin), tip[1] + len * (uy * cos + ux * s * sin)];
  return [leg(1), leg(-1)];
}

/** A shape's outline as a polyline in image pixels (the ellipse sampled and closed). */
export function outlinePoints(s: Shape, w: number, h: number, ellipseSamples = 64): XY[] {
  switch (s.t) {
    case "ellipse": {
      const pts: XY[] = [];
      for (let i = 0; i < ellipseSamples; i++) {
        const a = (2 * Math.PI * i) / ellipseSamples;
        pts.push([(s.cx + s.rx * Math.cos(a)) * w, (s.cy + s.ry * Math.sin(a)) * h]);
      }
      pts.push([...pts[0]]); // closed
      return pts;
    }
    case "arrow":
      return [
        [s.x1 * w, s.y1 * h],
        [s.x2 * w, s.y2 * h],
      ];
    case "freehand":
      return s.pts.map(([x, y]) => [x * w, y * h]);
  }
}

/** Distance from `p` to the nearest point of the polyline (a single point counts too). */
export function distanceToPolyline(p: XY, pts: XY[]): number {
  if (pts.length === 0) return Number.POSITIVE_INFINITY;
  let best = dist(p, pts[0]);
  for (let i = 1; i < pts.length; i++) {
    const [ax, ay] = pts[i - 1];
    const [bx, by] = pts[i];
    const vx = bx - ax;
    const vy = by - ay;
    const len2 = vx * vx + vy * vy;
    const t = len2 === 0 ? 0 : clamp(((p[0] - ax) * vx + (p[1] - ay) * vy) / len2, 0, 1);
    best = Math.min(best, Math.hypot(p[0] - (ax + t * vx), p[1] - (ay + t * vy)));
  }
  return best;
}

/** Index of the shape whose outline is nearest to `p` (image px) within `tolerance` (image px), else null. */
export function hitTest(shapes: Shape[], p: XY, w: number, h: number, tolerance: number): number | null {
  let hit: number | null = null;
  let best = tolerance;
  shapes.forEach((s, i) => {
    const d = distanceToPolyline(p, outlinePoints(s, w, h));
    if (d <= best) {
      best = d;
      hit = i;
    }
  });
  return hit;
}

/** What an accidental tap would leave behind: thinner than the stroke, or a freehand with too few points. */
export function tooSmall(s: Shape, w: number, h: number, stroke: number): boolean {
  switch (s.t) {
    case "ellipse":
      return s.rx * w < stroke || s.ry * h < stroke;
    case "arrow":
      return Math.hypot((s.x2 - s.x1) * w, (s.y2 - s.y1) * h) < 2 * stroke;
    case "freehand":
      return s.pts.length < MIN_POINTS;
  }
}
