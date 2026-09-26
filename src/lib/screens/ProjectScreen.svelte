<script lang="ts">
  import { onMount } from "svelte";
  import { api, asAppError, ptToMm, type AppError, type Plan, type Project } from "../api";
  import { t } from "../i18n.svelte";
  import { go } from "../nav.svelte";
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
  let addressStatus = $state<"idle" | "saving" | "saved" | "error">("idle");
  let addressTimer: number | undefined;
  let addressChain: Promise<void> = Promise.resolve();
  let importing = $state(false);
  let renamingPlan = $state<Plan | null>(null);
  let planTitle = $state("");
  let planToDelete = $state<Plan | null>(null);

  async function refresh() {
    try {
      project = await api.getProject(id);
      if (addressStatus !== "saving") addressValue = project.address;
      plans = await api.listPlans(id);
    } catch (e) {
      error = asAppError(e);
    }
  }
  onMount(refresh);

  async function saveName() {
    try { await api.renameProject(id, nameValue); editingName = false; await refresh(); } catch (e) { error = asAppError(e); }
  }
  /** Address saves itself: ~800 ms after typing stops, on blur and on Enter. Saves run one at a
   * time; the typed text is never overwritten by a reply, and stays in the field if saving fails. */
  function saveAddress() {
    clearTimeout(addressTimer);
    addressChain = addressChain.then(async () => {
      const value = addressValue.trim();
      if (!project || value === project.address) {
        if (addressStatus === "saving") addressStatus = "saved";
        return;
      }
      addressStatus = "saving";
      try {
        project = await api.updateProjectAddress(id, value);
        addressStatus = addressValue.trim() === project.address ? "saved" : "saving";
        if (addressStatus === "saving") saveAddress(); // typed more while saving
      } catch (e) {
        addressStatus = "error";
        error = asAppError(e);
      }
    });
  }
  function addressInput() {
    addressStatus = "idle";
    clearTimeout(addressTimer);
    addressTimer = window.setTimeout(saveAddress, 800);
  }
  onMount(() => () => { clearTimeout(addressTimer); });
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
        </div>
      {/if}
      <label>
        <span class="row">{t("projects.address")}<span class="status {addressStatus}" aria-live="polite">{addressStatus === "saving" ? t("common.saving") : addressStatus === "saved" ? `✓ ${t("common.saved")}` : addressStatus === "error" ? t("common.save_failed") : ""}</span></span>
        <input
          bind:value={addressValue}
          placeholder={t("project.address_placeholder")}
          oninput={addressInput}
          onblur={saveAddress}
          onkeydown={(e) => { if (e.key === "Enter") { e.preventDefault(); saveAddress(); } }}
          autocomplete="off"
        />
      </label>
    </header>

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
</style>
