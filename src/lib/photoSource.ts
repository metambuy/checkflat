// Where photos come from. Android: the camera-capture plugin (system camera, system Photo Picker)
// leaves a file in the app cache and the core processes it (`stage_photo`). Windows: the gallery
// is a file picker and there is no camera. A cancelled camera or picker resolves without a path
// (null) and is not an error.
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { api, type StagedPhoto } from "./api";
import { isAndroid } from "./platform";

/** The device's zone in minutes east of UTC, for EXIF times that state none. */
const utcOffsetMin = () => -new Date().getTimezoneOffset();

async function pathFrom(command: "capture" | "pick_image" | "take_recovered_capture"): Promise<string | null> {
  const r = await invoke<{ path?: string | null }>(`plugin:camera-capture|${command}`);
  return r.path ? r.path : null;
}

/** System camera app → path of the captured file, or null when cancelled. */
export const capturePath = () => pathFrom("capture");

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
export const pickPath = async (): Promise<string | null> => ((await isAndroid()) ? pathFrom("pick_image") : pickFile());

/** A photo taken while Android killed the app (see the plugin), once; null when there is none. */
export const recoveredCapturePath = () => pathFrom("take_recovered_capture");

/** Process a file from the plugin: orientation, ≤ 1600 px JPEG, capture time. */
export const stagePath = (path: string): Promise<StagedPhoto> => api.stagePhoto(path, utcOffsetMin());

export const captureStagedPhoto = async (): Promise<StagedPhoto | null> => {
  const p = await capturePath();
  return p ? stagePath(p) : null;
};
export const pickStagedPhoto = async (): Promise<StagedPhoto | null> => {
  const p = await pickPath();
  return p ? stagePath(p) : null;
};
