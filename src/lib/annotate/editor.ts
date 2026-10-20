// The annotation editor's state (Sprint 4, M6), pure and unit-tested: a tool, the committed shapes
// (normalised), the stroke in progress (image px), and the selection. One pointer draws: `begin`
// on pointerdown, `update` on every move, `end` on release. A release that moved less than
// TAP_SLOP_CSS — however long it was held — is a tap: it selects the nearest shape within
// HIT_TOLERANCE_CSS or clears the selection, in every tool (no separate select tool). Anything
// else commits the tool's shape unless it is too small to be deliberate. Undo removes the last
// shape added; Delete removes the selected one. The format's limits are kept here, so the editor
// never builds what the core would refuse: a freehand stroke ends at MAX_POINTS and at what is
// left of MAX_TOTAL_POINTS (the rest of the gesture draws nothing), a full photo takes no new
// shape, and `limit` says so for the hint. The component owns the DOM; this owns the rules.
import { dist, hitTest, tooSmall, type Size } from "./geometry.ts";
import { MAX_POINTS, MAX_SHAPES, MAX_TOTAL_POINTS, MIN_POINTS, strokeWidth, validShapes, type Annotation, type Shape, type XY } from "./model.ts";

export type Tool = "ellipse" | "arrow" | "freehand";

/** A release that moved less than this (CSS px) is a tap, regardless of its duration. */
export const TAP_SLOP_CSS = 8;
/** A tap selects the shape whose outline is within this distance (CSS px). */
export const HIT_TOLERANCE_CSS = 16;
/** Freehand points closer than this (CSS px) to the last kept one are dropped. */
export const THIN_CSS = 2;

export interface Stroke {
  tool: Tool;
  /** Image px. */
  start: XY;
  current: XY;
  /** Freehand only: the kept points, starting with `start`. */
  pts: XY[];
  /** The farthest the pointer went from where it came down, CSS px. */
  maxMove: number;
  /**
   * Freehand only: how many points this stroke may keep, MAX_POINTS or what is left of
   * MAX_TOTAL_POINTS. Below MIN_POINTS the stroke can still select (a tap) but never draws.
   */
  room: number;
}

export interface EditorState {
  natural: Size;
  tool: Tool;
  shapes: Shape[];
  stroke: Stroke | null;
  selected: number | null;
  /** A limit of the format stopped something drawn since the last stroke began or shape was removed. */
  limit: boolean;
}

export function initial(natural: Size, a: Annotation | null, tool: Tool = "ellipse"): EditorState {
  return { natural, tool, shapes: a ? [...a.shapes] : [], stroke: null, selected: null, limit: false };
}

/** Freehand points the photo can still take over all its shapes. */
export function pointBudget(shapes: Shape[]): number {
  let used = 0;
  for (const s of shapes) if (s.t === "freehand") used += s.pts.length;
  return Math.max(0, MAX_TOTAL_POINTS - used);
}

const canDraw = (st: Stroke): boolean => st.tool !== "freehand" || st.room >= MIN_POINTS;

export function setTool(s: EditorState, tool: Tool): EditorState {
  return s.tool === tool ? s : { ...s, tool };
}

/** Pointer down at `p` (image px). Ignored while a stroke is in progress (a second pointer). */
export function begin(s: EditorState, p: XY): EditorState {
  if (s.stroke) return s;
  const room = s.tool === "freehand" ? Math.min(MAX_POINTS, pointBudget(s.shapes)) : 0;
  return { ...s, limit: false, stroke: { tool: s.tool, start: p, current: p, pts: [p], maxMove: 0, room } };
}

/**
 * Pointer move to `p` (image px); `movedCss` is its distance from the pointer-down position in CSS
 * px and `thinPx` the freehand thinning distance in image px (THIN_CSS converted by the component).
 * A freehand stroke that has used its room keeps what it has and ignores the rest of the gesture;
 * moving on (more than a tap's slop) raises `limit`.
 */
