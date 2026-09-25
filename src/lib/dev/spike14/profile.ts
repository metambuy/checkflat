// Step 1b: where does the time (and memory) go? Stages are separated by marks so spikes/pss.sh can
// attribute renderer PSS to each one.
import type { PDFPageProxy } from "pdfjs-dist";
import { dpr, fetchPlan, loadPdfjs, mb, ms, openDoc, releaseCanvas, type Build, type Log, type Mark, type PlanKey } from "./pdf";

async function renderTo(page: PDFPageProxy, scale: number, w: number, h: number, offsetX = 0, offsetY = 0) {
  const canvas = document.createElement("canvas");
  canvas.width = Math.round(w);
  canvas.height = Math.round(h);
  await page.render({ canvas, viewport: page.getViewport({ scale, offsetX, offsetY }) }).promise;
  return canvas;
}

export async function profileRun(build: Build, plan: PlanKey, log: Log, mark: Mark): Promise<void> {
  log(`RUN profile plan=${plan} build=${build} DPR=${window.devicePixelRatio} ${window.innerWidth}×${window.innerHeight} css`);
  let t = performance.now();
  const lib = await loadPdfjs(build);
  log(`pdfjs ${lib.version} module loaded in ${ms(t)}`);
  await mark("start");

  t = performance.now();
  const data = await fetchPlan(plan);
  log(`fetch ${mb(data.length)} in ${ms(t)}`);
  t = performance.now();
  const { doc, destroy } = await openDoc(lib, data, true);
  log(`getDocument ${ms(t)}`);
  t = performance.now();
  const page = await doc.getPage(1);
  const vp1 = page.getViewport({ scale: 1 });
  log(`getPage ${ms(t)} · ${vp1.width.toFixed(0)}×${vp1.height.toFixed(0)} pt`);
  await mark("doc");

  // Operator list: parsing + image decode in the worker + transfer of decoded images.
  t = performance.now();
  const ol = await page.getOperatorList();
  log(`getOperatorList ${ms(t)} · ${ol.fnArray.length} ops`);
  const names = new Map<number, string>(Object.entries(lib.OPS).map(([k, v]) => [v as number, k]));
  const hist = new Map<string, number>();
  ol.fnArray.forEach((fn) => hist.set(names.get(fn) ?? String(fn), (hist.get(names.get(fn) ?? String(fn)) ?? 0) + 1));
  log("ops: " + [...hist].sort((a, b) => b[1] - a[1]).slice(0, 10).map(([k, v]) => `${k} ${v}`).join(", "));
  ol.fnArray.forEach((fn, i) => {
    if (fn !== lib.OPS.paintImageXObject && fn !== lib.OPS.paintImageMaskXObject) return;
    const id = ol.argsArray[i][0];
    try {
      const store = typeof id === "string" && id.startsWith("g_") ? page.commonObjs : page.objs;
      const img = store.get(id) as any;
      const kind = img?.bitmap ? "ImageBitmap" : img?.data ? `data ${mb(img.data.length)}` : "?";
      log(`image ${names.get(fn)} ${id}: ${img?.width}×${img?.height} ${kind}`);
    } catch {
      log(`image ${names.get(fn)} ${id}: not resolved`);
    }
  });
  await mark("oplist");

  // Rasterise. The first render may re-request the operator list (different cache key than
  // getOperatorList); the second one reuses the display-intent list, so it is pure rasterising.
  const long = Math.max(vp1.width, vp1.height);
  const sizes = [1024, 3072];
  const canvases: HTMLCanvasElement[] = [];
  for (const px of sizes) {
    const s = px / long;
    t = performance.now();
    canvases.push(await renderTo(page, s, vp1.width * s, vp1.height * s));
    log(`render ${px}px ${ms(t)}`);
    await mark(`r${px}`);
  }
  // 8× fit tile at the page centre, the size of this screen (DPR capped as in the viewer).
  const cw = window.innerWidth, ch = window.innerHeight;
  const fit = Math.min(cw / vp1.width, ch / vp1.height);
  const s8 = fit * 8 * dpr();
  const tw = cw * dpr(), th = ch * dpr();
  t = performance.now();
  canvases.push(await renderTo(page, s8, tw, th, -(vp1.width * s8 - tw) / 2, -(vp1.height * s8 - th) / 2));
  log(`render 8× tile ${Math.round(tw)}×${Math.round(th)} ${ms(t)}`);
  if ((page as any).stats) log(`page.stats: ${String((page as any).stats).replace(/\n/g, " | ")}`);
  await mark("tile8");

  canvases.forEach(releaseCanvas);
  await mark("canvases-released");
  page.cleanup();
  await mark("page-cleanup");
  await destroy();
  await mark("doc-destroyed");
  log("END profile");
}
