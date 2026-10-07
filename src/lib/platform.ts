// Which OS the app runs on, from the backend's `platform_info` (`std::env::consts::OS`: "android",
// "windows", ...). Read once.
import { invoke } from "@tauri-apps/api/core";

let cached: Promise<string> | null = null;

export const os = (): Promise<string> =>
  (cached ??= invoke<{ os: string }>("platform_info").then((p) => p.os).catch(() => "unknown"));

export const isAndroid = async (): Promise<boolean> => (await os()) === "android";
