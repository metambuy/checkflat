<script lang="ts">
  // One plan: tile viewer + pins. "+" enters add-pin mode; the next tap places a draft pin (not
  // persisted, no number) and leaves the mode; "Next" opens the observation sheet, whose Save
  // calls create_observation, which takes the number; Cancel/Back closes the sheet, then leaves
  // the mode or discards the draft (no gap). "Edit" on a selected pin opens the same sheet for
  // its description and photos (update_observation). The tile pyramid is generated here, in the
  // foreground, the first time the plan is opened (D-014).
  import { onMount, untrack } from "svelte";
  import { api, asAppError, type AppError, type Observation, type Plan, type TileInfo, type TileManifest } from "../api";
  import { t } from "../i18n.svelte";
  import { back, setBackHandler } from "../nav.svelte";
  import ConfirmDialog from "../components/ConfirmDialog.svelte";
  import ErrorBanner from "../components/ErrorBanner.svelte";
  import ObservationSheet from "./ObservationSheet.svelte";
  import PlanViewer from "../viewer/PlanViewer.svelte";
  import { cancel, confirm, moveDraft, noDraft, startAdding, tap, type DraftState } from "../viewer/draft";
  import { ensureTiles, isComplete, viewable, type Progress } from "../viewer/tiles";
  import { backStep } from "../viewer/back";
  import { stageView } from "../viewer/manifest";
  import { revertMove, withPosition } from "../viewer/pins";
  import type { Point } from "../viewer/coords";
  import { devlog } from "../devlog";
  import { createDraftSaver, DRAFT_KEY, type SheetDraft, type SheetState } from "../sheet/draft";

  type SheetPhotos = { photos: { token: string; takenAt: string }[]; removed: string[] };

  let { projectId, planId, recovered = null }: { projectId: string; planId: string; recovered?: SheetDraft | null } = $props();
  // The input of an open sheet is kept stored (SQLite) until the sheet closes, so it survives Android
  // killing the app, e.g. behind the camera (sheet/draft.ts).
  // Seeds the first sheet only; closing it (save, cancel, Back) ends the restore.
  let restoredDraft = $state<SheetDraft | null>(untrack(() => recovered));
  const saver = createDraftSaver((raw) => api.setSetting(DRAFT_KEY, raw));

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
  let sheet = $state<"create" | "edit" | null>(null);
  let editSaving = $state(false);
  let sheetError = $state<AppError | null>(null);
  let viewer = $state<PlanViewer | null>(null);
  let leaving = false;
  const opened = performance.now();

  const selected = $derived(pins.find((p) => p.id === selectedId) ?? null);
  const percent = $derived(progress && progress.total ? Math.floor((progress.done / progress.total) * 100) : 0);
  const stage = $derived(stageView(manifest !== null, genFailed, progress !== null));

  async function prepare() {
    if (!plan || !info) return;
    genFailed = false;
    const t0 = performance.now();
    devlog(`tiles: start (levels on disk: ${info.manifest?.levels.map((l) => l.size).join("/") || "none"})`);
    try {
      const m = await ensureTiles(plan, {
        onProgress: (p) => (progress = { ...p }),
        onLevel: (m) => {
          manifest = m;
          devlog(`tiles: level ${m.levels[m.levels.length - 1].size} done at ${(performance.now() - t0).toFixed(0)} ms`);
        },
        cancelled: () => leaving,
      });
      if (!leaving) manifest = m;
      progress = null;
      devlog(`tiles: ${leaving ? "paused (left screen)" : "complete"} after ${(performance.now() - t0).toFixed(0)} ms`);
    } catch (e) {
      console.error("[plan] tile generation failed", e);
      genFailed = true;
      progress = null;
      error = asAppError(e);
    }
  }

  onMount(() => {
    setBackHandler(() => {
      switch (backStep({ dialog: toDelete !== null, sheet: sheet !== null, adding: ds.adding, draft: ds.draft !== null, saving: ds.saving || editSaving, selected: selectedId !== null })) {
        case "dialog": toDelete = null; return true;
        case "sheet": closeSheet(); return true;
        case "draft": ds = cancel(ds); return true;
        case "selection": selectedId = null; return true;
        default: return false;
      }
    });
    (async () => {
      try {
        [plan, pins, info] = await Promise.all([api.getPlan(planId), api.listPins(planId), api.planTilesInfo(planId)]);
        // Never a manifest from other generator settings: prepare() clears that cache first, and
        // the viewer then gets the new levels as they complete.
        if (viewable(info.manifest)) manifest = info.manifest;
        if (recovered) restoreSheet(recovered);
        if (!isComplete(info.manifest)) await prepare();
      } catch (e) {
        error = asAppError(e);
      }
    })();
    const key = (e: KeyboardEvent) => {
      if (["INPUT", "TEXTAREA"].includes((e.target as HTMLElement)?.tagName)) return;
      if (e.defaultPrevented) return; // e.g. Escape already closed the delete dialog
      if (e.key === "Escape") back();
      else if (e.key === "+" || e.key === "=") viewer?.zoomBy(1.5);
      else if (e.key === "-") viewer?.zoomBy(1 / 1.5);
      else if (e.key === "0") viewer?.fitView();
    };
    window.addEventListener("keydown", key);
    return () => {
      if (sheet !== null) void saver.clear(); // leaving with the sheet open abandons it
      leaving = true; // stops generation after the current tile; it resumes on the next open
      window.removeEventListener("keydown", key);
    };
  });

  /** The sheet that was open when the app was killed comes back with its input and photos. */
  function restoreSheet(d: SheetDraft) {
    if (d.mode === "create") {
      ds = { adding: false, draft: { x: d.x, y: d.y }, saving: false };
      sheet = "create";
    } else if (pins.some((p) => p.id === d.observationId)) {
      selectedId = d.observationId;
      sheet = "edit";
    }
  }

  function startAdd() {
    selectedId = null;
    ds = startAdding(ds);
  }

  function closeSheet() {
    sheet = null;
    sheetError = null;
    restoredDraft = null;
    void saver.clear();
  }

  /** Input of the open sheet → the stored draft. */
  function sheetState(s: SheetState, immediate: boolean): Promise<void> | void {
    let d: SheetDraft | null = null;
    const base = { v: 1 as const, projectId, planId, ...s, savedAt: Date.now() };
    if (sheet === "create" && ds.draft) d = { ...base, mode: "create", observationId: null, x: ds.draft.x, y: ds.draft.y };
    else if (sheet === "edit" && selectedId) d = { ...base, mode: "edit", observationId: selectedId, x: 0, y: 0 };
    if (!d) return;
    return immediate ? saver.flush(d) : saver.schedule(d);
  }

  /** Save of the sheet: one create_observation for the draft; the list is re-read in the
   * server's order. On failure the sheet stays open with the error. */
  async function saveObservation(fraction: string, description: string, photos: SheetPhotos) {
    sheetError = null;
    try {
      const pin = await confirm(() => ds, (s) => (ds = s), (p) => api.createObservation(planId, p.x, p.y, fraction, description, photos.photos));
      if (!pin) return;
      closeSheet();
      selectedId = pin.id;
      pins = await api.listPins(planId);
    } catch (e) {
      if (sheet) sheetError = asAppError(e);
      else error = asAppError(e);
    }
  }

  /** Save of the sheet in edit mode: description and photos of the selected pin. */
  async function saveEdit(_fraction: string, description: string, photos: SheetPhotos) {
    const pin = selected;
    if (!pin || editSaving) return;
    sheetError = null;
    editSaving = true;
    try {
      await api.updateObservation(pin.id, description, photos.photos, photos.removed);
      pins = await api.listPins(planId);
      closeSheet();
    } catch (e) {
      sheetError = asAppError(e);
    } finally {
      editSaving = false;
    }
  }

  async function movePin(id: string, p: Point) {
    const old = pins.find((o) => o.id === id);
    if (!old) return;
    const previous = { x: old.xNorm, y: old.yNorm };
    pins = withPosition(pins, id, p); // optimistic
    try {
      const saved = await api.movePin(id, p.x, p.y);
      pins = pins.map((o) => (o.id === id ? saved : o));
    } catch (e) {
      pins = revertMove(pins, id, p, previous); // only this pin; the list may have changed meanwhile
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
      <div class="view">
      {#if stage.main === "viewer" && manifest && info}
        <PlanViewer
          bind:this={viewer}
          {manifest}
          dir={info.dir}
          {pins}
          draft={ds.draft}
          {selectedId}
          adding={ds.adding}
          ontap={(p) => (ds = tap(ds, p))}
          onpintap={(id) => { ds = cancel(ds); selectedId = id; }}
          onpinmove={movePin}
          ondraftmove={(p) => (ds = moveDraft(ds, p))}
          onready={() => devlog(`plan ${planId}: first view ${(performance.now() - opened).toFixed(0)} ms after opening`)}
        />
        {#if stage.chip === "progress"}
          <div class="chip">{t("viewer.detail_progress", { percent })}</div>
        {:else if stage.chip === "retry"}
          <div class="chip failed">{t("viewer.detail_failed")} <button onclick={prepare}>{t("viewer.retry")}</button></div>
        {/if}
      {:else if stage.main === "failed"}
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
      </div>

      {#if manifest}
        <div class="actions-bar">
          {#if ds.draft}
            <span class="grow">{t("pin.draft")}</span>
            <button onclick={() => (ds = cancel(ds))} disabled={ds.saving}>{t("common.cancel")}</button>
            <button class="primary" onclick={() => (sheet = "create")} disabled={ds.saving}>{t("common.next")}</button>
          {:else if ds.adding}
            <span class="grow">{t("viewer.tap_hint")}</span>
            <button onclick={() => (ds = cancel(ds))}>{t("common.cancel")}</button>
          {:else if selected}
            <span class="grow">{t("pin.label", { ref: selected.ref })}</span>
            <button onclick={() => (sheet = "edit")}>{t("pin.edit")}</button>
            <button class="danger" onclick={() => (toDelete = selected)}>{t("common.delete")}</button>
            <button onclick={() => (selectedId = null)}>{t("common.close")}</button>
            <button class="primary add" onclick={startAdd} aria-label={t("pin.add")} title={t("pin.add")}>+</button>
          {:else}
            <span class="grow"></span>
            <button onclick={() => viewer?.fitView()}>{t("viewer.fit")}</button>
            <button class="primary add" onclick={startAdd} aria-label={t("pin.add")} title={t("pin.add")}>+</button>
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
            <li><button class="pin-row" class:active={pin.id === selectedId} onclick={() => selectFromList(pin)}>{t("pin.label", { ref: pin.ref })}</button></li>
          {/each}
        </ul>
      {/if}
    </aside>
  </div>

  {#if sheet === "create" && ds.draft}
    <ObservationSheet {projectId} initial={restoredDraft?.mode === "create" ? restoredDraft : null} onstate={sheetState} saving={ds.saving} error={sheetError} onsave={saveObservation} oncancel={closeSheet} ondismiss={() => (sheetError = null)} />
  {:else if sheet === "edit" && selected}
    <ObservationSheet {projectId} observation={selected} initial={restoredDraft?.mode === "edit" ? restoredDraft : null} onstate={sheetState} saving={editSaving} error={sheetError} onsave={saveEdit} oncancel={closeSheet} ondismiss={() => (sheetError = null)} />
  {/if}
  <ConfirmDialog
    open={toDelete !== null}
    message={toDelete ? t("pin.delete_confirm", { ref: toDelete.ref }) : ""}
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
  .stage { flex: 1; min-width: 0; display: flex; flex-direction: column; }
  .view { flex: 1; min-height: 0; position: relative; }
  .panel { display: none; }
  .centre { position: absolute; inset: 0; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 0.5rem; padding: 16px; text-align: center; }
  .bar { width: min(320px, 80%); height: 8px; background: #dde3ec; border-radius: 4px; overflow: hidden; }
  .bar div { height: 100%; background: #143c78; transition: width 0.2s; }
  .chip { position: absolute; left: 8px; top: 8px; background: rgba(20, 60, 120, 0.9); color: #fff; font-size: 12px; padding: 3px 10px; border-radius: 12px; pointer-events: none; }
  .chip.failed { display: flex; align-items: center; gap: 0.5rem; background: rgba(179, 38, 30, 0.95); pointer-events: auto; padding: 4px 4px 4px 10px; }
  .chip.failed button { min-height: 0; padding: 4px 10px; font-size: 12px; }
  /* Below the viewer, not over it, so "fit" shows the whole plan. */
  .actions-bar {
    display: flex; align-items: center; gap: 0.5rem; padding: 0.5rem 12px calc(0.5rem + env(safe-area-inset-bottom));
    background: #fff; border-top: 1px solid #ddd;
  }
  .add { min-width: 44px; font-size: 20px; font-weight: 700; line-height: 1; }
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
