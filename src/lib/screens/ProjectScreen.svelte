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
  let importing = $state(false);
  let renamingPlan = $state<Plan | null>(null);
  let planTitle = $state("");
  let planToDelete = $state<Plan | null>(null);

  async function refresh() {
    try {
      project = await api.getProject(id);
      addressValue = project.address;
      plans = await api.listPlans(id);
    } catch (e) {
      error = asAppError(e);
    }
  }
  onMount(refresh);

  async function saveName() {
    try { await api.renameProject(id, nameValue); editingName = false; await refresh(); } catch (e) { error = asAppError(e); }
  }
  async function saveAddress() {
    if (!project || addressValue.trim() === project.address) return;
    try { await api.updateProjectAddress(id, addressValue); await refresh(); } catch (e) { error = asAppError(e); }
  }
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
      <label>{t("projects.address")}
        <input bind:value={addressValue} placeholder={t("project.address_placeholder")} onblur={saveAddress} autocomplete="off" />
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
              <div class="grow">
                <strong>{plan.title}</strong>
                <div class="muted">{t("plan.size", { width: ptToMm(plan.widthPt), height: ptToMm(plan.heightPt) })}</div>
              </div>
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
    <ImportPlanDialog projectId={id} onimported={() => { importing = false; refresh(); }} onclose={() => (importing = false)} />
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
