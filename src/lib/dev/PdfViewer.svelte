<script lang="ts">
  // Spike A: A1 plan in PDF.js with pan / pinch-zoom.
  // Strategy: one capped "base" render (CSS-transformed during gestures) + a viewport-sized
  // high-res "tile" rendered only when the gesture settles above base resolution.
  import { onMount } from "svelte";
  // Legacy build: the emulator ships WebView 124, which lacks URL.parse (Chrome 126+) needed by the modern build. See D-002.
import * as pdfjs from "pdfjs-dist/legacy/build/pdf.mjs";
  import type { PDFDocumentProxy, PDFPageProxy, RenderTask } from "pdfjs-dist";
  import { attachGestures, zoomAt, type Transform } from "./gestures";

  pdfjs.GlobalWorkerOptions.workerSrc = new URL("pdfjs-dist/legacy/build/pdf.worker.min.mjs", import.meta.url).toString();

  /** Long side of the base canvas in device pixels. The experiment knob. */
  const MAX_BASE_PX = 4096;
  const MAX_ZOOM = 8; // relative to "fit"
  const DPR = Math.min(window.devicePixelRatio || 1, 2);
  const PLAN_URL = "/spike/plan-a1.pdf";

  let container: HTMLDivElement;
  let baseCanvas: HTMLCanvasElement;
  let tileCanvas: HTMLCanvasElement;
  let status = $state("loading…");
  let stats = $state<string[]>([]);
  let missing = $state(false);
  let showTile = $state(false);

  let page: PDFPageProxy | null = null;
  let baseScale = 1; // pdf units → base canvas px
  let fitScale = 1; // css scale at which the whole page fits the container
  let t = $state<Transform>({ x: 0, y: 0, scale: 1 });
  let tileTask: RenderTask | null = null;
  let tileTransform: Transform | null = $state(null);

  const log = (s: string) => { stats = [...stats.slice(-14), s]; console.log("[spikeA]", s); };
  const mb = (bytes: number) => (bytes / 1048576).toFixed(1) + " MB";
  const heap = () => {
    const m = (performance as any).memory;
    return m ? ` · JS heap ${mb(m.usedJSHeapSize)}` : "";
  };

  async function load() {
    const t0 = performance.now();
    const res = await fetch(PLAN_URL);
    const ct = res.headers.get("content-type") ?? "";
    if (!res.ok || ct.includes("text/html")) {
      missing = true;
      status = "no spike plan bundled (run scripts/copy-spike-plan.sh)";
      return;
    }
    const data = new Uint8Array(await res.arrayBuffer());
    if (data.length < 5 || String.fromCharCode(...data.subarray(0, 4)) !== "%PDF") {
      missing = true;
      status = "no spike plan bundled (file is not a PDF)";
      return;
    }
    log(`fetched ${mb(data.length)} in ${(performance.now() - t0).toFixed(0)} ms`);

    const doc: PDFDocumentProxy = await pdfjs.getDocument({
      data,
      cMapUrl: "/pdfjs/cmaps/",
      cMapPacked: true,
      standardFontDataUrl: "/pdfjs/standard_fonts/",
      wasmUrl: "/pdfjs/wasm/",
      iccUrl: "/pdfjs/iccs/",
    }).promise;
    page = await doc.getPage(1);
    const vp1 = page.getViewport({ scale: 1 });
    log(`page ${vp1.width.toFixed(0)}×${vp1.height.toFixed(0)} pt (${(vp1.width / 72 * 25.4).toFixed(0)}×${(vp1.height / 72 * 25.4).toFixed(0)} mm)`);

    // Base render, capped.
    baseScale = MAX_BASE_PX / Math.max(vp1.width, vp1.height);
    const vp = page.getViewport({ scale: baseScale });
    baseCanvas.width = Math.round(vp.width);
    baseCanvas.height = Math.round(vp.height);
    const t1 = performance.now();
    await page.render({ canvas: baseCanvas, viewport: vp }).promise;
    log(`base render ${baseCanvas.width}×${baseCanvas.height} (${mb(baseCanvas.width * baseCanvas.height * 4)} RGBA) in ${(performance.now() - t1).toFixed(0)} ms${heap()}`);

    // Fit to container (css px).
    const cw = container.clientWidth, ch = container.clientHeight;
    fitScale = Math.min(cw / baseCanvas.width, ch / baseCanvas.height);
    t = { scale: fitScale, x: (cw - baseCanvas.width * fitScale) / 2, y: (ch - baseCanvas.height * fitScale) / 2 };
    status = `ready · first render ${(performance.now() - t0).toFixed(0)} ms`;
  }

  /** Effective zoom relative to fit, for display. */
  const zoomLabel = () => (t.scale / fitScale).toFixed(2) + "×";

  /** Programmatic zoom to `z`× fit around the container centre (for emulator tests via adb taps). */
  function zoomTo(z: number) {
    const cw = container.clientWidth, ch = container.clientHeight;
    const target = Math.min(Math.max(fitScale * z, fitScale * 0.5), fitScale * MAX_ZOOM);
    t = zoomAt(t, target / t.scale, cw / 2, ch / 2, fitScale * 0.5, fitScale * MAX_ZOOM);
    showTile = false;
    void renderTile();
  }

  /** Render the visible viewport at full device resolution when zoomed beyond base pixels. */
  async function renderTile() {
    if (!page) return;
    tileTask?.cancel();
    tileTask = null;
    const cw = container.clientWidth, ch = container.clientHeight;
    // Device px per base px currently on screen.
    const devPerBase = t.scale * DPR;
    if (devPerBase <= 1.0) { showTile = false; tileTransform = null; return; }

    // Visible region in base-canvas coordinates.
    const bx = -t.x / t.scale, by = -t.y / t.scale, bw = cw / t.scale, bh = ch / t.scale;
    // Render at pdf scale such that 1 base px == devPerBase device px.
    const scale = baseScale * devPerBase;
    const vp = page.getViewport({ scale, offsetX: -bx * devPerBase, offsetY: -by * devPerBase });
    tileCanvas.width = Math.round(cw * DPR);
    tileCanvas.height = Math.round(ch * DPR);
    const snap = { ...t };
    const t0 = performance.now();
    const task = page.render({ canvas: tileCanvas, viewport: vp });
    tileTask = task;
    try {
      await task.promise;
      if (tileTask !== task) return;
      tileTransform = snap;
      showTile = true;
      log(`tile ${tileCanvas.width}×${tileCanvas.height} @ ${zoomLabel()} in ${(performance.now() - t0).toFixed(0)} ms${heap()}`);
      void bw; void bh;
    } catch (e: any) {
      if (e?.name !== "RenderingCancelledException") log(`tile error: ${e}`);
    }
  }

  onMount(() => {
    load().catch((e) => { status = `error: ${e}`; log(`error ${e?.message ?? e}`); });
    const detach = attachGestures(container, {
      get: () => t,
      set: (nt) => { t = nt; if (tileTransform && (nt.scale !== tileTransform.scale)) showTile = false; },
      settled: () => { void renderTile(); },
      minScale: () => fitScale * 0.5,
      maxScale: () => fitScale * MAX_ZOOM,
    });
    return () => { detach(); tileTask?.cancel(); };
  });

  // The tile is positioned where it was rendered; while panning at the same scale it follows the base.
  const tileStyle = $derived(() => {
    if (!tileTransform) return "display:none";
    const dx = t.x - tileTransform.x, dy = t.y - tileTransform.y;
    return `transform: translate(${dx}px, ${dy}px); width:${container?.clientWidth ?? 0}px; height:${container?.clientHeight ?? 0}px; display:${showTile ? "block" : "none"}`;
  });
