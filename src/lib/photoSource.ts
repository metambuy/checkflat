// Where photos come from. Android: the camera-capture plugin (system camera, system Photo Picker)
// leaves a file in the app cache and the core processes it (`stage_photo`). Windows: the gallery
// is a file picker and there is no camera. A cancelled camera or picker resolves without a path
// (null) and is not an error.
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { api, utcOffsetMin, type StagedPhoto } from "./api";
import { devlog } from "./devlog";
import { isAndroid } from "./platform";

const since = (t0: number) => Math.round(performance.now() - t0);
const why = (e: unknown) => String((e as { message?: unknown } | null)?.message ?? e);

/**
 * A camera or picker call with the time it took in dev.log (spike builds), so a slow "Preparing
 * photo…" can be told apart afterwards: camera/picker side here (and `adb logcat -s CheckflatPick`
 * for the plugin's own lines), core side in `stagePath` (D-026, D-027).
 */
async function timed(kind: "camera" | "pick", get: () => Promise<string | null>): Promise<string | null> {
  const t0 = performance.now();
  try {
    const path = await get();
    devlog(`${kind}: ${path ? "path" : "no path (cancelled)"} after ${since(t0)} ms`);
    return path;
  } catch (e) {
    devlog(`${kind}: failed after ${since(t0)} ms: ${why(e)}`);
    throw e;
  }
}

async function pathFrom(command: "capture" | "pick_image" | "take_recovered_capture"): Promise<string | null> {
  const r = await invoke<{ path?: string | null }>(`plugin:camera-capture|${command}`);
  return r.path ? r.path : null;
}

/** System camera app → path of the captured file, or null when cancelled. */
export const capturePath = () => timed("camera", () => pathFrom("capture"));

/** Formats the core can decode. HEIC is not offered on desktop: only the Android plugin converts it (D-022). */
const DESKTOP_IMAGE_EXTENSIONS = ["jpg", "jpeg", "png", "webp"];

async function pickFile(): Promise<string | null> {
  const f = await open({ multiple: false, directory: false, filters: [{ name: "Images", extensions: DESKTOP_IMAGE_EXTENSIONS }] });
  return typeof f === "string" && f ? f : null;
}

/**
 * Gallery: the system Photo Picker on Android (path of the picked file, HEIC already converted),
 * a file picker on desktop. null when cancelled.
 */
export const pickPath = (): Promise<string | null> => timed("pick", async () => ((await isAndroid()) ? pathFrom("pick_image") : pickFile()));

/** A photo taken while Android killed the app (see the plugin), once; null when there is none. */
export const recoveredCapturePath = () => pathFrom("take_recovered_capture");

/** Process a file from the plugin: orientation, ≤ 1600 px JPEG, capture time. */
export async function stagePath(path: string): Promise<StagedPhoto> {
  const t0 = performance.now();
  try {
    const staged = await api.stagePhoto(path, utcOffsetMin());
    devlog(`stage: done in ${since(t0)} ms`);
    return staged;
  } catch (e) {
    devlog(`stage: failed after ${since(t0)} ms: ${why(e)}`);
    throw e;
  }
}

export const captureStagedPhoto = async (): Promise<StagedPhoto | null> => {
  const p = await capturePath();
  return p ? stagePath(p) : null;
};
export const pickStagedPhoto = async (): Promise<StagedPhoto | null> => {
  const p = await pickPath();
  return p ? stagePath(p) : null;
};
