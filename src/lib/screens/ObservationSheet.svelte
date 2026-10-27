<script lang="ts">
  // Observation sheet. Create mode (a draft pin): fraction (type-to-add from the project's list),
  // description and photos. Edit mode (`observation` given): description and photos only; the
  // fraction and the ref are shown, not editable. Save hands the input to the plan screen, which
  // calls create_observation / update_observation once; nothing is saved here. Photos are staged
  // as they are added (processed JPEGs in tmp/); one to MAX_PHOTOS must remain, and the camera and
  // the picker are not offered at the limit; staged files that are not saved are discarded when the
  // sheet goes away.
  import { onDestroy, onMount, untrack } from "svelte";
  import { convertFileSrc } from "@tauri-apps/api/core";
  import { api, asAppError, type AppError, type Fraction, type Observation } from "../api";
  import { t } from "../i18n.svelte";
  import { livePreview } from "../livePreview";
  import { capturePath, pickPath, stagePath } from "../photoSource";
  import { isAndroid } from "../platform";
  import { addStaged, canAdd, canSave, count, MAX_PHOTOS, noPhotos, removeExisting, removeStaged, setAnnotation, shown, startAdd, toSave, watchReturn, withExisting, type Phase, type PhotoItem, type PhotoSet, type SheetPhotos } from "../sheet/photos";
  import { devlog } from "../devlog";
  import type { SheetState } from "../sheet/draft";
  import type { Size } from "../annotate/geometry";
  import type { Annotation } from "../annotate/model";
  import AnnotationEditor from "../annotate/AnnotationEditor.svelte";
  import ShapesOverlay from "../annotate/ShapesOverlay.svelte";
  import ErrorBanner from "../components/ErrorBanner.svelte";

  let {
    projectId,
    observation = null,
    initial = null,
    onstate,
    saving,
    error,
    locked = false,
    onsave,
    oncancel,
    ondismiss,
  }: {
    projectId: string;
    /** Edit mode: the observation being edited. */
    observation?: Observation | null;
    /** A restored draft (after the app was killed): the input to start from. */
    initial?: SheetState | null;
    /** The sheet's input changed (debounced), or must be stored now (`immediate`, before the camera or picker opens). */
    onstate?: (s: SheetState, immediate: boolean) => void | Promise<void>;
    saving: boolean;
    /** The failed save, shown here so the user can fix the input and retry. */
    error: AppError | null;
    /** Inert while a question about the sheet ("Discard?") is open over it. */
    locked?: boolean;
    onsave: (fraction: string, description: string, photos: SheetPhotos) => void;
    oncancel: () => void;
    ondismiss: () => void;
  } = $props();

  let fractions = $state<Fraction[]>([]);
  // The mode and the starting text are fixed for the life of the sheet (the plan screen mounts a
  // new one per open), so reading them once is intended.
  const editing = untrack(() => observation !== null);
  let fraction = $state(untrack(() => initial?.fraction ?? ""));
  let description = $state(untrack(() => initial?.description ?? observation?.description ?? ""));
  let photos = $state<PhotoSet>(untrack(() => (initial ? { existing: [], staged: initial.staged, removed: initial.removed, annotations: initial.annotations } : noPhotos)));
  let phase = $state<Phase>("idle");
  /** The photo open in the annotation editor. */
  let annotating = $state<PhotoItem | null>(null);
  /** Natural sizes of the thumbnails' images, for their overlays. */
  let sizes = $state<Record<string, Size>>({});
  const busy = $derived(phase !== "idle");
  // Edit mode: the limit counts the stored photos, so the camera and the picker wait until they
  // are listed, and stay off if listing fails (the error is shown).
  let loaded = $state(untrack(() => observation === null));
  const blocked = $derived(busy || !loaded);
  let photoError = $state<AppError | null>(null);
  let camera = $state(false);
  let preview = $state<string | null>(null);
  let previewError = $state<AppError | null>(null);
  let input = $state<HTMLInputElement | null>(null);

  // Case folding matches the backend (fractions::find compares to_lowercase, full Unicode).
  const typed = $derived(fraction.trim());
  const exact = $derived(fractions.find((f) => f.code.toLowerCase() === typed.toLowerCase()) ?? null);
  const suggestions = $derived(
    fractions.filter((f) => f.code.toLowerCase().startsWith(typed.toLowerCase()) && f !== exact),
  );

  const previewer = livePreview<string>({
    compute: (f) => api.previewRef(projectId, f),
    show: (text, e) => {
      preview = text;
      previewError = e === null ? null : asAppError(e);
    },
  });
  $effect(() => {
    if (!editing) previewer.update(typed);
  });

  const snapshot = (): SheetState => ({ fraction, description, staged: photos.staged, removed: photos.removed, annotations: photos.annotations });
  // Every change of the input goes to the plan screen, which keeps the draft stored while the sheet is open.
  $effect(() => {
    const s = snapshot();
    untrack(() => void onstate?.(s, false));
  });

  onMount(() => {
    void isAndroid().then((a) => (camera = a));
    if (observation) {
      void api
        .listPhotos(observation.id)
        .then((p) => {
          photos = withExisting(photos, p);
          loaded = true;
        })
        .catch((e) => (photoError = asAppError(e)));
    } else {
      void api
        .listFractions(projectId)
        .then((f) => (fractions = f))
        .catch(() => {}); // the list is a convenience; typing still works
      input?.focus();
    }
    return () => previewer.cancel();
  });

  // Staged files that were not saved go away with the sheet. A saved one was moved out of tmp/
  // already, and discarding a missing file is a no-op, so this also covers a successful save.
  onDestroy(() => {
    for (const p of photos.staged) void api.discardStagedPhoto(p.token).catch(() => {});
  });

  async function add(getPath: () => Promise<string | null>) {
    // "Preparing photo…" from the moment the camera/picker hands control back until the file is
    // processed (HEIC conversion, cloud download); every way out ends idle.
    const r = await startAdd(
      () => !saving && canAdd(photos, blocked),
      () => onstate?.(snapshot(), true),
      { getPath, stage: stagePath, watchReturn: () => watchReturn(window, document) },
      (p) => {
        if (p === "picking") photoError = null;
        phase = p;
        devlog(`photo: ${p}`);
      },
    );
    if (!r) return;
    if (r.staged) photos = addStaged(photos, r.staged);
    if (r.error) photoError = asAppError(r.error);
  }
  function removeItem(key: string, staged: boolean) {
    if (staged) {
      photos = removeStaged(photos, key);
      void api.discardStagedPhoto(key).catch(() => {});
    } else {
      photos = removeExisting(photos, key);
    }
  }

  function choose(code: string) {
    fraction = code;
  }
  function sized(key: string, e: Event) {
    const img = e.currentTarget as HTMLImageElement;
    sizes = { ...sizes, [key]: { w: img.naturalWidth, h: img.naturalHeight } };
  }
  function openEditor(item: PhotoItem) {
    (document.activeElement as HTMLElement | null)?.blur?.(); // soft keyboard down before the editor takes the screen
    annotating = item;
  }
  function annotated(a: Annotation | null) {
    if (annotating) photos = setAnnotation(photos, annotating.key, annotating.staged, a);
    annotating = null;
  }
  function submit(e: Event) {
    e.preventDefault();
    if (!saving && canSave(photos, busy)) onsave(typed, description, toSave(photos));
  }
