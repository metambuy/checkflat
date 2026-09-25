// Step 1c design 2: render the page once into a tile pyramid on disk (WebP, JPEG fallback).
import { invoke } from "@tauri-apps/api/core";
import { fetchPlan, heap, loadPdfjs, mb, ms, openDoc, releaseCanvas, type Build, type Log, type PlanKey } from "./pdf";

export interface Level { L: number; W: number; H: number; cols: number; rows: number }
export interface Manifest { tile: number; ext: string; widthPt: number; heightPt: number; levels: Level[] }

export interface PyramidOptions {
  levels?: number[]; // long side in px, smallest first
  tile?: number;
  block?: number; // render block, multiple of tile
  type?: "image/webp" | "image/jpeg";
  quality?: number;
}

async function writeFile(path: string, bytes: Uint8Array) {
  await invoke("spike_write_tile", bytes, { headers: { "x-tile-path": path } });
}

export async function generatePyramid(build: Build, plan: PlanKey, key: string, log: Log, opts: PyramidOptions = {}): Promise<void> {
  const { levels = [1024, 2048, 4096, 8192], tile = 512, block = 2048, type = "image/webp", quality = 0.8 } = opts;
  log(`RUN generate plan=${plan} build=${build} key=${key} levels=${levels.join("/")} tile=${tile} block=${block} ${type} q${quality}`);
  const tAll = performance.now();
  await invoke("spike_tiles_clear", { key });
  const lib = await loadPdfjs(build);
  const { doc, destroy } = await openDoc(lib, await fetchPlan(plan));
  const page = await doc.getPage(1);
  const vp1 = page.getViewport({ scale: 1 });
  const long = Math.max(vp1.width, vp1.height);
  let ext = type === "image/webp" ? "webp" : "jpg";
  const manifest: Manifest = { tile, ext, widthPt: vp1.width, heightPt: vp1.height, levels: [] };
  const tc = document.createElement("canvas");
  const tctx = tc.getContext("2d")!;
  let totalBytes = 0, renderMs = 0, encodeMs = 0, writeMs = 0;

  for (const L of levels) {
    const tL = performance.now();
    const s = L / long;
    const W = Math.ceil(vp1.width * s), H = Math.ceil(vp1.height * s);
    const cols = Math.ceil(W / tile), rows = Math.ceil(H / tile);
    let levelBytes = 0;
    for (let by = 0; by < H; by += block) {
      for (let bx = 0; bx < W; bx += block) {
        const canvas = document.createElement("canvas");
        canvas.width = Math.min(block, W - bx);
        canvas.height = Math.min(block, H - by);
        let t = performance.now();
        await page.render({ canvas, viewport: page.getViewport({ scale: s, offsetX: -bx, offsetY: -by }) }).promise;
        renderMs += performance.now() - t;
        for (let ty = by; ty < by + canvas.height; ty += tile) {
          for (let tx = bx; tx < bx + canvas.width; tx += tile) {
            const tw = Math.min(tile, W - tx), th = Math.min(tile, H - ty);
            tc.width = tw;
            tc.height = th;
            tctx.drawImage(canvas, tx - bx, ty - by, tw, th, 0, 0, tw, th);
            t = performance.now();
            const blob = await new Promise<Blob | null>((r) => tc.toBlob(r, type, quality));
            if (!blob) throw new Error("toBlob returned null");
            if (blob.type !== type) {
              // WebView without WebP encoding falls back to PNG silently; record what we got.
              ext = blob.type === "image/jpeg" ? "jpg" : blob.type === "image/png" ? "png" : ext;
              manifest.ext = ext;
            }
            const bytes = new Uint8Array(await blob.arrayBuffer());
            encodeMs += performance.now() - t;
            t = performance.now();
            await writeFile(`${key}/${L}/${tx / tile}_${ty / tile}.${ext}`, bytes);
            writeMs += performance.now() - t;
            levelBytes += bytes.length;
          }
        }
        releaseCanvas(canvas);
      }
    }
    totalBytes += levelBytes;
    manifest.levels.push({ L, W, H, cols, rows });
    // Written after each finished level: a level is usable once it is in the manifest.
    await writeFile(`${key}/meta/manifest.json`, new TextEncoder().encode(JSON.stringify(manifest)));
    log(`level ${L}: ${W}×${H}, ${cols * rows} tiles, ${mb(levelBytes)} in ${ms(tL)}${heap()}`);
  }
  releaseCanvas(tc);
  page.cleanup();
  await destroy();
  log(`END generate ${mb(totalBytes)} total in ${ms(tAll)} (render ${renderMs.toFixed(0)} ms, encode ${encodeMs.toFixed(0)} ms, write ${writeMs.toFixed(0)} ms), ext ${ext}`);
}
