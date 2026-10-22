import { test } from "node:test";
import assert from "node:assert/strict";
import { runningJob, startJob } from "./jobs.ts";

function deferred<T>() {
  let resolve!: (v: T) => void;
  let reject!: (e: unknown) => void;
  const promise = new Promise<T>((res, rej) => ((resolve = res), (reject = rej)));
  return { promise, resolve, reject };
}

test("one job per project while it runs; a remounted screen gets the same job", async () => {
  const d = deferred<string>();
  let runs = 0;
  const first = startJob("p", () => (runs++, d.promise));
  const second = startJob("p", () => (runs++, Promise.resolve("other")));
  assert.equal(second, first);
  assert.equal(runningJob("p"), first);
  assert.equal(runs, 1);
  assert.equal(runningJob("q"), null, "other projects are independent");
  d.resolve("done");
  assert.equal(await first, "done");
  assert.equal(runningJob("p"), null, "settled jobs are forgotten");
  assert.equal(await startJob("p", () => (runs++, Promise.resolve("again"))), "again");
  assert.equal(runs, 2);
});

test("a failed job is forgotten too, and its error reaches whoever awaits it", async () => {
  const d = deferred<string>();
  const job = startJob("f", () => d.promise);
  d.reject(new Error("boom"));
  await assert.rejects(job, /boom/);
  assert.equal(runningJob("f"), null);
});
