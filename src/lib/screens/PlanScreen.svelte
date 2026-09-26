<script lang="ts">
  // One plan: tile viewer + pins. Tap places a draft pin (not persisted, no number); Confirm calls
  // create_pin, which takes the ref number; Cancel/Back discards the draft (no gap). The tile
  // pyramid is generated here, in the foreground, the first time the plan is opened (D-014).
  import { onMount } from "svelte";
  import { api, asAppError, type AppError, type Observation, type Plan, type TileInfo, type TileManifest } from "../api";
  import { t } from "../i18n.svelte";
  import { back, setBackHandler } from "../nav.svelte";
  import ConfirmDialog from "../components/ConfirmDialog.svelte";
  import ErrorBanner from "../components/ErrorBanner.svelte";
  import PlanViewer from "../viewer/PlanViewer.svelte";
  import { cancel, confirm, noDraft, place, type DraftState } from "../viewer/draft";
  import { ensureTiles, isComplete, type Progress } from "../viewer/tiles";
  import type { Point } from "../viewer/coords";

  let { projectId, planId }: { projectId: string; planId: string } = $props();

  let plan = $state<Plan | null>(null);
  let info = $state<TileInfo | null>(null);
  let manifest = $state<TileManifest | null>(null);
  let pins = $state<Observation[]>([]);
  let ds = $state<DraftState>(noDraft);
  let selectedId = $state<string | null>(null);
  let progress = $state<Progress | null>(null);
  let genFailed = $state(false);
  let error = $state<AppError | null>(null);
  let toDelete = $state<Observation | null>(null);
  let viewer = $state<PlanViewer | null>(null);
  let leaving = false;

  const selected = $derived(pins.find((p) => p.id === selectedId) ?? null);
  const percent = $derived(progress && progress.total ? Math.floor((progress.done / progress.total) * 100) : 0);
  const generating = $derived(progress !== null && !genFailed);

  async function prepare() {
    if (!plan || !info) return;
    genFailed = false;
    try {
      const m = await ensureTiles(plan, info, {
        onProgress: (p) => (progress = { ...p }),
        onLevel: (m) => (manifest = m),
        cancelled: () => leaving,
      });
      if (!leaving) manifest = m;
      progress = null;
    } catch (e) {
      console.error("[plan] tile generation failed", e);
      genFailed = true;
      error = asAppError(e);
    }
  }

  onMount(() => {
    setBackHandler(() => {
      if (ds.draft && !ds.saving) { ds = cancel(ds); return true; }
      if (selectedId) { selectedId = null; return true; }
      return false;
    });
    (async () => {
      try {
        [plan, pins, info] = await Promise.all([api.getPlan(planId), api.listPins(planId), api.planTilesInfo(planId)]);
        if (info.manifest?.levels.length) manifest = info.manifest;
        if (!isComplete(info.manifest)) await prepare();
      } catch (e) {
        error = asAppError(e);
      }
    })();
    const key = (e: KeyboardEvent) => {
      if ((e.target as HTMLElement)?.tagName === "INPUT") return;
      if (e.key === "Escape") back();
      else if (e.key === "+" || e.key === "=") viewer?.zoomBy(1.5);
      else if (e.key === "-") viewer?.zoomBy(1 / 1.5);
      else if (e.key === "0") viewer?.fitView();
    };
    window.addEventListener("keydown", key);
    return () => {
      leaving = true; // stops generation after the current tile; it resumes on the next open
      window.removeEventListener("keydown", key);
    };
  });

  function onTap(p: Point) {
    selectedId = null;
    ds = place(ds, p);
  }

  async function confirmDraft() {
    try {
      const pin = await confirm(() => ds, (s) => (ds = s), (p) => api.createPin(planId, p.x, p.y));
      if (pin) pins = [...pins, pin].sort((a, b) => a.refNo - b.refNo);
    } catch (e) {
      error = asAppError(e);
    }
  }

  async function movePin(id: string, p: Point) {
    const before = pins;
    pins = pins.map((o) => (o.id === id ? { ...o, xNorm: p.x, yNorm: p.y } : o)); // optimistic
    try {
      const saved = await api.movePin(id, p.x, p.y);
      pins = pins.map((o) => (o.id === id ? saved : o));
    } catch (e) {
      pins = before;
      error = asAppError(e);
    }
  }

  async function deletePin() {
    const pin = toDelete;
    toDelete = null;
    if (!pin) return;
    try {
      await api.deletePin(pin.id);
      pins = pins.filter((o) => o.id !== pin.id);
      if (selectedId === pin.id) selectedId = null;
    } catch (e) {
      error = asAppError(e);
    }
  }

  function selectFromList(pin: Observation) {
    ds = cancel(ds);
    selectedId = pin.id;
    viewer?.reveal({ x: pin.xNorm, y: pin.yNorm });
  }
</script>