export function update(s: EditorState, p: XY, movedCss: number, thinPx: number): EditorState {
  const st = s.stroke;
  if (!st) return s;
  let { pts } = st;
  let limit = s.limit;
  if (st.tool === "freehand" && dist(p, pts[pts.length - 1]) >= thinPx) {
    if (pts.length < st.room) pts = [...pts, p];
    else if (movedCss >= TAP_SLOP_CSS) limit = true;
  }
  return { ...s, limit, stroke: { ...st, current: p, pts, maxMove: Math.max(st.maxMove, movedCss) } };
}

const r4 = (v: number): number => Math.round(Math.min(1, Math.max(0, v)) * 1e4) / 1e4;

/** The shape a stroke stands for, normalised to the image (4 decimals: 0.16 px at 1600 px). */
export function toShape(st: Stroke, natural: Size): Shape {
  const { w, h } = natural;
  switch (st.tool) {
    case "ellipse": {
      const [x0, y0] = st.start;
      const [x1, y1] = st.current;
      return { t: "ellipse", cx: r4((x0 + x1) / 2 / w), cy: r4((y0 + y1) / 2 / h), rx: r4(Math.abs(x1 - x0) / 2 / w), ry: r4(Math.abs(y1 - y0) / 2 / h) };
    }
    case "arrow":
      return { t: "arrow", x1: r4(st.start[0] / w), y1: r4(st.start[1] / h), x2: r4(st.current[0] / w), y2: r4(st.current[1] / h) };
    case "freehand":
      return { t: "freehand", pts: st.pts.map(([x, y]) => [r4(x / w), r4(y / h)]) };
  }
}

/** The stroke in progress as a shape, for the overlay; null when nothing is being drawn. */
export function liveShape(s: EditorState): Shape | null {
  return s.stroke && canDraw(s.stroke) ? toShape(s.stroke, s.natural) : null;
}

/**
 * Pointer up. `pxPerCss` converts the CSS-px tolerances to image px. A tap selects or clears; a
 * drag commits its shape unless it is too small (silently: an accident) or the photo already holds
 * MAX_SHAPES (`limit`).
 */
export function end(s: EditorState, pxPerCss: number): EditorState {
  const st = s.stroke;
  if (!st) return s;
  const { w, h } = s.natural;
  if (st.maxMove < TAP_SLOP_CSS) {
    return { ...s, stroke: null, selected: hitTest(s.shapes, st.start, w, h, HIT_TOLERANCE_CSS * pxPerCss) };
  }
  const shape = toShape(st, s.natural);
  if (!canDraw(st) || tooSmall(shape, w, h, strokeWidth(w, h))) return { ...s, stroke: null };
  if (s.shapes.length >= MAX_SHAPES) return { ...s, stroke: null, limit: true };
  return { ...s, stroke: null, shapes: [...s.shapes, shape], selected: null };
}

/** A second finger, `pointercancel`, or leaving: the stroke in progress is dropped. */
export function cancelStroke(s: EditorState): EditorState {
  return s.stroke ? { ...s, stroke: null } : s;
}

/** Removes the last shape added (no redo; a delete is not undone). */
export function undo(s: EditorState): EditorState {
  if (s.shapes.length === 0) return s;
  return { ...s, shapes: s.shapes.slice(0, -1), selected: null, limit: false };
}

export function deleteSelected(s: EditorState): EditorState {
  if (s.selected === null) return s;
  return { ...s, shapes: s.shapes.filter((_, i) => i !== s.selected), selected: null, limit: false };
}

/**
 * What Done hands back: null when nothing is drawn. The shapes go through the model's rules once
 * more (`validShapes`), so the sheet never gets an annotation the core would refuse, whatever the
 * state above did; the editor's own caps make that a safety net that should never drop anything.
 */
export function commit(s: EditorState): Annotation | null {
  const shapes = validShapes(s.shapes);
  return shapes.length ? { v: 1, shapes } : null;
}
