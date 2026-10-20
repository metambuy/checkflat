// Photo annotations (Sprint 4, M6): the shapes drawn over a photo, kept as an object in the sheet
// state and the draft, stored as JSON in photo.annotation_json by the core and flattened onto the
// report copy in Rust. Coordinates are fractions of the image's width and height (0..1), so one set
// of shapes fits the stored ≤ 1600 px file, the thumbnails and the 800 px report copy.
// Mirrors crates/checkflat-core/src/annotation.rs — same format, same limits, same constants; a
// report test pins the literals below against this file, so change both.

export const ANNOTATION_VERSION = 1;
export const MAX_SHAPES = 50;
export const MAX_POINTS = 2000;
/** Over all freehand shapes of one photo. */
export const MAX_TOTAL_POINTS = 6000;
export const MIN_POINTS = 2;
/** Stroke width as a fraction of the image's long side. */
export const STROKE_FRACTION = 0.0075;
/** Each chevron leg of an arrow head is this many stroke widths long … */
export const ARROW_HEAD = 4;
/** … at this angle from the shaft. */
export const ARROW_HEAD_ANGLE_DEG = 30;
export const COLOUR = "#e60000";

/** `[x, y]`: fractions of the image size, or image pixels where a function says so. */
export type XY = [number, number];
export type Shape =
  /** Axis-aligned; `rx` is a fraction of the width, `ry` of the height. */
  | { t: "ellipse"; cx: number; cy: number; rx: number; ry: number }
  /** Tail (1) → head (2). */
  | { t: "arrow"; x1: number; y1: number; x2: number; y2: number }
  | { t: "freehand"; pts: XY[] };
export interface Annotation {
  v: 1;
  shapes: Shape[];
}

/** Stroke width in image pixels for an image of the given size. */
export const strokeWidth = (w: number, h: number): number => STROKE_FRACTION * Math.max(w, h);

const unit = (v: unknown): v is number => typeof v === "number" && Number.isFinite(v) && v >= 0 && v <= 1;

function parseShape(raw: unknown): Shape | null {
  if (typeof raw !== "object" || raw === null) return null;
  const s = raw as Record<string, unknown>;
  switch (s.t) {
    case "ellipse":
      if (!unit(s.cx) || !unit(s.cy) || !unit(s.rx) || !unit(s.ry) || s.rx <= 0 || s.ry <= 0) return null;
      return { t: "ellipse", cx: s.cx, cy: s.cy, rx: s.rx, ry: s.ry };
    case "arrow":
      if (!unit(s.x1) || !unit(s.y1) || !unit(s.x2) || !unit(s.y2)) return null;
      return { t: "arrow", x1: s.x1, y1: s.y1, x2: s.x2, y2: s.y2 };
    case "freehand": {
      if (!Array.isArray(s.pts) || s.pts.length < MIN_POINTS || s.pts.length > MAX_POINTS) return null;
      const pts: XY[] = [];
      for (const p of s.pts) {
        if (!Array.isArray(p) || p.length !== 2 || !unit(p[0]) || !unit(p[1])) return null;
        pts.push([p[0], p[1]]);
      }
      return { t: "freehand", pts };
    }
    default:
      return null;
  }
}

/**
 * The shapes of `raw` that pass the rules above, in order, within MAX_SHAPES and MAX_TOTAL_POINTS
 * (a shape that would exceed a budget is left out). What the editor hands back, so that whatever
 * its state, the sheet never gets an annotation the core would refuse; unlike `parseAnnotation`,
 * which refuses a whole annotation for one bad shape, this keeps the good ones.
 */
export function validShapes(raw: readonly unknown[]): Shape[] {
  const shapes: Shape[] = [];
  let points = 0;
  for (const r of raw) {
    if (shapes.length >= MAX_SHAPES) break;
    const s = parseShape(r);
    if (!s) continue;
    if (s.t === "freehand") {
      if (points + s.pts.length > MAX_TOTAL_POINTS) continue;
      points += s.pts.length;
    }
    shapes.push(s);
  }
  return shapes;
}

/**
 * The annotation in `v` (a backend object, a draft field, parsed JSON), or null when it is not a
 * valid one or has nothing drawn — "nothing drawn" is null throughout the app, never an empty list.
 */
export function parseAnnotation(v: unknown): Annotation | null {
  if (typeof v !== "object" || v === null) return null;
  const d = v as Record<string, unknown>;
  if (d.v !== ANNOTATION_VERSION || !Array.isArray(d.shapes) || d.shapes.length > MAX_SHAPES) return null;
  const shapes: Shape[] = [];
  let points = 0;
  for (const raw of d.shapes) {
    const s = parseShape(raw);
    if (!s) return null;
    if (s.t === "freehand") {
      points += s.pts.length;
      if (points > MAX_TOTAL_POINTS) return null;
    }
    shapes.push(s);
  }
  return shapes.length ? { v: 1, shapes } : null;
}
