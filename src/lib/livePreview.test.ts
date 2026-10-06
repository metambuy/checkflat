import { test } from "node:test";
import assert from "node:assert/strict";
import { livePreview } from "./livePreview.ts";

type Deferred = { resolve: (s: string) => void; reject: (e: unknown) => void };

function harness() {
  const pending: Deferred[] = [];
  const shown: Array<[string | null, unknown]> = [];
  const p = livePreview<string>({
    compute: () => new Promise<string>((resolve, reject) => pending.push({ resolve, reject })),
    show: (text, error) => shown.push([text, error]),
    delayMs: 0,
  });
  return { p, pending, shown };
}

const tick = () => new Promise((r) => setTimeout(r, 1));

test("a stale reply never overwrites a newer one", async () => {
  const { p, pending, shown } = harness();
  p.update("a");
  await tick();
  p.update("ab");
  await tick();
  assert.equal(pending.length, 2);
  pending[1].resolve("AB");
  await tick();
  pending[0].resolve("A"); // the older request answers late
  await tick();
  assert.deepEqual(shown, [["AB", null]]);
});

test("errors reach show() unless superseded; cancel drops everything in flight", async () => {
  const { p, pending, shown } = harness();
  const first = p.now("x");
  await tick();
  pending[0].reject("bad");
  await first;
  assert.deepEqual(shown, [[null, "bad"]]);
  p.update("y");
  await tick();
  p.cancel();
  pending[1].resolve("Y");
  await tick();
  assert.equal(shown.length, 1);
});