<div class="plan-screen">
  <div class="topbar">
    <button class="plain back" onclick={() => back()}>← {t("nav.back")}</button>
    <strong class="grow title">{plan?.title ?? ""}</strong>
    <span class="muted">{pins.length === 1 ? t("pin.count_one") : t("pin.count", { count: pins.length })}</span>
  </div>
  <ErrorBanner {error} ondismiss={() => (error = null)} />

  <div class="body">
    <div class="stage">
      {#if manifest && info}
        <PlanViewer
          bind:this={viewer}
          {manifest}
          dir={info.dir}
          {pins}
          draft={ds.draft}
          {selectedId}
          ontap={onTap}
          onpintap={(id) => { ds = cancel(ds); selectedId = id; }}
          onpinmove={movePin}
          ondraftmove={(p) => (ds = place(ds, p))}
        />
        {#if generating}<div class="chip">{t("viewer.detail_progress", { percent })}</div>{/if}
      {:else if genFailed}
        <div class="centre">
          <p>{t("viewer.failed")}</p>
          <button class="primary" onclick={prepare}>{t("viewer.retry")}</button>
        </div>
      {:else}
        <div class="centre">
          <p><strong>{t("viewer.preparing")}</strong></p>
          {#if progress}
            <div class="bar"><div style={`width:${percent}%`}></div></div>
            <p class="muted">{t("viewer.preparing_detail", { percent })}</p>
          {/if}
        </div>
      {/if}

      {#if manifest}
        <div class="actions-bar">
          {#if ds.draft}
            <span class="grow">{t("pin.draft")}</span>
            <button onclick={() => (ds = cancel(ds))} disabled={ds.saving}>{t("common.cancel")}</button>
            <button class="primary" onclick={confirmDraft} disabled={ds.saving}>{t("common.confirm")}</button>
          {:else if selected}
            <span class="grow">{t("pin.label", { ref: selected.refNo })}</span>
            <button class="danger" onclick={() => (toDelete = selected)}>{t("common.delete")}</button>
            <button onclick={() => (selectedId = null)}>{t("common.close")}</button>
          {:else}
            <span class="grow muted">{t("viewer.tap_hint")}</span>
            <button onclick={() => viewer?.fitView()}>{t("viewer.fit")}</button>
          {/if}
        </div>
      {/if}
    </div>

    <aside class="panel">
      <h2>{t("pin.list")}</h2>
      {#if pins.length === 0}
        <p class="muted">{t("pin.list_empty")}</p>
      {:else}
        <ul class="list">
          {#each pins as pin (pin.id)}
            <li><button class="pin-row" class:active={pin.id === selectedId} onclick={() => selectFromList(pin)}>{t("pin.label", { ref: pin.refNo })}</button></li>
          {/each}
        </ul>
      {/if}
    </aside>
  </div>

  <ConfirmDialog
    open={toDelete !== null}
    message={toDelete ? t("pin.delete_confirm", { ref: toDelete.refNo }) : ""}
    confirmLabel={t("common.delete")}
    danger
    onconfirm={deletePin}
    oncancel={() => (toDelete = null)}
  />
</div>

<style>
  .plan-screen { position: absolute; inset: 0; display: flex; flex-direction: column; }
  .topbar { display: flex; align-items: center; gap: 0.5rem; padding: 0.25rem 12px; background: #fff; border-bottom: 1px solid #ddd; }
  .title { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .plan-screen > :global(.banner) { margin: 0.4rem 12px; }
  .body { flex: 1; min-height: 0; display: flex; }
  .stage { flex: 1; min-width: 0; position: relative; }
  .panel { display: none; }
  .centre { position: absolute; inset: 0; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 0.5rem; padding: 16px; text-align: center; }
  .bar { width: min(320px, 80%); height: 8px; background: #dde3ec; border-radius: 4px; overflow: hidden; }
  .bar div { height: 100%; background: #143c78; transition: width 0.2s; }
  .chip { position: absolute; left: 8px; top: 8px; background: rgba(20, 60, 120, 0.9); color: #fff; font-size: 12px; padding: 3px 10px; border-radius: 12px; pointer-events: none; }
  .actions-bar {
    position: absolute; left: 0; right: 0; bottom: 0; display: flex; align-items: center; gap: 0.5rem;
    padding: 0.5rem 12px calc(0.5rem + env(safe-area-inset-bottom)); background: rgba(255, 255, 255, 0.96);
    border-top: 1px solid #ddd; box-shadow: 0 -2px 8px rgba(0, 0, 0, 0.08);
  }
  .actions-bar .muted { font-size: 13px; }
  .pin-row { width: 100%; text-align: left; border: none; border-radius: 6px; background: none; padding: 0.5rem 0.6rem; }
  .pin-row.active { background: #e3ebf7; color: #143c78; font-weight: 600; }
  /* Tablet and desktop: pin list beside the plan. */
  @media (min-width: 600px) {
    .panel { display: block; width: 220px; flex: none; overflow-y: auto; background: #fff; border-left: 1px solid #ddd; padding: 0.5rem 0.75rem calc(0.5rem + env(safe-area-inset-bottom)); }
    .panel h2 { margin-top: 0.25rem; }
  }
  @media (min-width: 1024px) {
    .panel { width: 280px; }
  }
</style>
