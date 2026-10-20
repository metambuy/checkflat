import { test } from "node:test";
import assert from "node:assert/strict";
import { begin, cancelStroke, commit, deleteSelected, end, initial, liveShape, pointBudget, setTool, undo, update, type EditorState } from "./editor.ts";
import { MAX_POINTS, MAX_SHAPES, MAX_TOTAL_POINTS, parseAnnotation, type Annotation, type Shape, type XY } from "./model.ts";

const natural = { w: 1600, h: 1200 };
/** The editor shows the 1600 px photo 400 px wide: 4 image px per CSS px. */
const PX_PER_CSS = 4;
const THIN_PX = 2 * PX_PER_CSS;

/** A drag from `a` to `b` (image px) with a couple of moves in between. */
function drag(s: EditorState, a: [number, number], b: [number, number]): EditorState {
  s = begin(s, a);
  const mid: [number, number] = [(a[0] + b[0]) / 2, (a[1] + b[1]) / 2];
  s = update(s, mid, Math.hypot(mid[0] - a[0], mid[1] - a[1]) / PX_PER_CSS, THIN_PX);
  s = update(s, b, Math.hypot(b[0] - a[0], b[1] - a[1]) / PX_PER_CSS, THIN_PX);
  return end(s, PX_PER_CSS);
}

test("an ellipse is committed from its bounding box, normalised and rounded", () => {
  let s = initial(natural, null);
  assert.equal(s.tool, "ellipse");
  s = drag(s, [400, 300], [1200, 600]);
  assert.deepEqual(s.shapes, [{ t: "ellipse", cx: 0.5, cy: 0.375, rx: 0.25, ry: 0.125 }]);
  assert.equal(s.stroke, null);
  assert.equal(s.selected, null);
  // Dragging the other way gives the same ellipse.
  const t = drag(initial(natural, null), [1200, 600], [400, 300]);
  assert.deepEqual(t.shapes, s.shapes);
  assert.deepEqual(commit(s), { v: 1, shapes: s.shapes });
});

test("arrow: tail where the pointer came down, head where it was released", () => {
  const s = drag(setTool(initial(natural, null), "arrow"), [160, 1080], [1440, 120]);
  assert.deepEqual(s.shapes, [{ t: "arrow", x1: 0.1, y1: 0.9, x2: 0.9, y2: 0.1 }]);
});

test("freehand keeps the points that are far enough apart, in order", () => {
  let s = setTool(initial(natural, null), "freehand");
  s = begin(s, [100, 100]);
  s = update(s, [103, 100], 1, THIN_PX); // 3 px: thinned away
  s = update(s, [110, 100], 2.5, THIN_PX);
  s = update(s, [112, 100], 3, THIN_PX); // 2 px from the last kept one: thinned
  s = update(s, [200, 200], 35, THIN_PX);
  assert.equal(s.stroke?.pts.length, 3);
  assert.deepEqual(liveShape(s), { t: "freehand", pts: [[0.0625, 0.0833], [0.0688, 0.0833], [0.125, 0.1667]] });
  s = end(s, PX_PER_CSS);
  assert.equal(s.shapes.length, 1);
  assert.deepEqual(s.shapes[0], { t: "freehand", pts: [[0.0625, 0.0833], [0.0688, 0.0833], [0.125, 0.1667]] });
});

test("a tap selects the nearest shape or clears the selection, in any tool, however long it was held", () => {
  const a: Annotation = { v: 1, shapes: [{ t: "ellipse", cx: 0.5, cy: 0.5, rx: 0.25, ry: 0.25 }, { t: "arrow", x1: 0.1, y1: 0.9, x2: 0.9, y2: 0.9 }] };
  let s = setTool(initial(natural, a), "freehand");
  // On the ring (1200, 600): moved 7 CSS px in total, never more → a tap (no duration involved).
  s = begin(s, [1200, 600]);
  s = update(s, [1220, 600], 5, THIN_PX);
  s = update(s, [1228, 600], 7, THIN_PX);
  s = end(s, PX_PER_CSS);
  assert.equal(s.selected, 0);
  assert.equal(s.shapes.length, 2, "nothing drawn by a tap");
  // 14 CSS px = 56 image px from the shaft: within the 16 CSS px tolerance.
  s = end(begin(s, [800, 1080 - 56]), PX_PER_CSS);
  assert.equal(s.selected, 1);
  // 20 CSS px away: nothing there → cleared.
  s = end(begin(s, [800, 1080 - 80]), PX_PER_CSS);
  assert.equal(s.selected, null);
  // Delete needs a selection; undo pops the last shape and clears the selection.
  assert.equal(deleteSelected(s), s);
  s = end(begin(s, [1200, 600]), PX_PER_CSS);
  assert.equal(s.selected, 0);
  s = deleteSelected(s);
  assert.deepEqual(s.shapes, [a.shapes[1]]);
  assert.equal(s.selected, null);
  s = end(begin(s, [800, 1080]), PX_PER_CSS);
  assert.equal(s.selected, 0);
  s = undo(s);
  assert.deepEqual(s.shapes, []);
  assert.equal(s.selected, null);
  assert.equal(undo(s), s);
  assert.equal(commit(s), null);
});

