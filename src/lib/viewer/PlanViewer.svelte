<script lang="ts">
  // Plan viewer (D-014): pre-rendered tile pyramid as <img> tiles, pan/zoom by CSS transform, pins
  // as a constant-size overlay. World px = px of the top tile level. The lowest level is always
  // underneath (never blank); the level matching the zoom is chosen when a gesture settles, and
  // only its visible tiles are in the DOM.
  import { onMount, tick } from "svelte";
  import { convertFileSrc } from "@tauri-apps/api/core";
  import type { Observation, TileManifest } from "../api";
  import { attachGestures } from "./gestures";
  import { TOP_LEVEL } from "./config";
  import {
    centreOn, chooseLevel, clamp01, clampPan, fitScale, fitTransform, insidePlan, maxScale, MIN_ZOOM,
    refit, toNorm, toScreen, type Point, type Size, type Transform,
  } from "./coords";

  let {
    manifest,
    dir,
    pins,
    draft,
    selectedId = null,
    ontap,
    onpintap,
    onpinmove,
    ondraftmove,
  }: {
    manifest: TileManifest;
    dir: string;
    pins: Observation[];
    draft: Point | null;
    selectedId?: string | null;
    ontap: (p: Point) => void;
    onpintap: (id: string) => void;
    onpinmove: (id: string, p: Point) => void;
    ondraftmove: (p: Point) => void;
  } = $props();

  const DPR = window.devicePixelRatio || 1;
  const DRAG_SLOP = 6;

  let container: HTMLDivElement;
  let view = $state<Size>({ w: 0, h: 0 });
  let t = $state<Transform>({ x: 0, y: 0, scale: 1 });
  let levelIdx = $state(0);
  // Pin being dragged (id "draft" for the draft pin) and its live position.
  let drag = $state<{ id: string; p: Point } | null>(null);

  // World size: the top level the generator produces, so the transform stays valid while higher
  // levels are still being generated.
  const worldSize = $derived.by((): Size => {
    const long = Math.max(manifest.widthPt, manifest.heightPt);
    return { w: Math.ceil((manifest.widthPt * TOP_LEVEL) / long), h: Math.ceil((manifest.heightPt * TOP_LEVEL) / long) };
  });
  const fit = $derived(view.w ? fitScale(view, worldSize) : 1);
  const maxS = $derived(maxScale(fit, DPR));
  const zoom = $derived(t.scale / fit);

  interface Tile { key: string; src: string; x: number; y: number; w: number; h: number }
  const tiles = $derived.by((): Tile[] => {
    if (!view.w || manifest.levels.length === 0) return [];
    const size = manifest.tile;
    const out: Tile[] = [];
    const push = (idx: number, all: boolean) => {
      const lv = manifest.levels[idx];
      const f = worldSize.w / lv.width; // world px per level px
      let i0 = 0, i1 = lv.cols - 1, j0 = 0, j1 = lv.rows - 1;
      if (!all) {
        const a = toNorm(t, worldSize, 0, 0), b = toNorm(t, worldSize, view.w, view.h);
        i0 = Math.max(0, Math.floor((a.x * lv.width) / size));
        i1 = Math.min(lv.cols - 1, Math.floor((b.x * lv.width) / size));
        j0 = Math.max(0, Math.floor((a.y * lv.height) / size));
        j1 = Math.min(lv.rows - 1, Math.floor((b.y * lv.height) / size));
      }
      for (let j = j0; j <= j1; j++)
        for (let i = i0; i <= i1; i++) {
          const w = Math.min(size, lv.width - i * size), h = Math.min(size, lv.height - j * size);
          out.push({
            key: `${lv.size}/${i}_${j}`,
            src: convertFileSrc(`${dir}/${lv.size}/${i}_${j}.webp`),
            x: i * size * f,
            y: j * size * f,
            w: (w + 0.5) * f, // half a source px of overlap hides seams
            h: (h + 0.5) * f,
          });
        }
    };
    push(0, true);
    const idx = Math.min(levelIdx, manifest.levels.length - 1);
    if (idx > 0) push(idx, false);
    return out;
  });

  function settle() {
    if (manifest.levels.length) levelIdx = chooseLevel(manifest.levels.map((l) => l.size), TOP_LEVEL, t.scale, DPR);
  }

  function setT(nt: Transform) {
    t = clampPan(nt, view, worldSize);
  }

  /** Zoom to `z`× fit around the view centre (or a plan point). */
  export function zoomTo(z: number, at?: Point) {
    const scale = Math.min(Math.max(fit * z, fit * MIN_ZOOM), maxS);
    const c = at ?? toNorm(t, worldSize, view.w / 2, view.h / 2);
    setT(centreOn(view, worldSize, clamp01(c.x), clamp01(c.y), scale));
    settle();
  }

  /** Multiply the zoom around the view centre (keyboard +/−). */
  export function zoomBy(f: number) {
    zoomTo(zoom * f);
  }

  /** Centre a plan point, zooming in to at least 3× fit. */
  export function reveal(p: Point) {
    zoomTo(Math.max(zoom, 3), p);
  }

  export function fitView() {
    setT(fitTransform(view, worldSize));
    settle();
  }

  const screenOf = (p: Point) => toScreen(t, worldSize, p.x, p.y);

  /** Pointer handling on a pin: a short press is a tap (select), movement beyond the slop drags. */
  function pinPointer(e: PointerEvent, id: string, start: Point) {
    if (e.pointerType === "mouse" && e.button !== 0) return;
    e.stopPropagation(); // the plan's gestures must not see this pointer
    const el = e.currentTarget as HTMLElement;
    el.setPointerCapture(e.pointerId);
    const r = container.getBoundingClientRect();
    const anchor = screenOf(start);
    const off = { x: e.clientX - r.left - anchor.x, y: e.clientY - r.top - anchor.y }; // no jump under the finger
    const x0 = e.clientX, y0 = e.clientY;
    let dragging = false;
    const move = (m: PointerEvent) => {
      if (m.pointerId !== e.pointerId) return;
      if (!dragging && Math.hypot(m.clientX - x0, m.clientY - y0) < DRAG_SLOP) return;
      dragging = true;
      const n = toNorm(t, worldSize, m.clientX - r.left - off.x, m.clientY - r.top - off.y);
      drag = { id, p: { x: clamp01(n.x), y: clamp01(n.y) } };
    };
    const up = (u: PointerEvent) => {
      if (u.pointerId !== e.pointerId) return;
      el.removeEventListener("pointermove", move);
      el.removeEventListener("pointerup", up);
      el.removeEventListener("pointercancel", up);
      const d = drag;
      drag = null;
      if (dragging && d && u.type === "pointerup") {
        if (id === "draft") ondraftmove(d.p);
        else onpinmove(id, d.p);
      } else if (!dragging && u.type === "pointerup" && id !== "draft") onpintap(id);
    };
    el.addEventListener("pointermove", move);
    el.addEventListener("pointerup", up);
    el.addEventListener("pointercancel", up);
  }

  const posOf = (id: string, p: Point) => (drag?.id === id ? drag.p : p);

  onMount(() => {
    view = { w: container.clientWidth, h: container.clientHeight };
    t = fitTransform(view, worldSize);
    settle();
    const ro = new ResizeObserver(() => {
      const nv = { w: container.clientWidth, h: container.clientHeight };
      if (!nv.w || !nv.h || (nv.w === view.w && nv.h === view.h)) return;
      const old = view;
      view = nv;
      setT(refit(t, old, nv, worldSize));
      settle();
    });
    ro.observe(container);
    const detach = attachGestures(container, {
      get: () => t,
      set: setT,
      settled: settle,
      minScale: () => fit * MIN_ZOOM,
      maxScale: () => maxS,
      tap: (x, y) => {
        const p = toNorm(t, worldSize, x, y);
        if (insidePlan(p)) ontap(p);
      },
    });
    return () => { detach(); ro.disconnect(); };
  });

  // New levels finished while viewing: re-evaluate once they are in the manifest.
  $effect(() => {
    void manifest.levels.length;
    void tick().then(settle);
  });
