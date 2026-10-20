import { test } from "node:test";
import assert from "node:assert/strict";
import { MAX_POINTS, MAX_SHAPES, parseAnnotation, strokeWidth, type Shape } from "./model.ts";

const ellipse = (over: Partial<Extract<Shape, { t: "ellipse" }>> = {}): Shape => ({ t: "ellipse", cx: 0.5, cy: 0.5, rx: 0.2, ry: 0.1, ...over });

test("the canonical format round-trips, with every shape type", () => {
  const json = '{"v":1,"shapes":[{"t":"ellipse","cx":0.52,"cy":0.4,"rx":0.12,"ry":0.09},{"t":"arrow","x1":0.1,"y1":0.8,"x2":0.45,"y2":0.5},{"t":"freehand","pts":[[0.2,0.2],[0.21,0.23]]}]}';
  const a = parseAnnotation(JSON.parse(json));
  assert.ok(a);
  assert.equal(a.shapes.length, 3);
  assert.equal(JSON.stringify(a), json, "serialises back to the same canonical JSON");
  // Extra fields are dropped, not copied (the object is rebuilt from the known fields).
  const b = parseAnnotation({ v: 1, shapes: [{ ...ellipse(), extra: 1 }], note: "x" });
  assert.deepEqual(b, { v: 1, shapes: [ellipse()] });
});

test("nothing drawn and invalid input are both null", () => {
  for (const v of [null, undefined, "", 42, [], {}, { v: 1 }, { v: 2, shapes: [ellipse()] }, { v: 1, shapes: [] }, { v: 1, shapes: {} }]) {
    assert.equal(parseAnnotation(v), null, JSON.stringify(v));
  }
  const bad: unknown[] = [
    { t: "square", x: 0 },
    ellipse({ cx: 1.5 }),
    ellipse({ cy: -0.1 }),
    ellipse({ rx: 0 }),
    ellipse({ ry: Number.NaN }),
    { t: "arrow", x1: 0.1, y1: 0.1, x2: 0.5 },
    { t: "arrow", x1: "0.1", y1: 0.1, x2: 0.5, y2: 0.5 },
    { t: "freehand", pts: [[0.1, 0.1]] },
    { t: "freehand", pts: [[0.1, 0.1], [0.1]] },
    { t: "freehand", pts: [[0.1, 0.1], [0.1, 1.1]] },
    { t: "freehand", pts: Array.from({ length: MAX_POINTS + 1 }, () => [0.1, 0.1]) },
  ];
  for (const s of bad) assert.equal(parseAnnotation({ v: 1, shapes: [s] }), null, JSON.stringify(s));
  // One bad shape invalidates the whole annotation (the core refuses it the same way).
  assert.equal(parseAnnotation({ v: 1, shapes: [ellipse(), ellipse({ rx: 2 })] }), null);
});

test("limits are inclusive", () => {
  const full = { v: 1, shapes: Array.from({ length: MAX_SHAPES }, () => ellipse()) };
  assert.equal(parseAnnotation(full)?.shapes.length, MAX_SHAPES);
  assert.equal(parseAnnotation({ v: 1, shapes: [...full.shapes, ellipse()] }), null);
  const pts = Array.from({ length: MAX_POINTS }, (_, i) => [i / MAX_POINTS, 0.5]);
  const three = { v: 1, shapes: [{ t: "freehand", pts }, { t: "freehand", pts }, { t: "freehand", pts }] };
  assert.equal(parseAnnotation(three)?.shapes.length, 3);
  assert.equal(parseAnnotation({ v: 1, shapes: [...three.shapes, { t: "freehand", pts }] }), null, "more than 6000 points in total");
});

test("stroke width follows the long side", () => {
  assert.equal(strokeWidth(800, 600), 6);
  assert.equal(strokeWidth(450, 800), 6);
  assert.equal(strokeWidth(1600, 1200), 12);
});
