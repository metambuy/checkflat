// Import-time tile pyramid (D-014): PDF.js renders the plan once into 512 px WebP tiles at four
// levels; the viewer then shows images only. Resumable per level: the manifest is rewritten after
// each finished level, and a restart continues with the first missing one.
import { convertFileSrc } from "@tauri-apps/api/core";
import { api, type Plan, type TileInfo, type TileManifest } from "../api";
import { TILES } from "./config";

/** PDF.js is loaded only when tiles must be generated; viewing never loads it (D-014). Legacy
 * build: no measurable gain from the modern one, and it also runs on WebView < 126. */
async function loadPdfjs() {
  const pdfjs = await import("pdfjs-dist/legacy/build/pdf.mjs");
  pdfjs.GlobalWorkerOptions.workerSrc = new URL("pdfjs-dist/legacy/build/pdf.worker.min.mjs", import.meta.url).toString();
  return pdfjs;
}

export interface Progress {
  done: number;
  total: number;
}

/** True when the manifest was produced with the current settings (possibly unfinished). */
function compatible(m: TileManifest | null): m is TileManifest {
  return !!m && m.version === TILES.version && m.tile === TILES.tile && m.levels.every((l, i) => l.size === TILES.levels[i]);
}

export function isComplete(m: TileManifest | null): boolean {
  return compatible(m) && m.levels.length === TILES.levels.length;
}

function blobToBase64(blob: Blob): Promise<string> {
  return new Promise((resolve, reject) => {
    const r = new FileReader();
    r.onload = () => {
      const s = String(r.result);
      resolve(s.slice(s.indexOf(",") + 1));
    };
    r.onerror = () => reject(r.error);
    r.readAsDataURL(blob);
  });
}

export interface GenerateOptions {
  onProgress?(p: Progress): void;
  /** Called after each finished level with the manifest now on disk. */
  onLevel?(m: TileManifest): void;
  /** Checked between tiles; generation stops (resumable) when it returns true. */
  cancelled?(): boolean;
}

/** Brings the plan's tile cache up to date and returns the final manifest (or the partial one
 * if cancelled). The PDF.js document is always destroyed before returning. */
export async function ensureTiles(plan: Plan, info: TileInfo, opts: GenerateOptions = {}): Promise<TileManifest> {
  let manifest: TileManifest;
  if (compatible(info.manifest)) {
    manifest = info.manifest;
    if (manifest.levels.length === TILES.levels.length) return manifest;
  } else {
    if (info.manifest) await api.clearPlanTiles(plan.id);
    manifest = { version: TILES.version, tile: TILES.tile, widthPt: plan.widthPt, heightPt: plan.heightPt, levels: [] };
  }

  const pdfjs = await loadPdfjs();
  const res = await fetch(convertFileSrc(info.pdf));
  if (!res.ok) throw new Error(`plan file unreadable (${res.status})`);
  const task = pdfjs.getDocument({
    data: new Uint8Array(await res.arrayBuffer()),
    cMapUrl: "/pdfjs/cmaps/",
    cMapPacked: true,
    standardFontDataUrl: "/pdfjs/standard_fonts/",
    wasmUrl: "/pdfjs/wasm/",
    iccUrl: "/pdfjs/iccs/",
  });
  try {
    const page = await (await task.promise).getPage(1);
    const vp1 = page.getViewport({ scale: 1 });
    // D-011 cross-check: PDF.js vs the size lopdf stored at import.
    if (Math.abs(vp1.width - plan.widthPt) > 1 || Math.abs(vp1.height - plan.heightPt) > 1) {
      console.warn(`[tiles] page size ${vp1.width}×${vp1.height} pt differs from stored ${plan.widthPt}×${plan.heightPt}`);
    }
    manifest.widthPt = vp1.width;
    manifest.heightPt = vp1.height;
    const long = Math.max(vp1.width, vp1.height);
    const todo = TILES.levels.slice(manifest.levels.length);
    const dims = todo.map((size) => {
      const s = size / long;
      const width = Math.ceil(vp1.width * s), height = Math.ceil(vp1.height * s);
      return { size, s, width, height, cols: Math.ceil(width / TILES.tile), rows: Math.ceil(height / TILES.tile) };
    });
    const progress = { done: 0, total: dims.reduce((n, d) => n + d.cols * d.rows, 0) };
    opts.onProgress?.(progress);

    const tc = document.createElement("canvas");
    const tctx = tc.getContext("2d")!;
    for (const d of dims) {
      for (let by = 0; by < d.height; by += TILES.block) {
        for (let bx = 0; bx < d.width; bx += TILES.block) {
          const block = document.createElement("canvas");
          block.width = Math.min(TILES.block, d.width - bx);
          block.height = Math.min(TILES.block, d.height - by);
          try {
            await page.render({ canvas: block, viewport: page.getViewport({ scale: d.s, offsetX: -bx, offsetY: -by }) }).promise;
            for (let ty = by; ty < by + block.height; ty += TILES.tile) {
              for (let tx = bx; tx < bx + block.width; tx += TILES.tile) {
                if (opts.cancelled?.()) return manifest;
                tc.width = Math.min(TILES.tile, d.width - tx);
                tc.height = Math.min(TILES.tile, d.height - ty);
                tctx.drawImage(block, tx - bx, ty - by, tc.width, tc.height, 0, 0, tc.width, tc.height);
                const blob = await new Promise<Blob | null>((r) => tc.toBlob(r, "image/webp", TILES.quality));
                if (!blob || blob.type !== "image/webp") throw new Error("this WebView cannot encode WebP");
                await api.writePlanTile(plan.id, d.size, tx / TILES.tile, ty / TILES.tile, await blobToBase64(blob));
                progress.done++;
                opts.onProgress?.(progress);
              }
            }
          } finally {
            block.width = block.height = 0; // release now, not at GC
          }
        }
      }
      manifest.levels.push({ size: d.size, width: d.width, height: d.height, cols: d.cols, rows: d.rows });
      await api.writePlanTileManifest(plan.id, manifest);
      opts.onLevel?.({ ...manifest, levels: [...manifest.levels] });
    }
    tc.width = tc.height = 0;
    return manifest;
  } finally {
    await task.destroy(); // frees the ~780 MB the A1 plan holds in the renderer (D-014)
  }
}
