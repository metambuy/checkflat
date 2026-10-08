// The observation sheet's unsaved input, kept in the `setting` table (SQLite: durable and
// synchronous, unlike WebView storage that may not be flushed when Android kills the app) while
// the sheet is open, so a photo taken while the app was killed behind the camera lands in the
// sheet it was meant for. Written on every change (debounced) and immediately before the camera or
// picker opens; cleared whenever the sheet closes (save, cancel, Back, leaving the screen).
import type { StagedPhoto } from "../api";

export const DRAFT_KEY = "sheet_draft";
/** Same as the `tmp/` rule for staged files: an older draft has nothing left to recover. */
export const MAX_AGE_MS = 24 * 3600 * 1000;

export interface SheetState {
  fraction: string;
  description: string;
  /** Photos already processed into tmp/ and not saved yet. */
  staged: StagedPhoto[];
  /** Stored photos (edit mode) marked for removal. */
  removed: string[];
}

export interface SheetDraft extends SheetState {
  v: 1;
  mode: "create" | "edit";
  projectId: string;
  planId: string;
  /** Edit mode: the observation being edited. */
  observationId: string | null;
  /** Create mode: the draft pin (0–1). */
  x: number;
  y: number;
  savedAt: number;
}

export const serializeDraft = (d: SheetDraft): string => JSON.stringify(d);

const isStr = (v: unknown): v is string => typeof v === "string";
const isUnit = (v: unknown): v is number => typeof v === "number" && Number.isFinite(v) && v >= 0 && v <= 1;

/** A stored draft, or null when there is none, it is malformed or it is too old. */
export function parseDraft(raw: string | null | undefined, now: number): SheetDraft | null {
  if (!raw) return null;
  let d: Record<string, unknown>;
  try {
    d = JSON.parse(raw);
  } catch {
    return null;
  }
  if (typeof d !== "object" || d === null || d.v !== 1) return null;
  if (d.mode !== "create" && d.mode !== "edit") return null;
  if (!isStr(d.projectId) || !isStr(d.planId) || !isStr(d.fraction) || !isStr(d.description)) return null;
  if (!isUnit(d.x) || !isUnit(d.y)) return null;
  if (typeof d.savedAt !== "number" || !(now - d.savedAt < MAX_AGE_MS) || d.savedAt > now + 60_000) return null;
  const observationId = d.observationId ?? null;
  if (observationId !== null && !isStr(observationId)) return null;
  if (d.mode === "edit" && observationId === null) return null;
  if (!Array.isArray(d.removed) || !d.removed.every(isStr)) return null;
  if (!Array.isArray(d.staged)) return null;
  const staged: StagedPhoto[] = [];
  for (const p of d.staged) {
    if (typeof p !== "object" || p === null || !isStr(p.token) || !isStr(p.path) || !isStr(p.takenAt)) return null;
    staged.push({ token: p.token, path: p.path, takenAt: p.takenAt });
  }
  return {
    v: 1,
    mode: d.mode,
    projectId: d.projectId,
    planId: d.planId,
    observationId,
    x: d.x,
    y: d.y,
    fraction: d.fraction,
    description: d.description,
    staged,
    removed: d.removed as string[],
    savedAt: d.savedAt,
  };
}

export interface DraftSaver {
  /** Write soon (coalesces rapid changes). */
  schedule(d: SheetDraft): void;
  /** Write now; resolves when it is stored. */
  flush(d: SheetDraft): Promise<void>;
  /** Forget the draft: cancels a pending write, then stores "". */
  clear(): Promise<void>;
}

/** Debounced, ordered writes: a clear can never be overtaken by an earlier write. */
export function createDraftSaver(put: (raw: string) => Promise<void>, delayMs = 400): DraftSaver {
  let timer: ReturnType<typeof setTimeout> | null = null;
  let latest: SheetDraft | null = null;
  let chain: Promise<void> = Promise.resolve();
  const enqueue = (raw: string) => (chain = chain.then(() => put(raw)).catch(() => {}));
  const cancelTimer = () => {
    if (timer !== null) clearTimeout(timer);
    timer = null;
  };
  const write = (): Promise<void> => {
    cancelTimer();
    const d = latest;
    latest = null;
    return d ? enqueue(serializeDraft(d)) : chain;
  };
  return {
    schedule(d) {
      latest = d;
      cancelTimer();
      timer = setTimeout(() => void write(), delayMs);
    },
    flush(d) {
      latest = d;
      return write();
    },
    clear() {
      cancelTimer();
      latest = null;
      return enqueue("");
    },
  };
}

// --- Startup recovery --------------------------------------------------------------------------
// At startup: if the sheet was open when the app died, bring back its input, plus the photo that
// was being taken (a capture that finished while the app was not running).

export interface RecoveryDeps {
  loadRaw: () => Promise<string | null>;
  /** The plugin's recovered capture (a file path), or null. Always drained, so a stale one never lingers. */
  takeCapture: () => Promise<string | null>;
  stage: (path: string) => Promise<StagedPhoto>;
  /** The subset of the tokens whose staged file still exists. */
  existingTokens: (tokens: string[]) => Promise<string[]>;
  /** The draft's plan (and, in edit mode, its observation) still exists. */
  targetExists: (d: SheetDraft) => Promise<boolean>;
  clear: () => Promise<void>;
  now: () => number;
}

/** The draft to restore (with the recovered capture added to its photos), or null. */
export async function recoverDraft(deps: RecoveryDeps): Promise<SheetDraft | null> {
  const raw = await deps.loadRaw();
  const draft = parseDraft(raw, deps.now());
  let captured: string | null = null;
  try {
    captured = await deps.takeCapture();
  } catch {
    // no plugin (desktop) or it failed: nothing recovered
  }
  if (!draft) {
    if (raw) await deps.clear(); // malformed or expired
    return null;
  }
  if (!(await deps.targetExists(draft))) {
    await deps.clear();
    return null;
  }
  let staged = draft.staged;
  if (captured) {
    try {
      staged = [...staged, await deps.stage(captured)];
    } catch (e) {
      console.warn("[recovery] the recovered capture could not be processed", e);
    }
  }
  const alive = new Set(await deps.existingTokens(staged.map((p) => p.token)));
  return { ...draft, staged: staged.filter((p) => alive.has(p.token)) };
}
