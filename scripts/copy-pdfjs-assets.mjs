// Copies the PDF.js runtime assets that are loaded at runtime (not bundled by Vite)
// from node_modules into public/pdfjs/. Runs on postinstall / predev / prebuild.
import { cpSync, existsSync, mkdirSync, rmSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const src = join(root, "node_modules", "pdfjs-dist");
const dst = join(root, "public", "pdfjs");

if (!existsSync(src)) {
  console.error("pdfjs-dist not installed; run pnpm install first");
  process.exit(1);
}
rmSync(dst, { recursive: true, force: true });
mkdirSync(dst, { recursive: true });
for (const dir of ["cmaps", "standard_fonts", "wasm", "iccs"]) {
  const from = join(src, dir);
  if (existsSync(from)) {
    cpSync(from, join(dst, dir), { recursive: true });
    console.log(`copied pdfjs-dist/${dir} -> public/pdfjs/${dir}`);
  } else {
    console.warn(`pdfjs-dist/${dir} not found, skipped`);
  }
}