test("a drag that moved 8 CSS px is a shape, not a tap; too-small shapes are dropped", () => {
  let s = initial(natural, null);
  // 8 CSS px of movement, but an ellipse 16 image px tall → rx 0 wide: too small.
  s = begin(s, [800, 600]);
  s = update(s, [800, 632], 8, THIN_PX);
  s = end(s, PX_PER_CSS);
  assert.deepEqual(s.shapes, []);
  // A short arrow (under two stroke widths = 24 px) is dropped too; a longer one is kept.
  s = setTool(s, "arrow");
  s = drag(s, [800, 600], [820, 600]);
  assert.deepEqual(s.shapes, []);
  s = drag(s, [800, 600], [900, 600]);
  assert.equal(s.shapes.length, 1);
});

test("cancel drops the stroke; a second pointer-down is ignored; the shape limit holds", () => {
  let s = initial(natural, null);
  s = begin(s, [100, 100]);
  s = begin(s, [500, 500]);
  assert.deepEqual(s.stroke?.start, [100, 100]);
  s = cancelStroke(s);
  assert.equal(s.stroke, null);
  assert.equal(cancelStroke(s), s);
  const full: Annotation = { v: 1, shapes: Array.from({ length: MAX_SHAPES }, () => ({ t: "ellipse", cx: 0.5, cy: 0.5, rx: 0.1, ry: 0.1 })) };
  const f = drag(initial(natural, full), [400, 300], [1200, 600]);
  assert.equal(f.shapes.length, MAX_SHAPES);
  assert.equal(f.limit, true, "a deliberate shape on a full photo says why nothing happened");
  // An accident (too small) on a full photo is dropped without a word.
  assert.equal(drag(initial(natural, full), [800, 600], [820, 600]).limit, false);
  // Making room lifts the limit again.
  const g = undo(f);
  assert.equal(g.limit, false);
  assert.equal(drag(g, [400, 300], [1200, 600]).shapes.length, MAX_SHAPES);
});

/** `n` points along a line, all inside the picture. */
const line = (n: number): XY[] => Array.from({ length: n }, (_, i) => [i / n, 0.5]);
/** A freehand stroke of `n` pointer moves 12 image px apart (over the thinning distance), snaking down the picture; still in progress. */
function scribble(s: EditorState, n: number): EditorState {
  let x = 100;
  let y = 100;
  let dir = 1;
  s = begin(s, [x, y]);
  for (let i = 0; i < n; i++) {
    x += 12 * dir;
    if (x > 1500 || x < 100) {
      dir = -dir;
      x += 12 * dir;
      y += 12;
    }
    s = update(s, [x, y], Math.hypot(x - 100, y - 100) / PX_PER_CSS, THIN_PX);
  }
  return s;
}
const points = (s: EditorState): number => s.shapes.reduce((n, sh) => n + (sh.t === "freehand" ? sh.pts.length : 0), 0);

test("a freehand stroke ends at MAX_POINTS: the rest of the gesture draws nothing and says so", () => {
  let s = setTool(initial(natural, null), "freehand");
  s = scribble(s, MAX_POINTS - 1); // the start plus 1999 moves: exactly the cap, nothing refused yet
  assert.equal(s.stroke?.pts.length, MAX_POINTS);
  assert.equal(s.limit, false);
  s = update(s, [1000, 900], 200, THIN_PX); // one more point than allowed
  assert.equal(s.stroke?.pts.length, MAX_POINTS);
  assert.equal(s.limit, true);
  s = scribble(end(s, PX_PER_CSS), 0); // the next stroke clears the hint
  assert.equal(s.limit, false);
  s = cancelStroke(s);

  s = scribble(s, MAX_POINTS + 500);
  assert.equal(s.stroke?.pts.length, MAX_POINTS);
  s = end(s, PX_PER_CSS);
  assert.equal(s.shapes.length, 2);
  assert.ok(s.shapes.every((sh) => sh.t === "freehand" && sh.pts.length === MAX_POINTS));
  assert.deepEqual(commit(s), parseAnnotation(commit(s)), "what Done hands back parses");
  assert.equal(commit(s)?.shapes.length, 2);
});

