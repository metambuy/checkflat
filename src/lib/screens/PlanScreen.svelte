<script lang="ts">
  // One plan: tile viewer + pins. "+" enters add-pin mode; the next tap places a draft pin (not
  // persisted, no number) and leaves the mode; "Next" opens the observation sheet, whose Save
  // calls create_observation, which takes the number; Cancel/Back closes the sheet, then leaves
  // the mode or discards the draft (no gap). "Edit" on a selected pin opens the same sheet for
  // its description and photos (update_observation). "Move" on a selected pin makes it the draft:
  // drag it (or tap the plan), Confirm saves the new place, Cancel/Back leaves it where it was;
  // stored pins never move otherwise (D-027). Arriving from the observation list (`focusId`)
  // selects and centres that pin. The tile pyramid is generated here, in the foreground, the first
  // time the plan is opened (D-014).
  import { onMount, untrack } from "svelte";
  import { api, asAppError, type AppError, type Observation, type Plan, type TileInfo, type TileManifest } from "../api";
  import { t } from "../i18n.svelte";
  import { back, go, setBackHandler } from "../nav.svelte";
  import ConfirmDialog from "../components/ConfirmDialog.svelte";
  import ErrorBanner from "../components/ErrorBanner.svelte";
  import ObservationSheet from "./ObservationSheet.svelte";
  import PlanViewer from "../viewer/PlanViewer.svelte";
  import { cancel, confirm, moveDraft, noDraft, sheetAfter, showsCreateSheet, startAdding, startMoving, tap, type DraftState, type SheetMode } from "../viewer/draft";
  import { ensureTiles, isComplete, viewable, type Progress } from "../viewer/tiles";
  import { backStep } from "../viewer/back";
  import { stageView } from "../viewer/manifest";
  import { listRoute } from "../list/rows";
  import Icon from "../components/Icon.svelte";
  import { devlog } from "../devlog";
  import { createDraftSaver, DRAFT_KEY, restoreTarget, type SheetDraft, type SheetState } from "../sheet/draft";
  import { sheetIsDirty } from "../sheet/discard";
  import { saveThenRefresh } from "../sheet/flow";

  import type { SheetPhotos } from "../sheet/photos";

  let { projectId, planId, recovered = null, focusId = null }: { projectId: string; planId: string; recovered?: SheetDraft | null; focusId?: string | null } = $props();
  // The input of an open sheet is kept stored (SQLite) until the sheet closes, so it survives Android
  // killing the app, e.g. behind the camera (sheet/draft.ts).
  // Seeds the first sheet only; closing it (save, cancel, Back) ends the restore.
  let restoredDraft = $state<SheetDraft | null>(untrack(() => recovered));
  const saver = createDraftSaver((raw) => api.setSetting(DRAFT_KEY, raw), 400, (e) => console.warn("[draft] could not store the sheet draft", e));

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
  let sheet = $state<SheetMode>(null);
  let editSaving = $state(false);
  let sheetError = $state<AppError | null>(null);
  // "Discard?" is asked before a sheet that holds input is closed (Cancel, Back, Escape); the latest
  // input of the open sheet comes in through `sheetState`.
  let confirmDiscard = $state(false);
  let sheetInput: SheetState | null = null;
  let viewer = $state<PlanViewer | null>(null);
  // Read once: the pin a tap in the observation list asked for, selected and centred when the
  // viewer is up and has a size (the effect below). Not state: only that effect uses it.
  let pendingFocus: string | null = untrack(() => focusId);
  // The pin the jump selected, while the user has not chosen another: that selection is not a Back
  // step (Back goes straight to the list), unlike one the user made.
  let jumpedId: string | null = null;
  let leaving = false;
  const opened = performance.now();

  const selected = $derived(pins.find((p) => p.id === selectedId) ?? null);
  // Move mode: the pin being moved is the viewer's draft, so it is not drawn at its old place too.
  const moved = $derived(ds.moving ? (pins.find((p) => p.id === ds.moving) ?? null) : null);
  const shownPins = $derived(ds.moving ? pins.filter((p) => p.id !== ds.moving) : pins);
  const createShown = $derived(showsCreateSheet(sheet, ds));
  const sheetShown = $derived(createShown || (sheet === "edit" && selected !== null));
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
      switch (backStep({ dialog: toDelete !== null, discard: confirmDiscard, sheet: sheet !== null, adding: ds.adding, draft: ds.draft !== null, saving: ds.saving || editSaving, selected: selectedId !== null && selectedId !== jumpedId })) {
        case "dialog": toDelete = null; return true;
        case "discard": confirmDiscard = false; return true; // keep editing
        case "busy": return true; // a save is in flight: Back does nothing
        case "sheet": requestCloseSheet(); return true;
        case "draft": setDraft(cancel(ds)); return true;
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
    const target = restoreTarget(d, pins.map((p) => p.id));
    if (!target) {
      // Nothing to reopen it on: drop it, so it can never seed another observation's sheet.
      restoredDraft = null;
      void saver.clear();
    } else if (target.sheet === "create") {
      setDraft({ ...noDraft, draft: { x: d.x, y: d.y } });
      sheet = "create";
    } else {
      selectedId = target.selectedId;
      sheet = "edit";
    }
  }

  function startAdd() {
    selectedId = null;
    setDraft(startAdding(ds));
  }

  /** Every change of the draft goes through here: a create sheet closes with its draft, wherever
   * the draft was cleared (Cancel, Back, a pin selected, a successful Save), and never stays open
   * into a move (D-027, review 1e). */
  function setDraft(next: DraftState) {
    ds = next;
    if (sheetAfter(sheet, next) !== sheet) closeSheet();
  }

  function closeSheet() {
    sheet = null;
    sheetError = null;
    restoredDraft = null;
    confirmDiscard = false;
    sheetInput = null;
    void saver.clear();
  }

  /** Cancel, Back and Escape on the sheet: a sheet that holds photos, shapes or text asks first
   * (a camera photo exists only in the app's cache until Save); an untouched one just closes. */
  function requestCloseSheet() {
    if (ds.saving || editSaving) return;
    const stored = sheet === "edit" ? (selected?.description ?? "") : "";
    if (sheetInput && sheetIsDirty(sheetInput, stored)) confirmDiscard = true;
    else closeSheet();
  }

  /** Input of the open sheet → the stored draft. */
  function sheetState(s: SheetState, immediate: boolean): Promise<void> | void {
    sheetInput = s;
    let d: SheetDraft | null = null;
    const base = { v: 1 as const, projectId, planId, ...s, savedAt: Date.now() };
    if (sheet === "create" && ds.draft) d = { ...base, mode: "create", observationId: null, x: ds.draft.x, y: ds.draft.y };
    else if (sheet === "edit" && selectedId) d = { ...base, mode: "edit", observationId: selectedId, x: 0, y: 0 };
    if (!d) return;
    return immediate ? saver.flush(d) : saver.schedule(d);
  }

  /** Save of the sheet: one create_observation for the draft; the sheet closes with the draft on
   * success (setDraft) and the list is re-read in the server's order. On failure the sheet stays
   * open with the error. */
  async function saveObservation(fraction: string, description: string, photos: SheetPhotos) {
    sheetError = null;
    try {
      const pin = await confirm(() => ds, setDraft, (p) => api.createObservation(planId, p.x, p.y, fraction, description, photos.photos));
      if (!pin) return;
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
      await saveThenRefresh({
        save: async () => void (await api.updateObservation(pin.id, description, photos.photos, photos.removed, photos.annotations)),
        onSaved: closeSheet,
        refresh: async () => void (pins = await api.listPins(planId)),
        onSaveError: (e) => (sheetError = asAppError(e)),
        onRefreshError: (e) => (error = asAppError(e)),
      });
    } finally {
      editSaving = false;
    }
  }

  /** Move on the selected pin: it becomes the draft, at its place. */
  function startMove() {
    if (selected) setDraft(startMoving(ds, selected.id, { x: selected.xNorm, y: selected.yNorm }));
  }

  /** Confirm of the move: one move_pin; the saved pin replaces the old one in the list (a move never changes the order). On failure the draft stays, with the error. */
  async function confirmMove() {
    const id = ds.moving;
    if (!id) return;
    try {
      const saved = await confirm(() => ds, setDraft, (p) => api.movePin(id, p.x, p.y));
      if (saved) pins = pins.map((o) => (o.id === saved.id ? saved : o));
    } catch (e) {
      error = asAppError(e);
    }
  }

  /** A tap on a stored pin selects it. While a pin is being moved (or saved) other pins wait. */
  function pinTapped(id: string) {
    if (ds.moving || ds.saving) return;
    setDraft(cancel(ds));
    selectedId = id;
    jumpedId = null;
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

  function selectFromList(pin: Observation, jumped = false) {
    if (ds.moving || ds.saving) return;
    setDraft(cancel(ds));
    selectedId = pin.id;
    jumpedId = jumped ? pin.id : null;
    viewer?.reveal({ x: pin.xNorm, y: pin.yNorm });
  }

  // The jump from the observation list: once the viewer is mounted and has a size (hasView() reads
  // the viewer's size, so this runs again when a 0×0 mount gets its real one). The tiles may still
  // be generating; the viewer is up from the first level. A pin that is gone is ignored.
  $effect(() => {
    if (!pendingFocus || !viewer || !viewer.hasView()) return;
    const id = pendingFocus;
    pendingFocus = null;
    const pin = pins.find((p) => p.id === id);
    if (pin) untrack(() => selectFromList(pin, true));
  });
</script>

<div class="plan-screen">
  <!-- While a sheet is open the screen behind it is inert, as the sheet is while the annotation editor is open: the backdrop
       stops the pointer, this stops Tab/Shift+Tab, so nothing under the sheet can cancel its draft or select a pin (D-027, review 1e). -->
  <div class="behind" inert={sheetShown}>
    <div class="topbar">
      <button class="plain back" onclick={() => back()}>← {t("nav.back")}</button>
      <strong class="grow title">{plan?.title ?? ""}</strong>
      <span class="muted">{pins.length === 1 ? t("pin.count_one") : t("pin.count", { count: pins.length })}</span>
      <button class="icon" onclick={() => go(listRoute(projectId, planId))} disabled={ds.saving || editSaving} aria-label={t("observations.title")} title={t("observations.title")}><Icon name="list" /></button>
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
            pins={shownPins}
            draft={ds.draft}
            draftLabel={moved?.marker ?? null}
            {selectedId}
            adding={ds.adding}
            ontap={(p) => setDraft(tap(ds, p))}
            onpintap={pinTapped}
            ondraftmove={(p) => setDraft(moveDraft(ds, p))}
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
          <div class="actions-bar" class:compact={selected !== null && !ds.draft && !ds.adding}>
            {#if ds.moving}
              <span class="grow hint">{t("pin.move_hint")}</span>
              <button onclick={() => setDraft(cancel(ds))} disabled={ds.saving}>{t("common.cancel")}</button>
              <button class="primary" onclick={confirmMove} disabled={ds.saving}>{t("common.confirm")}</button>
            {:else if ds.draft}
              <span class="grow">{t("pin.draft")}</span>
              <button onclick={() => setDraft(cancel(ds))} disabled={ds.saving}>{t("common.cancel")}</button>
              <button class="primary" onclick={() => (sheet = "create")} disabled={ds.saving}>{t("common.next")}</button>
            {:else if ds.adding}
              <span class="grow">{t("viewer.tap_hint")}</span>
              <button onclick={() => setDraft(cancel(ds))}>{t("common.cancel")}</button>
            {:else if selected}
              <span class="grow hint" title={t("pin.label", { ref: selected.ref })}>{t("pin.label", { ref: selected.ref })}</span>
              <button class="icon" onclick={() => (sheet = "edit")} aria-label={t("pin.edit")} title={t("pin.edit")}><Icon name="edit" /></button>
              <button class="icon" onclick={startMove} aria-label={t("pin.move")} title={t("pin.move")}><Icon name="move" /></button>
              <button class="icon danger" onclick={() => (toDelete = selected)} aria-label={t("common.delete")} title={t("common.delete")}><Icon name="trash" /></button>
              <button class="icon" onclick={() => (selectedId = null)} aria-label={t("common.close")} title={t("common.close")}><Icon name="close" /></button>
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
              <li><button class="pin-row" class:active={pin.id === selectedId} onclick={() => selectFromList(pin)} disabled={ds.moving !== null}>{t("pin.label", { ref: pin.ref })}</button></li>
            {/each}
          </ul>
        {/if}
      </aside>
    </div>
  </div>

  {#if createShown}
    <ObservationSheet {projectId} initial={restoredDraft?.mode === "create" ? restoredDraft : null} onstate={sheetState} saving={ds.saving} error={sheetError} locked={confirmDiscard} onsave={saveObservation} oncancel={requestCloseSheet} ondismiss={() => (sheetError = null)} />
  {:else if sheet === "edit" && selected}
    <ObservationSheet {projectId} observation={selected} initial={restoredDraft?.mode === "edit" ? restoredDraft : null} onstate={sheetState} saving={editSaving} error={sheetError} locked={confirmDiscard} onsave={saveEdit} oncancel={requestCloseSheet} ondismiss={() => (sheetError = null)} />
  {/if}
  <ConfirmDialog
    open={confirmDiscard}
    message={t("sheet.discard_confirm")}
    confirmLabel={t("sheet.discard")}
    cancelLabel={t("sheet.keep_editing")}
    danger
    onconfirm={closeSheet}
    oncancel={() => (confirmDiscard = false)}
  />
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
  .behind { flex: 1; min-height: 0; display: flex; flex-direction: column; }
  .topbar { display: flex; align-items: center; gap: 0.5rem; padding: 0.25rem 12px; background: #fff; border-bottom: 1px solid #ddd; }
  .title { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .behind > :global(.banner) { margin: 0.4rem 12px; }
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
  /* The selected pin's bar: the label and five 44 px buttons on one row at 360 px; the label wraps to two lines at most. */
  .actions-bar.compact { gap: 0.25rem; }
  .hint { min-width: 0; font-size: 14px; line-height: 1.2; overflow-wrap: anywhere; }
  .compact .hint { display: -webkit-box; -webkit-line-clamp: 2; line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden; }
  .topbar .icon { flex: none; }
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
