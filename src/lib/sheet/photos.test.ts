import test from "node:test";
import assert from "node:assert/strict";
import type { Phase } from "./photos.ts";
import { acquirePhoto, addStaged, canSave, count, noPhotos, removeExisting, removeStaged, shown, toSave, watchReturn, withExisting } from "./photos.ts";

const stored = (id: string) => ({ id, observationId: "o", path: `/data/projects/p/photos/${id}.jpg`, takenAt: "2026-10-05T12:00:00.000Z" });
const staged = (token: string) => ({ token, path: `/data/tmp/${token}.jpg`, takenAt: "2026-10-06T08:00:00.000Z" });

test("a new observation needs one staged photo", () => {
  assert.equal(canSave(noPhotos, false), false);
  const s = addStaged(noPhotos, staged("t1"));
  assert.equal(canSave(s, false), true);
  assert.equal(canSave(s, true), false, "not while a photo is being processed");
  assert.equal(canSave(removeStaged(s, "t1"), false), false);
});

test("adding the same token twice keeps one", () => {
  const s = addStaged(addStaged(noPhotos, staged("t1")), staged("t1"));
  assert.equal(count(s), 1);
});

test("stored photos show first, staged after, in the order added", () => {
  let s = withExisting(noPhotos, [stored("a"), stored("b")]);
  s = addStaged(addStaged(s, staged("t1")), staged("t2"));
  assert.deepEqual(shown(s).map((p) => p.key), ["a", "b", "t1", "t2"]);
  assert.deepEqual(shown(s).map((p) => p.staged), [false, false, true, true]);
});

test("removing a stored photo hides it and is sent to the save; removing twice or an unknown id is a no-op", () => {
  let s = withExisting(noPhotos, [stored("a"), stored("b")]);
  s = removeExisting(removeExisting(removeExisting(s, "a"), "a"), "zzz");
  assert.deepEqual(shown(s).map((p) => p.key), ["b"]);
  assert.deepEqual(toSave(s), { photos: [], removed: ["a"] });
});

test("an observation migrated without photos cannot be saved until one is added", () => {
  const s = withExisting(noPhotos, []);
  assert.equal(canSave(s, false), false);
  assert.equal(canSave(addStaged(s, staged("t1")), false), true);
});

test("removing the last stored photo needs a replacement", () => {
  const s = removeExisting(withExisting(noPhotos, [stored("a")]), "a");
  assert.equal(canSave(s, false), false);
  const swapped = addStaged(s, staged("t1"));
  assert.equal(canSave(swapped, false), true);
  assert.deepEqual(toSave(swapped), { photos: [{ token: "t1", takenAt: "2026-10-06T08:00:00.000Z" }], removed: ["a"] });
});

const track = () => {
  const phases: Phase[] = [];
  return { phases, onPhase: (p: Phase) => phases.push(p) };
};

/** A controllable stand-in for the window/document events. */
const fakeApp = () => {
  const win = new EventTarget();
  const doc = Object.assign(new EventTarget(), { visibilityState: "visible" });
  return {
    win,
    doc,
    leave: () => win.dispatchEvent(new Event("blur")),
    come_back: () => win.dispatchEvent(new Event("focus")),
    hide: () => { doc.visibilityState = "hidden"; doc.dispatchEvent(new Event("visibilitychange")); },
    show: () => { doc.visibilityState = "visible"; doc.dispatchEvent(new Event("visibilitychange")); },
  };
};
const tick = () => new Promise<void>((r) => setTimeout(r, 0));

test("backing out of the picker adds nothing, is not an error, and ends idle", async () => {
  const { phases, onPhase } = track();
  let processed = 0;
  const r = await acquirePhoto({ getPath: async () => null, stage: async () => { processed++; return staged("x"); } }, onPhase);
  assert.deepEqual(r, { staged: null, error: null });
  assert.deepEqual(phases, ["picking", "idle"]);
  assert.equal(processed, 0, "nothing is processed");
  // A missing or empty path is a cancel too.
  const r2 = await acquirePhoto({ getPath: async () => "", stage: async () => { throw new Error("no"); } }, track().onPhase);
  assert.deepEqual(r2, { staged: null, error: null });
});

test("backing out after control returned shows 'preparing' only until the cancel arrives, then idle", async () => {
  const app = fakeApp();
  const { phases, onPhase } = track();
  let finish: (p: string | null) => void = () => {};
  const done = acquirePhoto(
    { getPath: () => new Promise((r) => (finish = r)), stage: async () => staged("x"), watchReturn: () => watchReturn(app.win, app.doc) },
    onPhase,
  );
  app.leave(); // the picker opens
  app.come_back(); // the user backs out: control is back, the plugin is about to answer
  await tick();
  assert.deepEqual(phases, ["picking", "preparing"]);
  finish(null); // the plugin resolves without a path
  assert.deepEqual(await done, { staged: null, error: null });
  assert.deepEqual(phases, ["picking", "preparing", "idle"]);
});