</script>

<div class="backdrop" role="presentation">
  <!-- Escape is marked handled so the plan screen's Back does not act on the same key press. While the annotation editor (a sibling, not a child) is open the sheet is inert: Tab cannot reach it and its own Escape handler stays out of the way. -->
  <div class="dialog sheet" role="dialog" aria-modal="true" tabindex="-1" inert={annotating !== null || locked} onkeydown={(e) => { if (e.key === "Escape") { e.preventDefault(); if (!saving) oncancel(); } }}>
  <form onsubmit={submit}>
    <h2>{editing ? t("sheet.title_edit") : t("sheet.title")}</h2>
    <ErrorBanner error={error ?? photoError} ondismiss={() => { ondismiss(); photoError = null; }} />
    {#if observation}
      <p class="preview">{t("sheet.will_be", { ref: observation.ref })}</p>
    {:else}
      <label>
        {t("sheet.fraction")}
        <input bind:this={input} bind:value={fraction} placeholder={t("sheet.fraction_placeholder")} autocomplete="off" autocapitalize="characters" spellcheck="false" maxlength="16" />
      </label>
      <div class="chips" aria-label={t("settings.fractions")}>
        {#if typed && !exact}
          <button type="button" class="chip add" onclick={() => choose(typed)}>+ {t("sheet.fraction_add", { code: typed })}</button>
        {/if}
        {#each suggestions as f (f.id)}
          <button type="button" class="chip" onclick={() => choose(f.code)}>{f.code}</button>
        {/each}
        {#if exact}
          <span class="chip current">{exact.code}</span>
        {/if}
      </div>
      <p class="preview" class:bad={previewError !== null} aria-live="polite">
        {#if previewError}
          {previewError.code === "fraction_required" ? t("error.fraction_required") : previewError.message}
        {:else if preview !== null}
          {t("sheet.will_be", { ref: preview })}
        {/if}
      </p>
    {/if}
    <label>
      {t("sheet.description")}
      <textarea bind:value={description} rows="4"></textarea>
    </label>
    <div class="photos">
      <h3>{t("photos.title")}</h3>
      <ul class="thumbs">
        {#each shown(photos) as item, i (item.key)}
          <li>
            <!-- The thumbnail opens the annotation editor; the stored shapes are drawn over it. -->
            <button type="button" class="thumb" onclick={() => openEditor(item)} disabled={saving} aria-label={t("annotate.open", { n: i + 1 })} title={t("annotate.open", { n: i + 1 })}>
              <img src={convertFileSrc(item.path)} alt={t("photos.thumb", { n: i + 1 })} onload={(e) => sized(item.key, e)} />
              {#if item.annotation && sizes[item.key]}
                <ShapesOverlay shapes={item.annotation.shapes} width={sizes[item.key].w} height={sizes[item.key].h} fit="cover" thumb />
              {/if}
            </button>
            <button type="button" class="x" aria-label={t("photos.remove")} title={t("photos.remove")} onclick={() => removeItem(item.key, item.staged)} disabled={saving}>×</button>
          </li>
        {/each}
      </ul>
      <div class="add-row">
        {#if camera}
          <button type="button" onclick={() => add(capturePath)} disabled={saving || !canAdd(photos, blocked)}>{t("photos.camera")}</button>
        {/if}
        <button type="button" onclick={() => add(pickPath)} disabled={saving || !canAdd(photos, blocked)}>{t("photos.gallery")}</button>
        {#if phase === "preparing"}<span class="muted" aria-live="polite">{t("photos.preparing")}</span>{/if}
      </div>
      {#if !busy && count(photos) === 0}
        <p class="hint">{t("photos.hint")}</p>
      {:else if !busy && count(photos) >= MAX_PHOTOS}
        <p class="hint muted">{t("photos.limit", { count: MAX_PHOTOS })}</p>
      {/if}
    </div>
    <div class="actions">
      <button type="button" onclick={oncancel} disabled={saving}>{t("common.cancel")}</button>
      <button type="submit" class="primary" disabled={saving || !canSave(photos, busy)}>{saving ? t("common.saving") : t("common.save")}</button>
    </div>
  </form>
  </div>
</div>
{#if annotating}
  <!-- Above the sheet (its own fixed layer), outside the sheet's scrolling box. -->
  <AnnotationEditor path={annotating.path} annotation={annotating.annotation} ondone={annotated} oncancel={() => (annotating = null)} />
{/if}

<style>
  .sheet { max-height: 100%; overflow-y: auto; }
  textarea { font: inherit; padding: 0.55rem 0.7rem; border: 1px solid #bbb; border-radius: 6px; resize: vertical; }
  .chips { display: flex; flex-wrap: wrap; gap: 0.4rem; min-height: 2rem; }
  .chip { min-height: 36px; padding: 0.3rem 0.8rem; border-radius: 18px; font-size: 0.9rem; }
  .chip.add { border-style: dashed; color: #143c78; }
  .chip.current { display: inline-flex; align-items: center; background: #e3ebf7; color: #143c78; border: 1px solid #c5d3ea; font-weight: 600; }
  .preview { margin: 0.5rem 0 0; font-weight: 600; color: #143c78; min-height: 1.4em; }
  .preview.bad { color: #b3261e; font-weight: 400; }
  .photos h3 { margin: 0.75rem 0 0.4rem; font-size: 1rem; }
  .thumbs { list-style: none; margin: 0; padding: 0; display: flex; flex-wrap: wrap; gap: 0.5rem; }
  .thumbs li { position: relative; width: 96px; height: 96px; }
  .thumb { position: relative; width: 100%; height: 100%; min-height: 0; padding: 0; border: none; background: none; overflow: hidden; border-radius: 6px; }
  .thumbs img { width: 100%; height: 100%; object-fit: cover; border-radius: 6px; border: 1px solid #ccc; background: #eee; display: block; }
  /* 44 px touch target, drawn as a smaller badge in the corner. */
  .x { position: absolute; top: -10px; right: -10px; width: 44px; height: 44px; min-height: 0; padding: 0; border: none; background: none; font-size: 0; }
  .x::before { content: "×"; position: absolute; top: 10px; right: 10px; width: 24px; height: 24px; line-height: 22px; font-size: 18px; text-align: center; border-radius: 12px; background: #b3261e; color: #fff; }
  .add-row { display: flex; flex-wrap: wrap; align-items: center; gap: 0.5rem; margin-top: 0.5rem; }
  .hint { margin: 0.4rem 0 0; color: #b3261e; font-size: 0.9rem; }
  .hint.muted { color: #666; }
  /* Phones: the sheet fills the screen so the keyboard leaves room for the description. */
  @media (max-width: 599px) {
    :global(.backdrop:has(.sheet)) { align-items: stretch; padding: 0; }
    .sheet { width: 100%; border-radius: 0; padding-bottom: calc(1rem + env(safe-area-inset-bottom)); }
  }
</style>
