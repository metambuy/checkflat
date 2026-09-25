import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import { rmSync } from "node:fs";
import { resolve } from "node:path";

const host = process.env.TAURI_DEV_HOST;

// public/spike/ holds the client's plan (gitignored, see D-001). Vite copies public/ into every
// build, so drop it unless this is a spike build (VITE_SPIKES=1); CI never has the file.
const stripSpikeAssets = {
  name: "strip-spike-assets",
  apply: "build" as const,
  closeBundle() {
    if (process.env.VITE_SPIKES !== "1") rmSync(resolve(__dirname, "dist/spike"), { recursive: true, force: true });
  },
};

// https://v2.tauri.app/start/frontend/vite/
export default defineConfig({
  plugins: [svelte(), stripSpikeAssets],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host ? { protocol: "ws", host, port: 1421 } : undefined,
    watch: { ignored: ["**/src-tauri/**", "**/target/**"] },
  },
  build: {
    target: ["es2022", "chrome110", "safari16"],
  },
});