test("'preparing' starts when control comes back, before the path (HEIC conversion), and lasts through processing", async () => {
  const app = fakeApp();
  const { phases, onPhase } = track();
  let finishPick: (p: string | null) => void = () => {};
  let finishStage: (p: ReturnType<typeof staged>) => void = () => {};
  const done = acquirePhoto(
    {
      getPath: () => new Promise((r) => (finishPick = r)),
      stage: () => new Promise((r) => (finishStage = r)),
      watchReturn: () => watchReturn(app.win, app.doc),
    },
    onPhase,
  );
  assert.deepEqual(phases, ["picking"], "the picker is open");
  app.hide(); // the camera/picker covers the page
  app.show(); // the user chose a photo; the plugin is converting it
  await tick();
  assert.deepEqual(phases, ["picking", "preparing"]);
  finishPick("/cache/x.jpg");
  await tick();
  finishStage(staged("t9"));
  const r = await done;
  assert.equal(r.staged?.token, "t9");
  assert.deepEqual(phases, ["picking", "preparing", "preparing", "idle"]);
});

test("without a return signal, 'preparing' starts when the path arrives", async () => {
  const { phases, onPhase } = track();
  let phaseWhenPickerClosed: Phase | undefined;
  const r = await acquirePhoto(
    { getPath: async () => { phaseWhenPickerClosed = phases[phases.length - 1]; return "/cache/x.jpg"; }, stage: async (p) => staged(p) },
    onPhase,
  );
  assert.equal(phaseWhenPickerClosed, "picking", "not 'preparing' while the picker is open");
  assert.deepEqual(phases, ["picking", "preparing", "idle"]);
  assert.equal(r.error, null);
  assert.equal(r.staged?.token, "/cache/x.jpg");
});

test("a focus event without a departure does not count as a return", async () => {
  const app = fakeApp();
  const { phases, onPhase } = track();
  await acquirePhoto(
    { getPath: async () => { app.come_back(); await tick(); return null; }, stage: async (p) => staged(p), watchReturn: () => watchReturn(app.win, app.doc) },
    onPhase,
  );
  assert.deepEqual(phases, ["picking", "idle"]);
});

test("the listeners are removed and a late return cannot revive the sheet", async () => {
  const app = fakeApp();
  const { phases, onPhase } = track();
  await acquirePhoto(
    { getPath: async () => null, stage: async (p) => staged(p), watchReturn: () => watchReturn(app.win, app.doc) },
    onPhase,
  );
  app.leave();
  app.come_back();
  await tick();
  assert.deepEqual(phases, ["picking", "idle"], "nothing after idle");
});

test("a failing picker or a failing processing step ends idle with the error", async () => {
  const boom = new Error("picker failed");
  const a = track();
  const ra = await acquirePhoto({ getPath: async () => { throw boom; }, stage: async (p) => staged(p) }, a.onPhase);
  assert.equal(ra.staged, null);
  assert.equal(ra.error, boom);
  assert.deepEqual(a.phases, ["picking", "idle"]);

  const bad = { code: "unreadable_image", message: "x" };
  const b = track();
  const rb = await acquirePhoto({ getPath: async () => "/p.jpg", stage: async () => { throw bad; } }, b.onPhase);
  assert.equal(rb.error, bad);
  assert.deepEqual(b.phases, ["picking", "preparing", "idle"]);
});

test("the sheet cannot save while an add is in progress, and can again once it ended (also after a cancel)", async () => {
  const s = addStaged(noPhotos, staged("t1"));
  let busy = false;
  const seen: boolean[] = [];
  await acquirePhoto(
    { getPath: async () => { seen.push(canSave(s, busy)); return null; }, stage: async (p) => staged(p) },
    (p) => (busy = p !== "idle"),
  );
  assert.deepEqual(seen, [false], "disabled while the picker is open");
  assert.equal(canSave(s, busy), true, "enabled again after a cancel");
});

test("a restored draft's removals of photos that no longer exist are dropped", () => {
  const s = withExisting({ ...noPhotos, removed: ["gone", "a"] }, [stored("a"), stored("b")]);
  assert.deepEqual(s.removed, ["a"]);
  assert.deepEqual(shown(s).map((p) => p.key), ["b"]);
});
