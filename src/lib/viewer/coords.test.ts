import { test } from "node:test";
import assert from "node:assert/strict";
import { chooseLevel, clampPan, fitTransform, maxScale, refit, toNorm, toScreen } from "./coords.ts";

const world = { w: 8192, h: 5787 };
const close = (a: number, b: number, eps = 1e-9) => assert.ok(Math.abs(a - b) < eps, `${a} ≉ ${b}`);

test("screen ↔ normalised round trip at any transform", () => {
  for (const t of [fitTransform({ w: 360, h: 700 }, world), { x: -1234.5, y: 87.25, scale: 0.37 }, { x: 10, y: -9999, scale: 1.4 }]) {
    for (const [nx, ny] of [[0, 0], [1, 1], [0.25, 0.8], [0.5, 0.5]]) {
      const s = toScreen(t, world, nx, ny);
      const n = toNorm(t, world, s.x, s.y);
      close(n.x, nx);
      close(n.y, ny);
    }
  }
});

test("fit centres the plan", () => {
  const t = fitTransform({ w: 360, h: 700 }, world);
  const c = toNorm(t, world, 180, 350);
  close(c.x, 0.5);
  close(c.y, 0.5);
});

test("max zoom: phone keeps 8×, tablet capped at 1.5× upscaling", () => {
  const phoneFit = 360 / 8192; // P30 portrait, DPR 2
  close(maxScale(phoneFit, 2), phoneFit * 8);
  const tabletFit = Math.min(1280 / 8192, 752 / 5787); // 2560×1600 landscape, DPR 2
  close(maxScale(tabletFit, 2), 0.75);
  assert.ok(maxScale(tabletFit, 2) / tabletFit < 8);
  close(maxScale(2, 3), 2, 1e-12); // never below fit
});

test("level choice: smallest level with ≥ 1 source px per device px", () => {
  const sizes = [1024, 2048, 4096, 8192];
  assert.equal(chooseLevel(sizes, 8192, 0.044, 2), 0); // fit on a phone: 0.088 ≤ 1024/8192
  assert.equal(chooseLevel(sizes, 8192, 0.2, 2), 2); // 0.4 → 4096 (0.5)
  assert.equal(chooseLevel(sizes, 8192, 0.35, 2), 3); // 0.7 → 8192
  assert.equal(chooseLevel(sizes, 8192, 1, 2), 3); // beyond the top: top
  assert.equal(chooseLevel([1024, 2048], 8192, 0.35, 2), 1, "while generating: largest available");
});

test("pan is clamped so the view centre stays over the plan", () => {
  const view = { w: 400, h: 800 };
  const t = clampPan({ x: 5000, y: -99999, scale: 0.1 }, view, world);
  const c = toNorm(t, world, 200, 400);
  close(c.x, 0);
  close(c.y, 1);
});

test("refit keeps the centre point and the relative zoom", () => {
  const a = { w: 360, h: 700 }, b = { w: 700, h: 360 };
  const t = { x: -900, y: -300, scale: (360 / 8192) * 4 };
  const before = toNorm(t, world, a.w / 2, a.h / 2);
  const r = refit(t, a, b, world);
  const after = toNorm(r, world, b.w / 2, b.h / 2);
  close(after.x, before.x, 1e-9);
  close(after.y, before.y, 1e-9);
  close(r.scale / Math.min(b.w / world.w, b.h / world.h), 4, 1e-9);
});