</script>

<div class="viewer" bind:this={container}>
  <div class="world" style={`width:${worldSize.w}px;height:${worldSize.h}px;transform:translate(${t.x}px,${t.y}px) scale(${t.scale})`}>
    {#each tiles as tl (tl.key)}
      <img src={tl.src} alt="" decoding="async" draggable="false" style={`left:${tl.x}px;top:${tl.y}px;width:${tl.w}px;height:${tl.h}px`} />
    {/each}
  </div>
  {#each pins as pin (pin.id)}
    {@const s = screenOf(posOf(pin.id, { x: pin.xNorm, y: pin.yNorm }))}
    <button
      class="pin"
      class:selected={pin.id === selectedId}
      class:dragging={drag?.id === pin.id}
      style={`transform:translate(${s.x}px,${s.y}px)`}
      onpointerdown={(e) => pinPointer(e, pin.id, { x: pin.xNorm, y: pin.yNorm })}
      aria-label={String(pin.refNo)}
    ><span class="mark"></span><span class="num">{pin.refNo}</span></button>
  {/each}
  {#if draft}
    {@const s = screenOf(posOf("draft", draft))}
    <button class="pin draft" class:dragging={drag?.id === "draft"} style={`transform:translate(${s.x}px,${s.y}px)`} onpointerdown={(e) => pinPointer(e, "draft", draft)} aria-label="draft"><span class="mark"></span><span class="num">+</span></button>
  {/if}
  <div class="zoom">{zoom.toFixed(1)}×</div>
</div>

<style>
  .viewer { position: absolute; inset: 0; overflow: hidden; background: #8a8f96; touch-action: none; user-select: none; -webkit-user-select: none; }
  .world { position: absolute; left: 0; top: 0; transform-origin: 0 0; background: #fff; }
  .world img { position: absolute; max-width: none; pointer-events: none; }
  /* A pin is a 34 px teardrop whose tip sits on the point; it keeps its size at every zoom. */
  .pin {
    position: absolute; left: -17px; top: -46px; width: 34px; height: 46px; min-height: 0; padding: 0;
    border: none; background: none; touch-action: none; cursor: grab;
  }
  .mark {
    position: absolute; left: 0; top: 0; width: 34px; height: 34px; box-sizing: border-box;
    border-radius: 50% 50% 50% 0; transform: translate(0, 6px) rotate(-45deg); transform-origin: 50% 50%;
    background: #c62828; border: 2px solid #fff; box-shadow: 0 1px 4px rgba(0, 0, 0, 0.45);
  }
  .num { position: absolute; left: 0; top: 6px; width: 34px; line-height: 34px; text-align: center; color: #fff; font-weight: 700; font-size: 13px; }
  .pin.selected .mark { background: #143c78; box-shadow: 0 0 0 3px #ffd54f; }
  .pin.draft .mark { background: #fff; border: 2px dashed #c62828; }
  .pin.draft .num { color: #c62828; font-size: 22px; }
  .pin.dragging { cursor: grabbing; z-index: 2; }
  .zoom { position: absolute; right: 8px; top: 8px; color: #fff; background: rgba(0, 0, 0, 0.55); padding: 2px 8px; border-radius: 10px; font-size: 12px; pointer-events: none; }
</style>
