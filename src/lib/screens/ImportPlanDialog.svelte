<script lang="ts">
  // pick file -> stage_plan_source (reads once, inspects, proposes a title) -> editable title -> import_plan
  import { open } from "@tauri-apps/plugin-dialog";
  import { api, asAppError, ptToMm, type AppError, type Plan, type StagedPlan } from "../api";
  import { t } from "../i18n.svelte";
  import ErrorBanner from "../components/ErrorBanner.svelte";

  let { projectId, onimported, onclose }: { projectId: string; onimported: (p: Plan) => void; onclose: () => void } = $props();

  let phase = $state<"picking" | "inspecting" | "title" | "importing">("picking");
  let staged = $state<StagedPlan | null>(null);
  let title = $state("");
  let error = $state<AppError | null>(null);

  async function pick() {
    error = null;
    phase = "picking";
    let source: string | null = null;
    try {
      source = await open({ multiple: false, directory: false, filters: [{ name: "PDF", extensions: ["pdf"] }] });
    } catch (e) {
      error = asAppError(e);
      return;
    }
    if (!source) { onclose(); return; }
    phase = "inspecting";
    try {
      staged = await api.stagePlanSource(source);
      title = staged.defaultTitle;
      phase = "title";
    } catch (e) {
      error = asAppError(e);
      phase = "picking";
    }
  }

  async function confirm() {
    if (!staged) return;
    phase = "importing";
    try {
      const plan = await api.importPlan(projectId, staged.token, title);
      onimported(plan);
    } catch (e) {
      error = asAppError(e);
      phase = "title";
    }
  }

  async function cancel() {
    if (staged) { try { await api.discardStagedPlan(staged.token); } catch { /* sweep will clean it */ } }
    onclose();
  }

  $effect(() => { void pick(); });
</script>

<div class="backdrop" role="presentation">
  <div class="dialog" role="dialog" aria-modal="true">
    <h2>{t("project.import_plan")}</h2>
    <ErrorBanner {error} ondismiss={() => (error = null)} />
    {#if phase === "picking"}
      <p class="muted">{t("import.picking")}</p>
      <div class="actions"><button onclick={cancel}>{t("common.cancel")}</button><button class="primary" onclick={pick}>{t("import.picking")}</button></div>
    {:else if phase === "inspecting"}
      <p class="muted">{t("import.inspecting")}</p>
    {:else if staged}
      <p class="muted">{staged.displayName ?? ""} · {t("plan.size", { width: ptToMm(staged.info.widthPt), height: ptToMm(staged.info.heightPt) })}</p>
      <form onsubmit={(e) => { e.preventDefault(); confirm(); }}>
        <label>{t("import.title_label")}<input bind:value={title} required autocomplete="off" /></label>
        <div class="actions">
          <button type="button" onclick={cancel} disabled={phase === "importing"}>{t("common.cancel")}</button>
          <button type="submit" class="primary" disabled={!title.trim() || phase === "importing"}>{t("import.confirm")}</button>
        </div>
      </form>
    {/if}
  </div>
</div>

<style>
  .backdrop { position: fixed; inset: 0; background: rgba(0, 0, 0, 0.45); display: flex; align-items: center; justify-content: center; padding: 16px; z-index: 10; }
  .dialog { background: #fff; border-radius: 10px; padding: 1rem; width: min(100%, 460px); box-shadow: 0 8px 30px rgba(0,0,0,0.3); }
  h2 { margin: 0 0 0.5rem; font-size: 1.1rem; }
</style>
