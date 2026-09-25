<script lang="ts">
  // Spike A / D-014 design 1: a capped base render (optionally after a quick low-res pass), pan/zoom
  // by CSS transform, and a viewport tile rendered when the gesture settles above base resolution.
  // Tiles are double-buffered (rendered off-DOM, then swapped) and replaced canvases are released.
  import { onMount } from "svelte";
  import type { PDFPageProxy, RenderTask } from "pdfjs-dist";
  import { attachGestures, zoomAt, type Transform } from "./gestures";
  import { runBench } from "./spike14/bench";
  import { dpr, fetchPlan, heap, loadPdfjs, mb, ms, openDoc, releaseCanvas, type Build, type Log, type Mark, type OpenDoc, type PlanKey } from "./spike14/pdf";

  let {
    build = "legacy",
    plan = "a1",
    maxBase = 3072,
    quickPx = 1024,
    bench = false,
    log,
    mark,
  }: { build?: Build; plan?: PlanKey; maxBase?: number; quickPx?: number; bench?: boolean; log: Log; mark: Mark } = $props();

  const MAX_ZOOM = 8; // relative to fit
  const DPR = dpr();

  let container: HTMLDivElement;
  let world: HTMLDivElement;
  let tileHost: HTMLDivElement;
  let t = $state<Transform>({ x: 0, y: 0, scale: 1 });
  let fitScale = $state(1);
  let tileAt = $state<Transform | null>(null);
  let opened: OpenDoc | null = null;
  let page: PDFPageProxy | null = null;
  let baseScale = 1; // pdf units → base px (= world px)
  let tile: HTMLCanvasElement | null = null;
  let tileTask: RenderTask | null = null;
  let destroyed = false;

  function place(c: HTMLCanvasElement, w: number, h: number) {
    c.style.width = `${w}px`;
    c.style.height = `${h}px`;
    world.appendChild(c);
  }

  async function load() {
    const t0 = performance.now();
    log(`RUN design1 plan=${plan} build=${build} base=${maxBase} quick=${quickPx || "off"} DPR=${window.devicePixelRatio}→${DPR}`);
    const lib = await loadPdfjs(build);
    const data = await fetchPlan(plan);
    opened = await openDoc(lib, data);
    if (destroyed) return void opened.destroy();
    page = await opened.doc.getPage(1);
    const vp1 = page.getViewport({ scale: 1 });
    baseScale = maxBase / Math.max(vp1.width, vp1.height);
    const bw = Math.round(vp1.width * baseScale), bh = Math.round(vp1.height * baseScale);
    world.style.width = `${bw}px`;
    world.style.height = `${bh}px`;
    const cw = container.clientWidth, ch = container.clientHeight;
    fitScale = Math.min(cw / bw, ch / bh);
    t = { scale: fitScale, x: (cw - bw * fitScale) / 2, y: (ch - bh * fitScale) / 2 };
    log(`doc open ${ms(t0)}`);

    let quick: HTMLCanvasElement | null = null;
    if (quickPx > 0) {
      const s = quickPx / Math.max(vp1.width, vp1.height);
      quick = document.createElement("canvas");
      quick.width = Math.round(vp1.width * s);
      quick.height = Math.round(vp1.height * s);
      const t1 = performance.now();
      await page.render({ canvas: quick, viewport: page.getViewport({ scale: s }) }).promise;
      place(quick, bw, bh);
      log(`FIRST VIEW (quick ${quick.width}×${quick.height} in ${ms(t1)}) at ${ms(t0)}${heap()}`);
    }
    const base = document.createElement("canvas");
    base.width = bw;
    base.height = bh;
    const t2 = performance.now();
    await page.render({ canvas: base, viewport: page.getViewport({ scale: baseScale }) }).promise;
    if (destroyed) return releaseCanvas(base);
    place(base, bw, bh);
    releaseCanvas(quick);
    log(`${quickPx > 0 ? "base" : "FIRST VIEW base"} ${bw}×${bh} (${mb(bw * bh * 4)}) in ${ms(t2)} at ${ms(t0)}${heap()}`);
  }

  function dropTile() {
    tileTask?.cancel();
    tileTask = null;
    releaseCanvas(tile);
    tile = null;
    tileAt = null;
  }

  /** Viewport tile at device resolution when zoomed past base pixels; rendered off-DOM, then swapped. */
  async function renderTile(): Promise<void> {
    if (!page) return;
    tileTask?.cancel();
    const cw = container.clientWidth, ch = container.clientHeight;
    const devPerBase = t.scale * DPR;
    if (devPerBase <= 1) return dropTile();
    const bx = -t.x / t.scale, by = -t.y / t.scale;
    const next = document.createElement("canvas");
    next.width = Math.round(cw * DPR);
    next.height = Math.round(ch * DPR);
    next.style.width = `${cw}px`;
    next.style.height = `${ch}px`;
    const snap = { ...t };
    const vp = page.getViewport({ scale: baseScale * devPerBase, offsetX: -bx * devPerBase, offsetY: -by * devPerBase });
    const t0 = performance.now();
    const task = page.render({ canvas: next, viewport: vp });
    tileTask = task;
    try {
      await task.promise;
      if (tileTask !== task || destroyed) return releaseCanvas(next);
      releaseCanvas(tile);
      tile = next;
      tileHost.appendChild(next);
      tileAt = snap;
      log(`tile ${next.width}×${next.height} @ ${(snap.scale / fitScale).toFixed(2)}× in ${ms(t0)}${heap()}`);
    } catch (e: any) {
      releaseCanvas(next);
      if (e?.name !== "RenderingCancelledException") log(`tile error: ${e}`);
    }
  }

  function zoomTo(z: number): Promise<void> {
    const cw = container.clientWidth, ch = container.clientHeight;
    t = zoomAt(t, (fitScale * z) / t.scale, cw / 2, ch / 2, fitScale * 0.5, fitScale * MAX_ZOOM);
    return renderTile();
  }

  onMount(() => {
    load()
      .then(() => {
        if (bench && !destroyed)
          return runBench({ zoomTo, get: () => t, set: (nt) => (t = nt), settle: renderTile, width: () => container.clientWidth }, log, mark);
      })
      .catch((e) => log(`error: ${e?.message ?? e}`));
    const detach = attachGestures(container, {
      get: () => t,
      set: (nt) => (t = nt),
      settled: () => void renderTile(),
      minScale: () => fitScale * 0.5,
      maxScale: () => fitScale * MAX_ZOOM,
    });
    return () => {
      destroyed = true;
      detach();
      dropTile();
      world.querySelectorAll("canvas").forEach((c) => releaseCanvas(c));
      void opened?.destroy();
    };
  });

  // The tile stays where it was rendered while the scale is unchanged (pan); hidden during zoom.
  const tileStyle = $derived(
    tileAt && tileAt.scale === t.scale ? `transform: translate(${t.x - tileAt.x}px, ${t.y - tileAt.y}px)` : "display: none",
  );