</script>

<div class="viewer" bind:this={container}>
  <canvas bind:this={baseCanvas} class="base" style={`transform: translate(${t.x}px, ${t.y}px) scale(${t.scale}); transform-origin: 0 0;`}></canvas>
  <canvas bind:this={tileCanvas} class="tile" style={tileStyle()}></canvas>
  <div class="zoom">
    <button onclick={() => zoomTo(1)}>fit</button>
    <button onclick={() => zoomTo(2)}>2×</button>
    <button onclick={() => zoomTo(4)}>4×</button>
    <button onclick={() => zoomTo(8)}>8×</button>
    <button onclick={() => zoomTo(t.scale / fitScale * 1.5)}>+</button>
    <button onclick={() => zoomTo(t.scale / fitScale / 1.5)}>−</button>
  </div>
  <div class="hud">
    <div>{status} · zoom {zoomLabel()} · base cap {MAX_BASE_PX}px · DPR {DPR}</div>
    <div class="ua">{navigator.userAgent}</div>
    {#if missing}<div class="warn">Copy the A1 plan with <code>scripts/copy-spike-plan.sh</code>; client PDFs are never committed.</div>{/if}
    <pre>{stats.join("\n")}</pre>
  </div>
</div>

<style>
  .viewer { position: absolute; inset: 0; overflow: hidden; background: #888; touch-action: none; user-select: none; }
  canvas { position: absolute; left: 0; top: 0; image-rendering: auto; }
  .base { background: #fff; }
  .tile { pointer-events: none; }
  .hud {
    position: absolute; left: 8px; bottom: 8px; max-width: calc(100% - 16px); padding: 6px 8px;
    background: rgba(0,0,0,0.65); color: #fff; font-size: 11px; border-radius: 6px; pointer-events: none;
  }
  .hud pre { font-size: 10px; color: #ddd; max-height: 30vh; overflow: hidden; }
  .ua { color: #bbb; }
  .zoom { position: absolute; right: 8px; top: 8px; display: flex; flex-direction: column; gap: 4px; }
  .zoom button { min-width: 44px; min-height: 40px; padding: 0.3rem; font-size: 14px; }
  .warn { color: #ffd479; }
</style>
