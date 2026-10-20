<script lang="ts">
  import { onMount } from "svelte";
  import { api, asAppError, deviceTimeZone, ptToMm, utcOffsetMin, type AppError, type Plan, type Project, type ReportResult } from "../api";
  import { lang, t, type Lang } from "../i18n.svelte";
  import { go } from "../nav.svelte";
  import { listRoute } from "../list/rows";
  import { addressSaver, type SaveStatus } from "../addressSaver";
  import ConfirmDialog from "../components/ConfirmDialog.svelte";
  import ErrorBanner from "../components/ErrorBanner.svelte";
  import ImportPlanDialog from "./ImportPlanDialog.svelte";

  let { id }: { id: string } = $props();
  let project = $state<Project | null>(null);
  let plans = $state<Plan[]>([]);
  let error = $state<AppError | null>(null);
  let editingName = $state(false);
  let nameValue = $state("");
  let addressValue = $state("");
  let addressStatus = $state<SaveStatus>("idle");
  let importing = $state(false);
  let renamingPlan = $state<Plan | null>(null);
  let planTitle = $state("");
  let planToDelete = $state<Plan | null>(null);
  // Report (Sprint 5, temporary action until share/save in Sprint 6): language chosen here,
  // defaulting to the UI language; the PDF is written to the app data dir and its path shown.
  let reportLang = $state<Lang>(lang());
  let generating = $state(false);
  let report = $state<ReportResult | null>(null);

  async function generateReport() {
    generating = true;
    report = null;
    try {
      report = await api.generateReport(id, reportLang, deviceTimeZone(), utcOffsetMin());
    } catch (e) {
      error = asAppError(e);
    } finally {
      generating = false;
    }
  }

  async function refresh() {
    try {
      project = await api.getProject(id);
      if (address.mayReplace()) addressValue = project.address; // never over unsaved or failed edits
      plans = await api.listPlans(id);
    } catch (e) {
      error = asAppError(e);
    }
  }
  onMount(refresh);

  async function saveName() {
    try { await api.renameProject(id, nameValue); editingName = false; await refresh(); } catch (e) { error = asAppError(e); }
  }
  /** Address saves itself: ~800 ms after typing stops, on blur and on Enter (addressSaver.ts). */
  const address = addressSaver({
    get: () => addressValue,
    stored: () => project?.address ?? null,
    put: async (value) => { project = await api.updateProjectAddress(id, value); },
    status: (s) => (addressStatus = s),
    error: (e) => (error = asAppError(e)),
  });
  // Leaving the screen (Back, Escape, opening a plan) saves an edit still waiting for typing to stop.
  onMount(() => () => { void address.flush(); });
  async function savePlanTitle() {
    if (!renamingPlan) return;
    try { await api.renamePlan(renamingPlan.id, planTitle); renamingPlan = null; await refresh(); } catch (e) { error = asAppError(e); }
  }
  async function deletePlan() {
    if (!planToDelete) return;
    const plan = planToDelete;
    planToDelete = null;
    try { await api.deletePlan(plan.id); await refresh(); } catch (e) { error = asAppError(e); }
  }
</script>

