import { test } from "node:test";
import assert from "node:assert/strict";
import { singleFlight } from "./singleFlight.ts";

/** A fake generator: "loads the document", works until told to stop, then "destroys" it. */
function generator(log: string[], name: string, alive: { n: number; max: number }) {
  return async (superseded: () => boolean) => {
    if (superseded()) { log.push(`${name} skipped`); return name; }
    alive.n++;
    alive.max = Math.max(alive.max, alive.n);
    log.push(`${name} loaded`);
    try {
      for (let i = 0; i < 50 && !superseded(); i++) await new Promise((r) => setTimeout(r, 1));
      return name;
    } finally {
      await new Promise((r) => setTimeout(r, 2)); // doc.destroy()
      alive.n--;
      log.push(`${name} destroyed`);
    }
  };
}

test("a new run waits for the previous run's cancellation and cleanup", async () => {
  const log: string[] = [], alive = { n: 0, max: 0 };
  const a = singleFlight("plan", generator(log, "a", alive));
  await new Promise((r) => setTimeout(r, 3));
  const b = singleFlight("plan", generator(log, "b", alive));
  assert.deepEqual(await Promise.all([a, b]), ["a", "b"]);
  assert.equal(alive.max, 1, "never two documents alive");
  assert.deepEqual(log.slice(0, 3), ["a loaded", "a destroyed", "b loaded"]);
});

test("three quick opens: the middle one never loads, the last one runs", async () => {
  const log: string[] = [], alive = { n: 0, max: 0 };
  const a = singleFlight("plan", generator(log, "a", alive));
  const b = singleFlight("plan", generator(log, "b", alive));
  const c = singleFlight("plan", generator(log, "c", alive));
  await Promise.all([a, b, c]);
  assert.equal(alive.max, 1);
  assert.ok(log.includes("b skipped") && log.includes("c loaded"), log.join(", "));
});

test("a failed run releases the key; other keys are independent", async () => {
  await assert.rejects(singleFlight("plan", async () => { throw new Error("render"); }));
  assert.equal(await singleFlight("plan", async (s) => s()), false);
  let other = false;
  const slow = singleFlight("p1", async (s) => { await new Promise((r) => setTimeout(r, 5)); return s(); });
  await singleFlight("p2", async () => { other = true; });
  assert.equal(other, true, "p2 did not wait for p1");
  assert.equal(await slow, false, "p1 not cancelled by p2");
});
