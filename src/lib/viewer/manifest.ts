// What a tile manifest read from disk is good for. Pure (no Tauri, no PDF.js) so it is unit-tested.
import type { TileManifest } from "../api";

export interface TileSettings {
  version: number;
  tile: number;
  levels: readonly number[];
}

/** True when the manifest was produced with these settings (possibly unfinished). */
export function compatible(m: TileManifest | null, cfg: TileSettings): m is TileManifest {
  return !!m && m.version === cfg.version && m.tile === cfg.tile && m.levels.length <= cfg.levels.length && m.levels.every((l, i) => l.size === cfg.levels[i]);
}

export function isComplete(m: TileManifest | null, cfg: TileSettings): boolean {
  return compatible(m, cfg) && m.levels.length === cfg.levels.length;
}

/** May the viewer show this manifest? Only one made with the current settings and with at least
 * one finished level: the generator deletes any other cache before it starts, so handing such a
 * manifest to the viewer would show tiles that are about to disappear, with the old geometry. */
export function viewable(m: TileManifest | null, cfg: TileSettings): m is TileManifest {
  return compatible(m, cfg) && m.levels.length > 0;
}

/** What the plan stage shows. With at least one level the viewer stays up and generation state is
 * a chip over it; a failure always offers Retry, with or without finished levels. */
export interface StageView {
  main: "viewer" | "failed" | "preparing";
  chip: "progress" | "retry" | null;
}
export function stageView(hasLevels: boolean, failed: boolean, generating: boolean): StageView {
  if (!hasLevels) return { main: failed ? "failed" : "preparing", chip: null };
  return { main: "viewer", chip: failed ? "retry" : generating ? "progress" : null };
}
