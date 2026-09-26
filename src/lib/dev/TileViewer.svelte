<script lang="ts">
  // D-014 design 2: display a pre-rendered tile pyramid as <img> tiles; no PDF.js while viewing.
  // World px = px of the top level. The lowest level is always underneath (never blank); the level
  // matching the zoom is chosen when a gesture settles, and only its visible tiles are in the DOM.
  import { onMount, tick } from "svelte";
  import { convertFileSrc, invoke } from "@tauri-apps/api/core";
  import { attachGestures, type Transform } from "../viewer/gestures";
  import { runBench } from "./spike14/bench";
  import { dpr, mb, ms, type Log, type Mark } from "./spike14/pdf";
  import type { Level, Manifest } from "./spike14/tiles";

  let { tilesKey, bench = false, focus = { x: 0.5, y: 0.5 }, log, mark }: { tilesKey: string; bench?: boolean; focus?: { x: number; y: number }; log: Log; mark: Mark } = $props();

  const MAX_ZOOM = 8;
  const DPR = dpr();
  let container: HTMLDivElement;
  let world: HTMLDivElement;
  let man = $state<Manifest | null>(null);
  let dir = "";
  let cw = $state(0), ch = $state(0);
  let t = $state<Transform>({ x: 0, y: 0, scale: 1 });
  let fitScale = $state(1);
  let levelIdx = $state(0);
  let status = $state("");

  const top = (): Level => man!.levels[man!.levels.length - 1];

  interface Tile { key: string; src: string; x: number; y: number; w: number; h: number }
  const tiles = $derived.by((): Tile[] => {
    if (!man || !cw) return [];
    const T = top(), size = man.tile;
    const out: Tile[] = [];
    const push = (lv: Level, all: boolean) => {
      const f = T.L / lv.L; // world px per level px
      let i0 = 0, i1 = lv.cols - 1, j0 = 0, j1 = lv.rows - 1;
      if (!all) {
        const x0 = -t.x / t.scale, y0 = -t.y / t.scale;
        const x1 = x0 + cw / t.scale, y1 = y0 + ch / t.scale;
        i0 = Math.max(0, Math.floor(x0 / (size * f)));
        i1 = Math.min(lv.cols - 1, Math.floor(x1 / (size * f)));
        j0 = Math.max(0, Math.floor(y0 / (size * f)));
        j1 = Math.min(lv.rows - 1, Math.floor(y1 / (size * f)));
      }
      for (let j = j0; j <= j1; j++)
        for (let i = i0; i <= i1; i++) {
          const w = Math.min(size, lv.W - i * size), h = Math.min(size, lv.H - j * size);
          out.push({
            key: `${lv.L}/${i}_${j}`,
            src: convertFileSrc(`${dir}/${lv.L}/${i}_${j}.${man!.ext}`),
            x: i * size * f, y: j * size * f,
            w: (w + 0.5) * f, h: (h + 0.5) * f, // half a source px of overlap hides seams
          });
        }
    };
    push(man.levels[0], true);
    if (levelIdx > 0) push(man.levels[levelIdx], false);
    return out;
  });

  /** Smallest level with ≥ 1 source px per device px, else the top level. */
  function chooseLevel() {
    if (!man) return;
    const need = t.scale * DPR; // device px per world px
    const T = top();
    const idx = man.levels.findIndex((lv) => lv.L / T.L >= need);
    levelIdx = idx === -1 ? man.levels.length - 1 : idx;
  }

  /** Resolves when every tile currently in the DOM is loaded and decoded. */
  async function settle(): Promise<void> {
    chooseLevel();
    await tick();
    const imgs = [...world.querySelectorAll("img")];
    await Promise.all(imgs.map((i) => i.decode().catch(() => {})));
  }

  /** Zoom to z× fit with the focus point (0–1 page coordinates) at the centre of the view. */
  function zoomTo(z: number): Promise<void> {
    const T = top();
    const scale = Math.min(Math.max(fitScale * z, fitScale * 0.5), fitScale * MAX_ZOOM);
    t = { scale, x: cw / 2 - focus.x * T.W * scale, y: ch / 2 - focus.y * T.H * scale };
    return settle();
  }

  async function load() {
    const t0 = performance.now();
    const info = await invoke<{ dir: string; files: number; bytes: number; manifest: string | null }>("spike_tiles_info", { key: tilesKey });
    if (!info.manifest) {
      status = `no pyramid for ${tilesKey}: generate it first`;
      log(status);
      return false;
    }
    dir = info.dir;
    man = JSON.parse(info.manifest) as Manifest;
    log(`RUN design2 key=${tilesKey} ${info.files} files ${mb(info.bytes)} levels ${man.levels.map((l) => l.L).join("/")} DPR=${window.devicePixelRatio}→${DPR}`);
    cw = container.clientWidth;
    ch = container.clientHeight;
    const T = top();
    fitScale = Math.min(cw / T.W, ch / T.H);
    t = { scale: fitScale, x: (cw - T.W * fitScale) / 2, y: (ch - T.H * fitScale) / 2 };
    await settle();
    log(`FIRST VIEW (level ${man.levels[levelIdx].L}) at ${ms(t0)}`);
    return true;
  }

  onMount(() => {
    load()
      .then((ok) => {
        if (ok && bench) return runBench({ zoomTo, get: () => t, set: (nt) => (t = nt), settle, width: () => cw }, log, mark);
      })
      .catch((e) => log(`error: ${e?.message ?? e}`));
    const ro = new ResizeObserver(() => { cw = container.clientWidth; ch = container.clientHeight; });
    ro.observe(container);
    const detach = attachGestures(container, {
      get: () => t,
      set: (nt) => (t = nt),
      settled: () => void settle(),
      minScale: () => fitScale * 0.5,
      maxScale: () => fitScale * MAX_ZOOM,
    });
    return () => { detach(); ro.disconnect(); };
  });
</script>

<div class="viewer" bind:this={container}>
  <div class="world" bind:this={world} style={`transform: translate(${t.x}px, ${t.y}px) scale(${t.scale})`}>
    {#each tiles as tl (tl.key)}
      <img src={tl.src} alt="" decoding="async" draggable="false" style={`left:${tl.x}px;top:${tl.y}px;width:${tl.w}px;height:${tl.h}px`} />
    {/each}
  </div>
  <div class="zoom">
    <button onclick={() => zoomTo(1)}>fit</button>
    <button onclick={() => zoomTo(4)}>4×</button>
    <button onclick={() => zoomTo(8)}>8×</button>
  </div>
  <div class="z">{(t.scale / fitScale).toFixed(2)}× · L{man?.levels[levelIdx]?.L ?? "–"} · {tiles.length} img {status}</div>
</div>

<style>
  .viewer { position: absolute; inset: 0; overflow: hidden; background: #888; touch-action: none; user-select: none; }
  .world { position: absolute; left: 0; top: 0; transform-origin: 0 0; }
  .world img { position: absolute; max-width: none; pointer-events: none; }
  .zoom { position: absolute; right: 8px; top: 8px; display: flex; flex-direction: column; gap: 4px; }
  .zoom button { min-width: 44px; min-height: 40px; padding: 0.3rem; font-size: 14px; }
  .z { position: absolute; right: 8px; bottom: 8px; color: #fff; background: rgba(0,0,0,.6); padding: 2px 6px; border-radius: 4px; font-size: 12px; }
</style>
