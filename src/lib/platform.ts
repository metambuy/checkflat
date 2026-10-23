// Which OS the app runs on and whether it is a debug build, from the backend's `platform_info`
// (`std::env::consts::OS`: "android", "windows", ...). Read once.
import { invoke } from "@tauri-apps/api/core";

interface PlatformInfo {
  os: string;
  debug: boolean;
}

let cached: Promise<PlatformInfo> | null = null;

const info = (): Promise<PlatformInfo> =>
  (cached ??= invoke<PlatformInfo>("platform_info").catch(() => ({ os: "unknown", debug: false })));

export const os = async (): Promise<string> => (await info()).os;

export const isAndroid = async (): Promise<boolean> => (await os()) === "android";

export const isDebug = async (): Promise<boolean> => (await info()).debug;
