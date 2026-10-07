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

export const withExisting = (s: PhotoSet, existing: Photo[]): PhotoSet => ({ ...s, existing });

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
