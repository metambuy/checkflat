// Where photos come from. Android: the camera-capture plugin (system camera, system Photo Picker)
// leaves a file in the app cache and the core processes it (`stage_photo`). A cancelled camera or
// picker resolves without a path (null) and is not an error.
import { invoke } from "@tauri-apps/api/core";
import { api, type StagedPhoto } from "./api";

/** The device's zone in minutes east of UTC, for EXIF times that state none. */
const utcOffsetMin = () => -new Date().getTimezoneOffset();

async function pathFrom(command: "capture" | "pick_image"): Promise<string | null> {
  const r = await invoke<{ path?: string | null }>(`plugin:camera-capture|${command}`);
  return r.path ? r.path : null;
}

/** System camera app → path of the captured file, or null when cancelled. */
export const capturePath = () => pathFrom("capture");

/** System Photo Picker → path of the picked file (HEIC already converted), or null when cancelled. */
export const pickPath = () => pathFrom("pick_image");

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
