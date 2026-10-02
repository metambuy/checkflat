import { test } from "node:test";
import assert from "node:assert/strict";
import { compatible, isComplete, stageView, viewable } from "./manifest.ts";

const cfg = { version: 2, tile: 512, levels: [1024, 2048, 4096, 8192] };
const level = (size: number) => ({ size, width: size, height: Math.ceil(size * 0.7), cols: size / 512, rows: Math.ceil((size * 0.7) / 512) });
const manifest = (version: number, tile: number, sizes: number[]) => ({ version, tile, widthPt: 2384, heightPt: 1684, levels: sizes.map(level) });

test("a manifest from other generator settings is never shown: its tiles are about to be cleared", () => {
  const all = [1024, 2048, 4096, 8192];
  assert.equal(viewable(manifest(1, 512, all), cfg), false, "older version");
  assert.equal(viewable(manifest(2, 256, all), cfg), false, "other tile size");
  assert.equal(viewable(manifest(2, 512, [1024, 3072]), cfg), false, "other levels");
  assert.equal(viewable(manifest(2, 512, [...all, 16384]), cfg), false, "more levels than configured");
  assert.equal(viewable(null, cfg), false);
});

test("a current manifest is shown once it has a level, and is complete with all of them", () => {
  assert.equal(viewable(manifest(2, 512, []), cfg), false, "nothing to show yet");
  assert.equal(compatible(manifest(2, 512, []), cfg), true, "but resumable");
  assert.equal(viewable(manifest(2, 512, [1024]), cfg), true);
  assert.equal(isComplete(manifest(2, 512, [1024]), cfg), false);
  assert.equal(isComplete(manifest(2, 512, [1024, 2048, 4096, 8192]), cfg), true);
});

test("a failed generation always offers Retry, also when some levels exist", () => {
  assert.deepEqual(stageView(false, true, false), { main: "failed", chip: null });
  assert.deepEqual(stageView(true, true, false), { main: "viewer", chip: "retry" }, "viewer stays, Retry over it");
  assert.deepEqual(stageView(true, true, true), { main: "viewer", chip: "retry" }, "failure wins over stale progress");
  assert.deepEqual(stageView(true, false, true), { main: "viewer", chip: "progress" });
  assert.deepEqual(stageView(true, false, false), { main: "viewer", chip: null });
  assert.deepEqual(stageView(false, false, true), { main: "preparing", chip: null });
});
