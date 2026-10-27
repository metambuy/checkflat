// Rejections of the camera-capture plugin (system camera, Photo Picker, file picker) as the UI's
// errors. The Kotlin side gives each failure it understands a code (CameraCapturePlugin.kt) and the
// plugin's Rust error passes it on as `{ code, message }`; those codes have messages of their own.
// Anything else — an unknown code, a bare string from the dialog plugin, an exception text — shows
// the generic error, and its detail goes to dev.log (spike builds) instead of the screen.
import type { AppError } from "./api";

/** The codes the Kotlin plugin rejects with that the UI translates (`errors.ts`). */
export const PLUGIN_CODES: readonly string[] = ["no_camera_app", "no_photo_picker", "source_unreadable", "image_unsupported", "unreadable_image"];

export function pluginError(e: unknown, log: (line: string) => void): AppError {
  const o = e as { code?: unknown; message?: unknown } | null;
  if (o && typeof o === "object" && typeof o.code === "string" && PLUGIN_CODES.includes(o.code)) {
    return { code: o.code, message: typeof o.message === "string" ? o.message : "" };
  }
  const detail = o && typeof o === "object" && "message" in o ? `${String(o.code ?? "")} ${String(o.message)}`.trim() : String(e);
  log(`plugin error (shown as the generic one): ${detail}`);
  return { code: "internal", message: "" };
}
