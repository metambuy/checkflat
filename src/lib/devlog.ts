// Spike builds only (dev, or a release built with VITE_SPIKES=1): the Dev screen, and timing lines
// appended to dev.log on the device (D-014 measurements). Both constants are replaced at build time,
// so normal builds drop the calls.
import { invoke } from "@tauri-apps/api/core";

export const SPIKES = import.meta.env.DEV || import.meta.env.VITE_SPIKES === "1";

export function devlog(line: string): void {
  if (!SPIKES) return;
  const l = `${new Date().toISOString()} ${line}`;
  console.log("[dev]", l);
  invoke("dev_log", { lines: [l] }).catch(() => {});
}
