// Photos of the observation sheet as pure state. `existing` are stored photos (edit mode),
// `staged` are processed files waiting for Save, `removed` are stored photos marked for removal.
// The save takes the staged ones and the removed ids; at least one photo must remain.
import type { Photo, PhotoInput, StagedPhoto } from "../api";

export interface PhotoSet {
  existing: Photo[];
  staged: StagedPhoto[];
  removed: string[];
}

export interface PhotoItem {
  /** Stable key: the photo id (stored) or the token (staged). */
  key: string;
  /** Absolute path for convertFileSrc. */
  path: string;
  staged: boolean;
}

export const noPhotos: PhotoSet = { existing: [], staged: [], removed: [] };

/** Stored photos arrived (edit mode); removals of photos that no longer exist (a restored draft) are dropped. */
export const withExisting = (s: PhotoSet, existing: Photo[]): PhotoSet => ({
  ...s,
  existing,
  removed: s.removed.filter((id) => existing.some((p) => p.id === id)),
});

export const addStaged = (s: PhotoSet, p: StagedPhoto): PhotoSet =>
  s.staged.some((x) => x.token === p.token) ? s : { ...s, staged: [...s.staged, p] };

export const removeStaged = (s: PhotoSet, token: string): PhotoSet => ({ ...s, staged: s.staged.filter((p) => p.token !== token) });

export const removeExisting = (s: PhotoSet, id: string): PhotoSet =>
  s.existing.some((p) => p.id === id) && !s.removed.includes(id) ? { ...s, removed: [...s.removed, id] } : s;

/** What the sheet shows: the stored photos not marked for removal, then the staged ones, in the order added. */
export function shown(s: PhotoSet): PhotoItem[] {
  return [
    ...s.existing.filter((p) => !s.removed.includes(p.id)).map((p) => ({ key: p.id, path: p.path, staged: false })),
    ...s.staged.map((p) => ({ key: p.token, path: p.path, staged: true })),
  ];
}

export const count = (s: PhotoSet): number => shown(s).length;

/** Save is allowed with at least one photo and nothing being processed. */
export const canSave = (s: PhotoSet, busy: boolean): boolean => !busy && count(s) >= 1;

export const toSave = (s: PhotoSet): { photos: PhotoInput[]; removed: string[] } => ({
  photos: s.staged.map((p) => ({ token: p.token, takenAt: p.takenAt })),
  removed: [...s.removed],
});

/**
 * Where an add is: "picking" while the camera/picker is in front (waiting for the user), "preparing"
 * from the moment control comes back to the app until the file is processed (this covers the
 * plugin's HEIC conversion and a cloud-backed file's download, which happen before the path comes
 * back, and the core's processing after it).
 */
export type Phase = "idle" | "picking" | "preparing";

/** The app left (window blur, page hidden) and is back (focus, page visible). */
export interface ReturnWatch {
  returned: Promise<void>;
  stop: () => void;
}

type Win = { addEventListener(t: string, f: () => void): void; removeEventListener(t: string, f: () => void): void };
type Doc = Win & { visibilityState: string };

/**
 * The page cannot hear the plugin until its call settles, which on a HEIC is after the conversion;
 * the window's own focus and visibility say when the camera/picker handed control back. Only a
 * return after a departure counts.
 */
export function watchReturn(win: Win, doc: Doc): ReturnWatch {
  let left = false;
  let resolve: () => void = () => {};
  const returned = new Promise<void>((r) => (resolve = r));
  const out = () => void (left = true);
  const back = () => {
    if (left) resolve();
  };
  const visibility = () => (doc.visibilityState === "hidden" ? out() : back());
  win.addEventListener("blur", out);
  win.addEventListener("focus", back);
  doc.addEventListener("visibilitychange", visibility);
  return {
    returned,
    stop: () => {
      win.removeEventListener("blur", out);
      win.removeEventListener("focus", back);
      doc.removeEventListener("visibilitychange", visibility);
    },
  };
}

export interface PhotoSource {
  /** Camera or picker: the file's path, or null when the user backed out. */
  getPath: () => Promise<string | null>;
  stage: (path: string) => Promise<StagedPhoto>;
  /** Optional: when control came back to the app from the camera/picker. */
  watchReturn?: () => ReturnWatch;
}

/**
 * One add of a photo. "preparing" starts when control comes back (the signal from `watchReturn`)
 * or, at the latest, when a path arrives, and lasts until the photo is processed; every way out
 * (cancelled, failed, done) ends in "idle", so the sheet can never be left waiting. A cancel is
 * `{ staged: null, error: null }`.
 */
export async function acquirePhoto(
  source: PhotoSource,
  onPhase: (p: Phase) => void,
): Promise<{ staged: StagedPhoto | null; error: unknown }> {
  let finished = false;
  let watch: ReturnWatch | undefined;
  onPhase("picking");
  try {
    // Inside the try: whatever fails here still ends in "idle".
    watch = source.watchReturn?.();
    void watch?.returned.then(() => {
      if (!finished) onPhase("preparing");
    });
    const path = await source.getPath();
    if (!path) return { staged: null, error: null };
    onPhase("preparing");
    return { staged: await source.stage(path), error: null };
  } catch (error) {
    return { staged: null, error };
  } finally {
    finished = true;
    watch?.stop();
    onPhase("idle");
  }
}