test("freehand strokes share MAX_TOTAL_POINTS: the last one is cut to what is left, then none starts", () => {
  // 2000 + 2000 + 1500 are stored: 500 points are left.
  const stored: Annotation = { v: 1, shapes: [{ t: "freehand", pts: line(2000) }, { t: "freehand", pts: line(2000) }, { t: "freehand", pts: line(1500) }] };
  assert.ok(parseAnnotation(stored));
  let s = setTool(initial(natural, stored), "freehand");
  assert.equal(pointBudget(s.shapes), 500);
  s = scribble(s, 1000);
  assert.equal(s.stroke?.pts.length, 500, "truncated to the budget");
  assert.equal(s.limit, true);
  s = end(s, PX_PER_CSS);
  assert.equal(s.shapes.length, 4);
  assert.equal(points(s), MAX_TOTAL_POINTS);
  assert.ok(parseAnnotation(commit(s)));

  // No budget left: a new freehand stroke draws nothing, shows nothing live, and says so.
  assert.equal(pointBudget(s.shapes), 0);
  s = scribble(s, 50);
  assert.equal(s.stroke?.pts.length, 1);
  assert.equal(liveShape(s), null);
  assert.equal(s.limit, true);
  s = end(s, PX_PER_CSS);
  assert.equal(s.shapes.length, 4);
  assert.equal(points(s), MAX_TOTAL_POINTS);

  // Fewer than MIN_POINTS left cannot make a shape either.
  const one: Annotation = { v: 1, shapes: [{ t: "freehand", pts: line(2000) }, { t: "freehand", pts: line(2000) }, { t: "freehand", pts: line(1999) }] };
  const t = scribble(setTool(initial(natural, one), "freehand"), 50);
  assert.equal(pointBudget(t.shapes), 1);
  assert.equal(liveShape(t), null);
  assert.equal(end(t, PX_PER_CSS).shapes.length, 3);

  // A tap still selects in the freehand tool with the budget gone, and Delete / Undo give room back.
  s = end(begin(s, [100, 100]), PX_PER_CSS);
  assert.notEqual(s.selected, null);
  assert.equal(s.limit, false, "a new gesture clears the hint");
  const deleted = deleteSelected(s);
  assert.ok(pointBudget(deleted.shapes) >= 500);
  assert.equal(deleted.limit, false);
  assert.equal(scribble(deleted, 10).stroke?.pts.length, 11);
  const undone = undo(end(scribble(s, 50), PX_PER_CSS));
  assert.equal(undone.limit, false);
  assert.equal(pointBudget(undone.shapes), 500, "the 500-point stroke is gone: its points are free again");
});

test("ellipse and arrow strokes are not held back by the point budget", () => {
  const stored: Annotation = { v: 1, shapes: [{ t: "freehand", pts: line(2000) }, { t: "freehand", pts: line(2000) }, { t: "freehand", pts: line(2000) }] };
  let s = initial(natural, stored);
  assert.equal(pointBudget(s.shapes), 0);
  s = drag(s, [400, 300], [1200, 600]);
  s = drag(setTool(s, "arrow"), [160, 1080], [1440, 120]);
  assert.deepEqual(s.shapes.slice(3).map((sh) => sh.t), ["ellipse", "arrow"]);
  assert.equal(s.limit, false);
});

test("Done never hands back what the core would refuse: bad shapes and over-budget shapes are left out", () => {
  const good: Shape = { t: "ellipse", cx: 0.5, cy: 0.5, rx: 0.1, ry: 0.1 };
  const arrow: Shape = { t: "arrow", x1: 0.1, y1: 0.1, x2: 0.9, y2: 0.9 };
  const state = (shapes: Shape[]): EditorState => ({ ...initial(natural, null), shapes });
  const mixed = state([
    good,
    { t: "ellipse", cx: 2, cy: 0.5, rx: 0.1, ry: 0.1 }, // out of range
    { t: "freehand", pts: line(MAX_POINTS + 1) }, // longer than a stroke may be
    { t: "freehand", pts: line(1) }, // too short
    { t: "freehand", pts: line(2000) },
    { t: "freehand", pts: line(2000) },
    { t: "freehand", pts: line(2000) },
    { t: "freehand", pts: line(2) }, // would be the 6002nd point
    arrow,
  ]);
  const a = commit(mixed);
  assert.deepEqual(a?.shapes.map((sh) => sh.t), ["ellipse", "freehand", "freehand", "freehand", "arrow"]);
  assert.deepEqual(a, parseAnnotation(a), "the same rules as parseAnnotation: it accepts the result as it is");
  const crowd = commit(state(Array.from({ length: MAX_SHAPES + 10 }, () => good)));
  assert.equal(crowd?.shapes.length, MAX_SHAPES);
  assert.ok(parseAnnotation(crowd));
  assert.equal(commit(state([{ t: "ellipse", cx: Number.NaN, cy: 0.5, rx: 0.1, ry: 0.1 }])), null, "nothing valid is nothing drawn");
});

test("initial copies the annotation's shapes and setTool is a no-op for the same tool", () => {
  const a: Annotation = { v: 1, shapes: [{ t: "arrow", x1: 0.1, y1: 0.1, x2: 0.5, y2: 0.5 }] };
  const s = initial(natural, a, "freehand");
  assert.notEqual(s.shapes, a.shapes);
  assert.deepEqual(s.shapes, a.shapes);
  assert.equal(setTool(s, "freehand"), s);
  assert.equal(setTool(s, "arrow").tool, "arrow");
  assert.equal(liveShape(s), null);
});
