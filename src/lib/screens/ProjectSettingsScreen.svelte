<script lang="ts">
  // Project settings (Sprint 3a, D-020): project code, ref template and numbering scope with a
  // live preview of the next ref, plus the fraction list. Code/template/scope are saved
  // together with an explicit Save (the template must validate); fractions save on the spot.
  import { onMount } from "svelte";
  import { api, asAppError, type AppError, type Fraction, type Project, type Scope } from "../api";
  import { t } from "../i18n.svelte";
  import { go } from "../nav.svelte";
  import { livePreview } from "../livePreview";
  import ErrorBanner from "../components/ErrorBanner.svelte";

  let { id }: { id: string } = $props();
  let project = $state<Project | null>(null);
  let fractions = $state<Fraction[]>([]);
  let error = $state<AppError | null>(null);
  let code = $state("");
  let template = $state("");
  let scope = $state<Scope>("project");
  let saving = $state(false);
  let saved = $state(false);
  let preview = $state<string | null>(null);
  let previewError = $state<AppError | null>(null);
  let newFraction = $state("");

  const dirty = $derived(project !== null && (code.trim() !== project.code || template.trim() !== project.refTemplate || scope !== project.seqScope));
  // The preview needs a fraction when numbers run per fraction: the first listed one, else "A".
  const sampleFraction = $derived(scope === "fraction" ? (fractions[0]?.code ?? "A") : "");

  const previewer = livePreview<{ code: string; template: string; scope: Scope; fraction: string }>({
    compute: (s) => api.previewRefSettings(id, s.code, s.template, s.scope, s.fraction),
    show: (text, e) => {
      preview = text;
      previewError = e === null ? null : asAppError(e);
    },
  });
  $effect(() => {
    if (project) previewer.update({ code, template, scope, fraction: sampleFraction });
  });

  async function refresh() {
    try {
      project = await api.getProject(id);
      code = project.code;
      template = project.refTemplate;
      scope = project.seqScope;
      fractions = await api.listFractions(id);
    } catch (e) {
      error = asAppError(e);
    }
  }
  onMount(() => {
    void refresh();
    return () => previewer.cancel();
  });

  async function save(e: Event) {
    e.preventDefault();
    saving = true;
    saved = false;
    error = null;
    try {
      project = await api.updateRefSettings(id, code, template, scope);
      code = project.code;
      template = project.refTemplate;
      scope = project.seqScope;
      saved = true;
    } catch (err) {
      error = asAppError(err);
    } finally {
      saving = false;
    }
  }

  async function addFraction(e: Event) {
    e.preventDefault();
    if (!newFraction.trim()) return;
    error = null;
    try {
      await api.addFraction(id, newFraction);
      newFraction = "";
      fractions = await api.listFractions(id);
    } catch (err) {
      error = asAppError(err);
    }
  }

  async function deleteFraction(f: Fraction) {
    error = null;
    try {
      await api.deleteFraction(f.id);
      fractions = await api.listFractions(id);
    } catch (err) {
      error = asAppError(err);
    }
  }
</script>

<section class="container">
  <button class="plain back" onclick={() => go({ name: "project", id })}>← {t("nav.back")}</button>
  <h1>{t("settings.title")}</h1>
  <ErrorBanner {error} ondismiss={() => (error = null)} />
  {#if project}
    <form class="card" onsubmit={save}>
      <h2>{t("settings.numbering")}</h2>
      <label>
        {t("settings.code")}
        <input bind:value={code} placeholder={t("settings.code_placeholder")} autocomplete="off" autocapitalize="characters" spellcheck="false" maxlength="16" />
      </label>
      <label>
        {t("settings.template")}
        <input bind:value={template} autocomplete="off" autocapitalize="off" spellcheck="false" maxlength="64" required />
        <span class="muted">{t("settings.template_help")}</span>
      </label>
      <fieldset>
        <legend>{t("settings.scope")}</legend>
        <label class="radio"><input type="radio" name="scope" value="project" bind:group={scope} disabled={project.scopeLocked} /> {t("settings.scope_project")}</label>
        <label class="radio"><input type="radio" name="scope" value="fraction" bind:group={scope} disabled={project.scopeLocked} /> {t("settings.scope_fraction")}</label>
        {#if project.scopeLocked}
          <p class="muted">{t("settings.scope_locked", { count: project.observationCount })}</p>
        {/if}
      </fieldset>
      <p class="preview" class:bad={previewError !== null} aria-live="polite">
        {#if previewError}
          {previewError.code === "invalid_template" ? previewError.message.replace(/^invalid ref template: /, "") : previewError.message}
        {:else if preview !== null}
          {t("settings.next_ref", { ref: preview })}
        {/if}
      </p>
      <div class="actions">
        <span class="status" aria-live="polite">{saving ? t("common.saving") : saved && !dirty ? `✓ ${t("common.saved")}` : ""}</span>
        <button type="submit" class="primary" disabled={saving || !dirty || previewError !== null}>{t("common.save")}</button>
      </div>
    </form>

    <div class="card">
      <h2>{t("settings.fractions")}</h2>
      {#if fractions.length === 0}
        <p class="muted">{t("settings.fractions_empty")}</p>
      {:else}
        <ul class="chips">
          {#each fractions as f (f.id)}
            <li class="chip">
              <span>{f.code}</span>
              <button type="button" class="x" onclick={() => deleteFraction(f)} aria-label={t("settings.fraction_delete", { code: f.code })} title={t("settings.fraction_delete", { code: f.code })}>×</button>
            </li>
          {/each}
        </ul>
      {/if}
      <form class="row" onsubmit={addFraction}>
        <input class="grow" bind:value={newFraction} placeholder={t("sheet.fraction_placeholder")} autocomplete="off" autocapitalize="characters" spellcheck="false" maxlength="16" />
        <button type="submit" disabled={!newFraction.trim()}>{t("common.add")}</button>
      </form>
    </div>
  {:else}
    <p class="muted">{t("common.loading")}</p>
  {/if}
</section>

<style>
  fieldset { border: none; padding: 0; margin: 0.4rem 0; }
  legend { font-size: 0.9rem; color: #444; padding: 0; margin-bottom: 0.25rem; }
  .radio { flex-direction: row; align-items: center; gap: 0.5rem; margin: 0.2rem 0; color: inherit; font-size: 1rem; }
  .radio input { min-height: 0; width: 20px; height: 20px; }
  .preview { margin: 0.5rem 0 0; font-weight: 600; color: #143c78; min-height: 1.4em; }
  .preview.bad { color: #b3261e; font-weight: 400; }
  .status { align-self: center; font-size: 12px; color: #2e7d32; }
  .chips { list-style: none; padding: 0; margin: 0 0 0.5rem; display: flex; flex-wrap: wrap; gap: 0.4rem; }
  .chip { display: inline-flex; align-items: center; gap: 0.2rem; background: #e3ebf7; color: #143c78; border: 1px solid #c5d3ea; border-radius: 18px; padding: 0 0.2rem 0 0.8rem; font-weight: 600; }
  .x { min-height: 36px; min-width: 36px; border: none; background: none; color: #143c78; font-size: 18px; padding: 0; }
</style>