</script>

<div class="viewer" bind:this={container}>
  <div class="world" bind:this={world} style={`transform: translate(${t.x}px, ${t.y}px) scale(${t.scale})`}></div>
  <div class="tile" bind:this={tileHost} style={tileStyle}></div>
  <div class="zoom">
    <button onclick={() => zoomTo(1)}>fit</button>
    <button onclick={() => zoomTo(4)}>4×</button>
    <button onclick={() => zoomTo(8)}>8×</button>
  </div>
  <div class="z">{(t.scale / fitScale).toFixed(2)}×</div>
</div>

<style>
  .viewer { position: absolute; inset: 0; overflow: hidden; background: #888; touch-action: none; user-select: none; }
  .world { position: absolute; left: 0; top: 0; transform-origin: 0 0; background: #fff; }
  .world :global(canvas) { position: absolute; left: 0; top: 0; }
  .tile { position: absolute; left: 0; top: 0; pointer-events: none; }
  .tile :global(canvas) { position: absolute; left: 0; top: 0; }
  .zoom { position: absolute; right: 8px; top: 8px; display: flex; flex-direction: column; gap: 4px; }
  .zoom button { min-width: 44px; min-height: 40px; padding: 0.3rem; font-size: 14px; }
  .z { position: absolute; right: 8px; bottom: 8px; color: #fff; background: rgba(0,0,0,.6); padding: 2px 6px; border-radius: 4px; font-size: 12px; }
</style>
