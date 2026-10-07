// Where photos come from. Android: the camera-capture plugin (system camera, system Photo Picker)
// leaves a file in the app cache and the core processes it (`stage_photo`). A cancelled camera or
// picker resolves without a path and is not an error.
import { invoke } from "@tauri-apps/api/core";
import { api, type StagedPhoto } from "./api";

/** The device's zone in minutes east of UTC, for EXIF times that state none. */
const utcOffsetMin = () => -new Date().getTimezoneOffset();

async function fromPlugin(command: "capture" | "pick_image"): Promise<StagedPhoto | null> {
  const r = await invoke<{ path?: string | null }>(`plugin:camera-capture|${command}`);
  return r.path ? api.stagePhoto(r.path, utcOffsetMin()) : null;
}

/** System camera app → staged photo, or null when cancelled. */
export const captureStagedPhoto = () => fromPlugin("capture");

/** System Photo Picker → staged photo (HEIC converted on the way), or null when cancelled. */
export const pickStagedPhoto = () => fromPlugin("pick_image");
