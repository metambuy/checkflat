import test, { mock } from "node:test";
import assert from "node:assert/strict";
import { createDraftSaver, MAX_AGE_MS, parseDraft, recoverDraft, serializeDraft, type RecoveryDeps, type SheetDraft } from "./draft.ts";

const NOW = 1_800_000_000_000;
const photo = (t: string) => ({ token: t, path: `/data/tmp/${t}.jpg`, takenAt: "2026-10-08T09:00:00.000Z" });
const draft = (over: Partial<SheetDraft> = {}): SheetDraft => ({
  v: 1, mode: "create", projectId: "p", planId: "pl", observationId: null, x: 0.25, y: 0.75,
  fraction: "A", description: "crack", staged: [photo("t1")], removed: [], savedAt: NOW - 1000, ...over,
});

test("a draft round-trips", () => {
  const d = draft({ staged: [photo("t1"), photo("t2")], description: "Ático · área\nline 2" });
  assert.deepEqual(parseDraft(serializeDraft(d), NOW), d);
});

test("nothing, garbage, wrong shape, too old or from the future is no draft", () => {
  for (const raw of [null, undefined, "", "{", "[]", "null", "{}", JSON.stringify({ ...draft(), v: 2 })]) {
    assert.equal(parseDraft(raw as string, NOW), null, String(raw));
  }
  assert.equal(parseDraft(serializeDraft(draft({ savedAt: NOW - MAX_AGE_MS - 1 })), NOW), null, "expired");
  assert.equal(parseDraft(serializeDraft(draft({ savedAt: NOW + 3_600_000 })), NOW), null, "future");
  assert.equal(parseDraft(serializeDraft(draft({ x: 1.5 })), NOW), null, "pin outside the plan");
  assert.equal(parseDraft(serializeDraft(draft({ mode: "edit", observationId: null })), NOW), null, "edit needs its observation");
  assert.equal(parseDraft(JSON.stringify({ ...draft(), staged: [{ token: 1 }] }), NOW), null, "bad photo");
});

test("saver: rapid changes write once, the last one", async () => {
  mock.timers.enable({ apis: ["setTimeout"] });
  try {
    const puts: string[] = [];
    const s = createDraftSaver(async (r) => void puts.push(r), 400);
    s.schedule(draft({ description: "a" }));
    mock.timers.tick(200);
    s.schedule(draft({ description: "ab" }));
    mock.timers.tick(399);
    assert.equal(puts.length, 0);
    mock.timers.tick(1);
    await Promise.resolve();
    await Promise.resolve();
    assert.deepEqual(puts.map((r) => JSON.parse(r).description), ["ab"]);
  } finally {
    mock.timers.reset();
  }
});

test("saver: flush writes now; clear cancels a pending write and is not overtaken", async () => {
  mock.timers.enable({ apis: ["setTimeout"] });
  try {
    const puts: string[] = [];
    const s = createDraftSaver(async (r) => void puts.push(r), 400);
    await s.flush(draft({ description: "now" }));
    assert.deepEqual(puts.map((r) => JSON.parse(r).description), ["now"]);
    s.schedule(draft({ description: "pending" }));
    await s.clear();
    mock.timers.tick(1000);
    await Promise.resolve();
    assert.equal(puts.length, 2);
    assert.equal(puts[1], "", "the pending write never happens; the draft is cleared");
  } finally {
    mock.timers.reset();
  }
});

const deps = (over: Partial<RecoveryDeps> = {}): RecoveryDeps & { cleared: number; taken: number } => {
  const d = {
    cleared: 0,
    taken: 0,
    loadRaw: async () => serializeDraft(draft()),
    takeCapture: async () => { d.taken++; return null; },
    stage: async (p: string) => photo(`staged-${p}`),
    existingTokens: async (t: string[]) => t,
    targetExists: async () => true,
    clear: async () => { d.cleared++; },
    now: () => NOW,
    ...over,
  };
  return d as never;
};

test("recovery: no draft stored is nothing to do, but the plugin's capture is still drained", async () => {
  const d = deps({ loadRaw: async () => null });
  assert.equal(await recoverDraft(d), null);
  assert.equal(d.taken, 1);
  assert.equal(d.cleared, 0);
});

test("recovery: an unusable stored draft is cleared; so is one whose plan is gone", async () => {
  const bad = deps({ loadRaw: async () => "{broken" });
  assert.equal(await recoverDraft(bad), null);
  assert.equal(bad.cleared, 1);
  const gone = deps({ targetExists: async () => false });
  assert.equal(await recoverDraft(gone), null);
  assert.equal(gone.cleared, 1);
});

test("recovery: the photo taken behind the camera is added to the restored sheet", async () => {
  const d = deps({ takeCapture: async () => "/cache/captures/IMG_1.jpg" });
  const r = await recoverDraft(d);
  assert.deepEqual(r?.staged.map((p) => p.token), ["t1", "staged-/cache/captures/IMG_1.jpg"]);
  assert.equal(r?.description, "crack");
  assert.equal(r?.fraction, "A");
  assert.equal(d.cleared, 0, "the draft stays stored until the sheet closes");
});

test("recovery: staged photos whose file is gone are dropped; an unreadable capture is skipped", async () => {
  const d = deps({
    loadRaw: async () => serializeDraft(draft({ staged: [photo("t1"), photo("t2")] })),
    existingTokens: async (t) => t.filter((x) => x !== "t1"),
    takeCapture: async () => "/cache/captures/bad.jpg",
    stage: async () => { throw { code: "unreadable_image" }; },
  });
  const r = await recoverDraft(d);
  assert.deepEqual(r?.staged.map((p) => p.token), ["t2"]);
});

test("recovery: a failing plugin (desktop) does not block restoring the draft", async () => {
  const r = await recoverDraft(deps({ takeCapture: async () => { throw new Error("unsupported"); } }));
  assert.equal(r?.description, "crack");
});

test("recovery: an edit draft keeps its observation and removed photos", async () => {
  const e = draft({ mode: "edit", observationId: "o1", removed: ["ph1"], staged: [] });
  const r = await recoverDraft(deps({ loadRaw: async () => serializeDraft(e) }));
  assert.equal(r?.observationId, "o1");
  assert.deepEqual(r?.removed, ["ph1"]);
});
