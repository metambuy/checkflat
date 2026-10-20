import { test } from "node:test";
import assert from "node:assert/strict";
import { arrowHead, distanceToPolyline, fitBox, hitTest, outlinePoints, toImage, tooSmall } from "./geometry.ts";
import type { Shape } from "./model.ts";

const close = (a: number, b: number, eps = 1e-9) => assert.ok(Math.abs(a - b) < eps, `${a} ≉ ${b}`);

test("fitBox centres the picture at the largest contained size", () => {
  assert.deepEqual(fitBox({ w: 400, h: 600 }, { w: 1600, h: 1200 }), { x: 0, y: 150, w: 400, h: 300 });
  // Portrait picture, still width-bound in a 400×600 stage.
  const b = fitBox({ w: 400, h: 600 }, { w: 1200, h: 1600 });
  close(b.x, 0);
  close(b.y, 100 / 3);
  close(b.w, 400);
  close(b.h, 1600 / 3);
  // Height-bound.
  const c = fitBox({ w: 800, h: 300 }, { w: 1600, h: 1200 });
  assert.deepEqual(c, { x: 200, y: 0, w: 400, h: 300 });
  assert.deepEqual(fitBox({ w: 0, h: 600 }, { w: 1600, h: 1200 }), { x: 0, y: 0, w: 0, h: 0 });
});

test("toImage maps the box to image pixels and clamps outside it", () => {
  const box = { x: 0, y: 150, w: 400, h: 300 };
  const natural = { w: 1600, h: 1200 };
  assert.deepEqual(toImage(0, 150, box, natural), [0, 0]);
  assert.deepEqual(toImage(400, 450, box, natural), [1600, 1200]);
  assert.deepEqual(toImage(100, 225, box, natural), [400, 300]);
  assert.deepEqual(toImage(-50, 1000, box, natural), [0, 1200]);
  assert.deepEqual(toImage(5, 5, { x: 0, y: 0, w: 0, h: 0 }, natural), [0, 0]);
});

/** Same fixture as `annotation::tests::chevron_geometry_matches_the_frontend_fixture` in the core. */
test("arrow head legs match the Rust fixture", () => {
  const legs = arrowHead([0, 0], [100, 0], 10);
  assert.ok(legs);
  close(legs[0][0], 91.33974596215562);
  close(legs[0][1], -5);
  close(legs[1][0], 91.33974596215562);
  close(legs[1][1], 5);
  const up = arrowHead([50, 100], [50, 0], 10);
  assert.ok(up);
  close(up[0][0], 45);
  close(up[0][1], 8.660254037844386);
  close(up[1][0], 55);
  close(up[1][1], 8.660254037844386);
  assert.equal(arrowHead([1, 1], [1, 1], 10), null);
});

test("distance to a polyline is the distance to its nearest segment", () => {
  const line = [[0, 0], [100, 0]] as [number, number][];
  close(distanceToPolyline([50, 7], line), 7);
  close(distanceToPolyline([-3, 4], line), 5, 1e-9); // before the first point
  close(distanceToPolyline([100, 0], line), 0);
  close(distanceToPolyline([5, 5], [[0, 0]]), Math.SQRT2 * 5);
  assert.equal(distanceToPolyline([0, 0], []), Number.POSITIVE_INFINITY);
});

test("hitTest picks the nearest outline within the tolerance", () => {
  const shapes: Shape[] = [
    { t: "ellipse", cx: 0.5, cy: 0.5, rx: 0.25, ry: 0.25 },
    { t: "arrow", x1: 0.1, y1: 0.9, x2: 0.9, y2: 0.9 },
    { t: "freehand", pts: [[0.1, 0.1], [0.2, 0.1]] },
  ];
  const [w, h] = [800, 600];
  assert.equal(hitTest(shapes, [600, 300], w, h, 16), 0, "on the ring");
  assert.equal(hitTest(shapes, [400, 300], w, h, 16), null, "the ellipse's centre is not its outline");
  assert.equal(hitTest(shapes, [400, 545], w, h, 16), 1, "near the arrow shaft");
  assert.equal(hitTest(shapes, [400, 520], w, h, 16), null, "20 px away: outside the tolerance");
  assert.equal(hitTest(shapes, [120, 62], w, h, 16), 2);
  assert.equal(hitTest(shapes, [600, 300], w, h, 16), 0);
  assert.equal(hitTest([], [1, 1], w, h, 16), null);
  const ring = outlinePoints(shapes[0], w, h);
  assert.equal(ring.length, 65, "closed: the first point repeats at the end");
  assert.deepEqual(ring[0], ring[64]);
});

test("too small: thinner than the stroke, shorter than two strokes, or a single point", () => {
  const [w, h, stroke] = [800, 600, 6];
  assert.equal(tooSmall({ t: "ellipse", cx: 0.5, cy: 0.5, rx: 0.005, ry: 0.1 }, w, h, stroke), true);
  assert.equal(tooSmall({ t: "ellipse", cx: 0.5, cy: 0.5, rx: 0.01, ry: 0.01 }, w, h, stroke), false);
  assert.equal(tooSmall({ t: "arrow", x1: 0.5, y1: 0.5, x2: 0.51, y2: 0.5 }, w, h, stroke), true);
  assert.equal(tooSmall({ t: "arrow", x1: 0.5, y1: 0.5, x2: 0.52, y2: 0.5 }, w, h, stroke), false);
  assert.equal(tooSmall({ t: "freehand", pts: [[0.5, 0.5]] }, w, h, stroke), true);
  assert.equal(tooSmall({ t: "freehand", pts: [[0.5, 0.5], [0.6, 0.5]] }, w, h, stroke), false);
});
