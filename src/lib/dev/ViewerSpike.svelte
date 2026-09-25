<script lang="ts">
  // D-014 viewer spike controls: profile (1b), design 0 = D-006 baseline, design 1 (progressive
  // base + tile), tile generation + design 2 (pyramid). Every line also goes to spike.log.
  import { onMount } from "svelte";
  import PdfViewer from "./PdfViewer.svelte";
  import TileViewer from "./TileViewer.svelte";
  import { makeLogger, makeMark, type Build, type PlanKey } from "./spike14/pdf";
  import { profileRun } from "./spike14/profile";
  import { generatePyramid } from "./spike14/tiles";

  type View = "none" | "d0" | "d1" | "d2";
  let plan = $state<PlanKey>("a1");
  let build = $state<Build>("legacy");
  let bench = $state(true);
  let enc = $state<"image/webp" | "image/jpeg">("image/webp");
  let transport = $state<"array" | "b64">("array");
  let pauseS = $state(6);
  let view = $state<View>("none");
  let run = $state(0);
  let busy = $state(false);
  let lines = $state<string[]>([]);
  const log = makeLogger((l) => (lines = [...lines.slice(-30), l.slice(11, 23) + l.slice(24)]));
  const mark = makeMark(log, () => pauseS * 1000);
  const key = $derived(`${plan}-${build}`);

  function show(v: View) {
    view = v;
    run++;
  }
  async function job(f: () => Promise<void>) {
    view = "none";
    busy = true;
    try { await f(); } catch (e: any) { log(`error: ${e?.message ?? e}`); } finally { busy = false; }
  }

  onMount(() => {
    const vis = () => log(`VISIBILITY ${document.visibilityState}`);
    document.addEventListener("visibilitychange", vis);
    return () => document.removeEventListener("visibilitychange", vis);
  });
</script>

<div class="spike">
  <div class="bar">
    <select bind:value={plan} disabled={busy}><option value="a1">A1</option><option value="a4">A4 (1A)</option></select>
    <select bind:value={build} disabled={busy}><option value="legacy">legacy</option><option value="modern">modern</option></select>
    <label><input type="checkbox" bind:checked={bench} /> bench</label>
    <select bind:value={enc} disabled={busy}><option value="image/webp">webp</option><option value="image/jpeg">jpeg</option></select>
    <select bind:value={transport} disabled={busy}><option value="array">array</option><option value="b64">b64</option></select>
    <label>pause <input type="number" min="0" max="30" bind:value={pauseS} /> s</label>
    <button disabled={busy} onclick={() => job(() => profileRun(build, plan, log, mark))}>Profile</button>
    <button disabled={busy} onclick={() => show("d0")}>D0 4096</button>
    <button disabled={busy} onclick={() => show("d1")}>D1</button>
    <button disabled={busy} onclick={() => job(() => generatePyramid(build, plan, key, log, { type: enc, quality: enc === "image/jpeg" ? 0.85 : 0.8, transport }))}>Gen tiles</button>
    <button disabled={busy} onclick={() => show("d2")}>D2</button>
    <button disabled={busy} onclick={() => show("none")}>✕</button>
    <button onclick={() => (lines = [])}>clear</button>
  </div>
  <div class="stage">
    {#key run}
      {#if view === "d0"}<PdfViewer {build} {plan} maxBase={4096} quickPx={0} {bench} {log} {mark} />
      {:else if view === "d1"}<PdfViewer {build} {plan} maxBase={3072} quickPx={1024} {bench} {log} {mark} />
      {:else if view === "d2"}<TileViewer tilesKey={key} {bench} {log} {mark} />{/if}
    {/key}
    <pre class="hud">{lines.join("\n")}</pre>
  </div>
</div>

<style>
  .spike { display: flex; flex-direction: column; height: 100%; }
  .bar { display: flex; flex-wrap: wrap; gap: 0.3rem; padding: 0.3rem 0.5rem; align-items: center; background: #e9eef6; font-size: 13px; }
  .bar button, .bar select { padding: 0.25rem 0.5rem; min-height: 34px; }
  .bar input[type="number"] { width: 3.2em; }
  .stage { flex: 1; min-height: 0; position: relative; }
  .hud {
    position: absolute; left: 6px; bottom: 6px; right: 70px; max-height: 38%; overflow: hidden; margin: 0;
    background: rgba(0,0,0,.7); color: #eee; font-size: 10px; padding: 4px 6px; border-radius: 6px;
    pointer-events: none; white-space: pre-wrap; word-break: break-all;
  }
</style>
