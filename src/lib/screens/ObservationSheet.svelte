<script lang="ts">
  // Observation sheet. Create mode (a draft pin): fraction (type-to-add from the project's list),
  // description and photos. Edit mode (`observation` given): description and photos only; the
  // fraction and the ref are shown, not editable. Save hands the input to the plan screen, which
  // calls create_observation / update_observation once; nothing is saved here. Photos are staged
  // as they are added (processed JPEGs in tmp/) and at least one must remain; staged files that
  // are not saved are discarded when the sheet goes away.
  import { onDestroy, onMount, untrack } from "svelte";
  import { convertFileSrc } from "@tauri-apps/api/core";
  import { api, asAppError, type AppError, type Fraction, type Observation } from "../api";
  import { t } from "../i18n.svelte";
  import { livePreview } from "../livePreview";
  import { capturePath, pickPath, stagePath } from "../photoSource";
  import { isAndroid } from "../platform";
  import { acquirePhoto, addStaged, canSave, noPhotos, removeExisting, removeStaged, shown, toSave, withExisting, type Phase, type PhotoSet } from "../sheet/photos";
  import ErrorBanner from "../components/ErrorBanner.svelte";

  let {
    projectId,
    observation = null,
    saving,
    error,
    onsave,
    oncancel,
    ondismiss,
  }: {
    projectId: string;
    /** Edit mode: the observation being edited. */
    observation?: Observation | null;
    saving: boolean;
    /** The failed save, shown here so the user can fix the input and retry. */
    error: AppError | null;
    onsave: (fraction: string, description: string, photos: { photos: { token: string; takenAt: string }[]; removed: string[] }) => void;
    oncancel: () => void;
    ondismiss: () => void;
  } = $props();

  let fractions = $state<Fraction[]>([]);
  // The mode and the starting text are fixed for the life of the sheet (the plan screen mounts a
  // new one per open), so reading them once is intended.
  const editing = untrack(() => observation !== null);
  let fraction = $state("");
  let description = $state(untrack(() => observation?.description ?? ""));
  let photos = $state<PhotoSet>(noPhotos);
  let phase = $state<Phase>("idle");
  const busy = $derived(phase !== "idle");
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

  onMount(() => {
    void isAndroid().then((a) => (camera = a));
    if (observation) {
      void api
        .listPhotos(observation.id)
        .then((p) => (photos = withExisting(photos, p)))
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
    if (busy || saving) return;
    photoError = null;
    // "Processing" shows only once the camera/picker returned a file; every way out ends idle.
    const r = await acquirePhoto({ getPath, stage: stagePath }, (p) => (phase = p));
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
  function submit(e: Event) {
    e.preventDefault();
    if (!saving && canSave(photos, busy)) onsave(typed, description, toSave(photos));
  }
</script>

<div class="backdrop" role="presentation">
  <!-- Escape is marked handled so the plan screen's Back does not act on the same key press. -->
  <div class="dialog sheet" role="dialog" aria-modal="true" tabindex="-1" onkeydown={(e) => { if (e.key === "Escape") { e.preventDefault(); if (!saving) oncancel(); } }}>
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
            <img src={convertFileSrc(item.path)} alt={t("photos.thumb", { n: i + 1 })} />
            <button type="button" class="x" aria-label={t("photos.remove")} title={t("photos.remove")} onclick={() => removeItem(item.key, item.staged)} disabled={saving}>×</button>
          </li>
        {/each}
      </ul>
      <div class="add-row">
        {#if camera}
          <button type="button" onclick={() => add(capturePath)} disabled={busy || saving}>{t("photos.camera")}</button>
        {/if}
        <button type="button" onclick={() => add(pickPath)} disabled={busy || saving}>{t("photos.gallery")}</button>
        {#if phase === "processing"}<span class="muted" aria-live="polite">{t("photos.processing")}</span>{/if}
      </div>
      {#if !busy && shown(photos).length === 0}
        <p class="hint">{t("photos.hint")}</p>
      {/if}
    </div>
    <div class="actions">
      <button type="button" onclick={oncancel} disabled={saving}>{t("common.cancel")}</button>
      <button type="submit" class="primary" disabled={saving || !canSave(photos, busy)}>{saving ? t("common.saving") : t("common.save")}</button>
    </div>
  </form>
  </div>
</div>

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
  .thumbs img { width: 100%; height: 100%; object-fit: cover; border-radius: 6px; border: 1px solid #ccc; background: #eee; display: block; }
  /* 44 px touch target, drawn as a smaller badge in the corner. */
  .x { position: absolute; top: -10px; right: -10px; width: 44px; height: 44px; min-height: 0; padding: 0; border: none; background: none; font-size: 0; }
  .x::before { content: "×"; position: absolute; top: 10px; right: 10px; width: 24px; height: 24px; line-height: 22px; font-size: 18px; text-align: center; border-radius: 12px; background: #b3261e; color: #fff; }
  .add-row { display: flex; flex-wrap: wrap; align-items: center; gap: 0.5rem; margin-top: 0.5rem; }
  .hint { margin: 0.4rem 0 0; color: #b3261e; font-size: 0.9rem; }
  /* Phones: the sheet fills the screen so the keyboard leaves room for the description. */
  @media (max-width: 599px) {
    :global(.backdrop:has(.sheet)) { align-items: stretch; padding: 0; }
    .sheet { width: 100%; border-radius: 0; padding-bottom: calc(1rem + env(safe-area-inset-bottom)); }
  }
</style>
