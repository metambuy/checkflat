<script lang="ts">
  import { onMount } from "svelte";
  import { api, asAppError, type AppError, type ProjectSummary } from "../api";
  import { t } from "../i18n.svelte";
  import { go } from "../nav.svelte";
  import ConfirmDialog from "../components/ConfirmDialog.svelte";
  import ErrorBanner from "../components/ErrorBanner.svelte";

  let projects = $state<ProjectSummary[]>([]);
  let loading = $state(true);
  let error = $state<AppError | null>(null);
  let creating = $state(false);
  let newName = $state("");
  let newAddress = $state("");
  let renamingId = $state<string | null>(null);
  let renameValue = $state("");
  let toDelete = $state<ProjectSummary | null>(null);

  async function refresh() {
    try {
      projects = await api.listProjects();
    } catch (e) {
      error = asAppError(e);
    } finally {
      loading = false;
    }
  }
  onMount(refresh);

  async function create() {
    try {
      await api.createProject(newName, newAddress);
      newName = ""; newAddress = ""; creating = false;
      await refresh();
    } catch (e) { error = asAppError(e); }
  }
  async function rename() {
    if (!renamingId) return;
    try {
      await api.renameProject(renamingId, renameValue);
      renamingId = null;
      await refresh();
    } catch (e) { error = asAppError(e); }
  }
  async function remove() {
    if (!toDelete) return;
    try {
      await api.deleteProject(toDelete.id);
      toDelete = null;
      await refresh();
    } catch (e) { error = asAppError(e); toDelete = null; }
  }
  const plansLabel = (n: number) => (n === 1 ? t("projects.plans_count_one") : t("projects.plans_count", { count: n }));
</script>

<section class="container">
  <h1>{t("projects.title")}</h1>
  <ErrorBanner {error} ondismiss={() => (error = null)} />

  {#if creating}
    <form class="card" onsubmit={(e) => { e.preventDefault(); create(); }}>
      <label>{t("projects.name")}<input bind:value={newName} required autocomplete="off" /></label>
      <label>{t("projects.address")}<input bind:value={newAddress} placeholder={t("project.address_placeholder")} autocomplete="off" /></label>
      <div class="actions">
        <button type="button" onclick={() => (creating = false)}>{t("common.cancel")}</button>
        <button type="submit" class="primary" disabled={!newName.trim()}>{t("common.save")}</button>
      </div>
    </form>
  {:else}
    <button class="primary" onclick={() => (creating = true)}>{t("projects.new")}</button>
  {/if}

  {#if loading}
    <p class="muted">{t("common.loading")}</p>
  {:else if projects.length === 0}
    <p class="muted">{t("projects.empty")}</p>
  {:else}
    <ul class="list">
      {#each projects as p (p.id)}
        <li class="card row">
          {#if renamingId === p.id}
            <form class="grow" onsubmit={(e) => { e.preventDefault(); rename(); }}>
              <input bind:value={renameValue} required autocomplete="off" />
              <div class="actions">
                <button type="button" onclick={() => (renamingId = null)}>{t("common.cancel")}</button>
                <button type="submit" class="primary">{t("common.save")}</button>
              </div>
            </form>
          {:else}
            <button class="grow plain" onclick={() => go({ name: "project", id: p.id })}>
              <strong>{p.name}</strong>
              <span class="muted">{p.address || "—"} · {plansLabel(p.planCount)}</span>
            </button>
            <button onclick={() => { renamingId = p.id; renameValue = p.name; }}>{t("common.rename")}</button>
            <button class="danger" onclick={() => (toDelete = p)}>{t("common.delete")}</button>
          {/if}
        </li>
      {/each}
    </ul>
  {/if}

  <ConfirmDialog
    open={toDelete !== null}
    message={toDelete ? t("projects.delete_confirm", { name: toDelete.name, count: toDelete.planCount }) : ""}
    confirmLabel={t("common.delete")}
    danger
    onconfirm={remove}
    oncancel={() => (toDelete = null)}
  />
</section>
