import { test } from "node:test";
import assert from "node:assert/strict";
import { addressSaver, type SaveStatus } from "./addressSaver.ts";

function field(initial: string | null = "") {
  const f = {
    text: initial ?? "",
    stored: initial,
    puts: [] as string[],
    statuses: [] as SaveStatus[],
    errors: 0,
    fail: false,
  };
  const saver = addressSaver({
    get: () => f.text,
    stored: () => f.stored,
    put: async (v) => {
      f.puts.push(v);
      await new Promise((r) => setTimeout(r, 1));
      if (f.fail) throw new Error("db");
      f.stored = v;
    },
    status: (s) => f.statuses.push(s),
    error: () => f.errors++,
    delayMs: 10_000, // never fires within a test unless asked
  });
  const type = (text: string) => { f.text = text; saver.input(); };
  return { f, saver, type };
}

test("Back within the debounce: the pending edit is saved, not dropped", async () => {
  const { f, saver, type } = field("Rua A");
  type("Rua B, 12");
  await saver.flush(); // unmount
  assert.deepEqual(f.puts, ["Rua B, 12"]);
  assert.equal(f.stored, "Rua B, 12");
  assert.equal(f.statuses.at(-1), "saved");
});

test("flush with nothing pending saves nothing", async () => {
  const { f, saver, type } = field("Rua A");
  await saver.flush();
  type("Rua B");
  await saver.save(); // blur
  await saver.flush();
  assert.deepEqual(f.puts, ["Rua B"], "one save, by the blur");
});

test("typing stops: saved after the delay; typing more while saving saves again", async () => {
  const f = { text: "a", stored: "a" as string | null, puts: [] as string[] };
  const saver = addressSaver({
    get: () => f.text,
    stored: () => f.stored,
    put: async (v) => { f.puts.push(v); await new Promise((r) => setTimeout(r, 5)); f.stored = v; },
    status: () => {},
    error: () => {},
    delayMs: 2,
  });
  f.text = "ab";
  saver.input();
  await new Promise((r) => setTimeout(r, 4)); // the save of "ab" is in flight
  f.text = "abc";
  await new Promise((r) => setTimeout(r, 30));
  assert.deepEqual(f.puts, ["ab", "abc"]);
});

test("a failed save keeps the text and reports the error", async () => {
  const { f, saver, type } = field("Rua A");
  f.fail = true;
  type("Rua B");
  await saver.save();
  assert.equal(f.text, "Rua B");
  assert.equal(f.statuses.at(-1), "error");
  assert.equal(f.errors, 1);
});