<section class="container">
  <button class="plain back" onclick={() => go({ name: "projects" })}>← {t("nav.back")}</button>
  <ErrorBanner {error} ondismiss={() => (error = null)} />
  {#if project}
    <header class="card">
      {#if editingName}
        <form onsubmit={(e) => { e.preventDefault(); saveName(); }}>
          <input bind:value={nameValue} required autocomplete="off" />
          <div class="actions">
            <button type="button" onclick={() => (editingName = false)}>{t("common.cancel")}</button>
            <button type="submit" class="primary">{t("common.save")}</button>
          </div>
        </form>
      {:else}
        <div class="row">
          <h1 class="grow">{project.name}</h1>
          <button onclick={() => { nameValue = project!.name; editingName = true; }}>{t("common.rename")}</button>
          <button onclick={() => go({ name: "projectSettings", id })}>{t("common.settings")}</button>
        </div>
      {/if}
      <label>
        <span class="row">{t("projects.address")}<span class="status {addressStatus}" aria-live="polite">{addressStatus === "saving" ? t("common.saving") : addressStatus === "saved" ? `✓ ${t("common.saved")}` : addressStatus === "error" ? t("common.save_failed") : ""}</span></span>
        <input
          bind:value={addressValue}
          placeholder={t("project.address_placeholder")}
          oninput={address.input}
          onblur={address.save}
          onkeydown={(e) => { if (e.key === "Enter") { e.preventDefault(); void address.save(); } }}
          autocomplete="off"
        />
      </label>
      <div class="row report">
        <label class="row">
          <span class="muted">{t("report.language")}</span>
          <select bind:value={reportLang} disabled={generating}>
            <option value="en">{t("lang.en")}</option>
            <option value="pt">{t("lang.pt")}</option>
          </select>
        </label>
        <button class="primary" onclick={generateReport} disabled={generating}>{t("report.generate")}</button>
        {#if generating}<span class="muted" aria-live="polite">{t("report.generating")}</span>{/if}
      </div>
      {#if report}
        <p class="muted done" aria-live="polite">
          {t("report.done", { path: report.path, pages: report.pages, kb: Math.round(report.bytes / 1024), ms: report.totalMs })}
          {#if report.stats.skippedPhotos > 0}<br />{t("report.skipped_photos", { count: report.stats.skippedPhotos })}{/if}
          <br /><code>snapshots {report.stats.snapshotMs} ms · photos {report.stats.photoMs} ms · compile {report.compileMs} ms · pdf {report.pdfMs} ms · {report.stats.observations} obs · {report.stats.photos} photos · {report.stats.withoutSnapshot} without snapshot</code>
        </p>
      {/if}
    </header>

    <div class="card row">
      <button class="plain grow" onclick={() => go(listRoute(id))}><strong>{t("observations.title")}</strong></button>
    </div>

    <div class="row">
      <h2 class="grow">{t("project.plans")}</h2>
      <button class="primary" onclick={() => (importing = true)}>{t("project.import_plan")}</button>
    </div>
    {#if plans.length === 0}
      <p class="muted">{t("project.plans_empty")}</p>
    {:else}
      <ul class="list">
        {#each plans as plan (plan.id)}
          <li class="card row">
            {#if renamingPlan?.id === plan.id}
              <form class="grow" onsubmit={(e) => { e.preventDefault(); savePlanTitle(); }}>
                <input bind:value={planTitle} required autocomplete="off" />
                <div class="actions">
                  <button type="button" onclick={() => (renamingPlan = null)}>{t("common.cancel")}</button>
                  <button type="submit" class="primary">{t("common.save")}</button>
                </div>
              </form>
            {:else}
              <button class="plain grow" onclick={() => go({ name: "plan", projectId: id, planId: plan.id })}>
                <strong>{plan.title}</strong>
                <span class="muted">{t("plan.size", { width: ptToMm(plan.widthPt), height: ptToMm(plan.heightPt) })}</span>
              </button>
              <button onclick={() => { renamingPlan = plan; planTitle = plan.title; }}>{t("common.rename")}</button>
              <button class="danger" onclick={() => (planToDelete = plan)}>{t("common.delete")}</button>
            {/if}
          </li>
        {/each}
      </ul>
    {/if}
  {:else}
    <p class="muted">{t("common.loading")}</p>
  {/if}

  {#if importing}
    <ImportPlanDialog projectId={id} onimported={(p) => { importing = false; go({ name: "plan", projectId: id, planId: p.id }); }} onclose={() => (importing = false)} />
  {/if}
  <ConfirmDialog
    open={planToDelete !== null}
    message={planToDelete ? t("plan.delete_confirm", { title: planToDelete.title }) : ""}
    confirmLabel={t("common.delete")}
    danger
    onconfirm={deletePlan}
    oncancel={() => (planToDelete = null)}
  />
</section>

<style>
  .status { font-size: 12px; margin-left: 0.5rem; color: #666; }
  .status.saved { color: #2e7d32; }
  .status.error { color: #b3261e; }
  .report { margin-top: 0.75rem; }
  .done { word-break: break-all; }
  .done code { font-size: 11px; }
</style>
