import test from "node:test";
import assert from "node:assert/strict";
import { saveThenRefresh } from "./flow.ts";

const run = (over: { save?: () => Promise<void>; refresh?: () => Promise<void> }) => {
  const events: string[] = [];
  const f = {
    save: async () => void events.push("save"),
    refresh: async () => void events.push("refresh"),
    ...over,
    onSaved: () => void events.push("closed"),
    onSaveError: (e: unknown) => void events.push(`save-error:${(e as Error).message}`),
    onRefreshError: (e: unknown) => void events.push(`refresh-error:${(e as Error).message}`),
  };
  return { events, done: saveThenRefresh(f) };
};

test("a successful save closes the sheet before the list is refreshed", async () => {
  const { events, done } = run({});
  assert.equal(await done, true);
  assert.deepEqual(events, ["save", "closed", "refresh"]);
});

test("a failed save keeps the sheet open (no close, no refresh) and reports on the sheet", async () => {
  const { events, done } = run({ save: async () => { throw new Error("boom"); } });
  assert.equal(await done, false);
  assert.deepEqual(events, ["save-error:boom"]);
});

test("a failing refresh after a successful save leaves the sheet closed and reports on the screen", async () => {
  const { events, done } = run({ refresh: async () => { throw new Error("db"); } });
  assert.equal(await done, true, "the save did succeed");
  assert.deepEqual(events, ["save", "closed", "refresh-error:db"]);
});
