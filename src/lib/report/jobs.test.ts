import { test } from "node:test";
import assert from "node:assert/strict";
import { FOLLOW_INTERVAL_MS, followBackend, runningJob, startJob } from "./jobs.ts";

function deferred<T>() {
  let resolve!: (v: T) => void;
  let reject!: (e: unknown) => void;
  const promise = new Promise<T>((res, rej) => ((resolve = res), (reject = rej)));
  return { promise, resolve, reject };
}

test("one job per project while it runs; a remounted screen gets the same job and its info", async () => {
  const d = deferred<string>();
  let runs = 0;
  const first = startJob("p", "en", () => (runs++, d.promise));
  const second = startJob("p", "pt", () => (runs++, Promise.resolve("other")));
  assert.equal(second, first);
  assert.deepEqual(runningJob("p"), { job: first, info: "en" }, "the first run's info, not the second call's");
  assert.equal(runs, 1);
  assert.equal(runningJob("q"), null, "other projects are independent");
  d.resolve("done");
  assert.equal(await first, "done");
  assert.equal(runningJob("p"), null, "settled jobs are forgotten");
  assert.equal(await startJob("p", "pt", () => (runs++, Promise.resolve("again"))), "again");
  assert.equal(runs, 2);
});

test("a failed job is forgotten too, and its error reaches whoever awaits it", async () => {
  const d = deferred<string>();
  const job = startJob("f", null, () => d.promise);
  d.reject(new Error("boom"));
  await assert.rejects(job, /boom/);
  assert.equal(runningJob("f"), null);
});

type Run = { projectId: string; lang: string } | null;
/** A backend that answers `answers` in turn (then no run) and records its pauses. */
function backend(answers: Run[], onSleep: () => void = () => {}) {
  const b = {
    looks: 0,
    sleeps: [] as number[],
    running: async () => answers[b.looks++] ?? null,
    sleep: async (ms: number) => {
      b.sleeps.push(ms);
      onSleep();
    },
  };
  return b;
}

test("a reloaded page follows the backend's run of its project until it ends (D-029)", async () => {
  const b = backend([{ projectId: "p", lang: "pt" }, { projectId: "p", lang: "pt" }, null]);
  const starts: string[] = [];
  assert.equal(await followBackend("p", b, new AbortController().signal, (l) => starts.push(l)), true);
  assert.deepEqual(starts, ["pt"], "the run's language, once");
  assert.equal(b.looks, 3);
  assert.deepEqual(b.sleeps, [FOLLOW_INTERVAL_MS, FOLLOW_INTERVAL_MS]);
});

test("no run, or another project's run: nothing to follow", async () => {
  for (const answers of [[null], [{ projectId: "q", lang: "en" }]]) {
    const b = backend(answers);
    let started = false;
    assert.equal(await followBackend("p", b, new AbortController().signal, () => (started = true)), false);
    assert.equal(started, false);
    assert.equal(b.looks, 1);
    assert.deepEqual(b.sleeps, []);
  }
});

test("following stops when the run ends and another project's starts, or when the screen is left", async () => {
  const b = backend([{ projectId: "p", lang: "en" }, { projectId: "q", lang: "pt" }]);
  assert.equal(await followBackend("p", b, new AbortController().signal, () => {}), true);
  assert.equal(b.looks, 2);

  const left = new AbortController();
  const forever = backend(Array(100).fill({ projectId: "p", lang: "en" }), () => left.abort());
  assert.equal(await followBackend("p", forever, left.signal, () => {}), true);
  assert.equal(forever.looks, 1, "no further look once the screen is gone");
});
