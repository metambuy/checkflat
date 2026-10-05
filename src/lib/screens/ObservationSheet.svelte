<script lang="ts">
  // Observation sheet (Sprint 3a): fraction (type-to-add from the project's list) + description
  // for a draft pin. Save hands both to the plan screen, which calls create_observation once;
  // nothing is written here. Photos come in Sprint 3b.
  import { onMount } from "svelte";
  import { api, asAppError, type AppError, type Fraction } from "../api";
  import { t } from "../i18n.svelte";
  import { livePreview } from "../livePreview";
  import ErrorBanner from "../components/ErrorBanner.svelte";

  let {
    projectId,
    saving,
    error,
    onsave,
    oncancel,
    ondismiss,
  }: {
    projectId: string;
    saving: boolean;
    /** The failed save, shown here so the user can fix the input and retry. */
    error: AppError | null;
    onsave: (fraction: string, description: string) => void;
    oncancel: () => void;
    ondismiss: () => void;
  } = $props();

  let fractions = $state<Fraction[]>([]);
  let fraction = $state("");
  let description = $state("");
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
    previewer.update(typed);
  });

  onMount(() => {
    void api
      .listFractions(projectId)
      .then((f) => (fractions = f))
      .catch(() => {}); // the list is a convenience; typing still works
    input?.focus();
    return () => previewer.cancel();
  });

  function choose(code: string) {
    fraction = code;
  }
  function submit(e: Event) {
    e.preventDefault();
    if (!saving) onsave(typed, description);
  }
</script>

<div class="backdrop" role="presentation">
  <!-- Escape is marked handled so the plan screen's Back does not act on the same key press. -->
  <div class="dialog sheet" role="dialog" aria-modal="true" tabindex="-1" onkeydown={(e) => { if (e.key === "Escape") { e.preventDefault(); if (!saving) oncancel(); } }}>
  <form onsubmit={submit}>
    <h2>{t("sheet.title")}</h2>
    <ErrorBanner {error} {ondismiss} />
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
    <label>
      {t("sheet.description")}
      <textarea bind:value={description} rows="4"></textarea>
    </label>
    <div class="actions">
      <button type="button" onclick={oncancel} disabled={saving}>{t("common.cancel")}</button>
      <button type="submit" class="primary" disabled={saving}>{saving ? t("common.saving") : t("common.save")}</button>
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
  /* Phones: the sheet fills the screen so the keyboard leaves room for the description. */
  @media (max-width: 599px) {
    :global(.backdrop:has(.sheet)) { align-items: stretch; padding: 0; }
    .sheet { width: 100%; border-radius: 0; padding-bottom: calc(1rem + env(safe-area-inset-bottom)); }
  }
</style>
