<script lang="ts">
  // Debug builds only: seeds N observations (3 photos each) on a plan through the normal save
  // path — the 50-observation fixture for the report measurements (D-024). The command does not
  // exist in release builds (`#[cfg(debug_assertions)]`) unless built locally with the `dev-seed`
  // feature (D-026), so this tab reports an error there.
  import { onMount } from "svelte";
  import { api, type Plan, type ProjectSummary } from "../api";

  let projects = $state<ProjectSummary[]>([]);
  let plans = $state<Plan[]>([]);
  let projectId = $state("");
  let planId = $state("");
  let count = $state(50);
  let annotate = $state(false);
  let busy = $state(false);
  let log = $state<string[]>([]);
  const add = (s: string) => (log = [...log, `${new Date().toLocaleTimeString()} ${s}`]);

  onMount(async () => {
    projects = await api.listProjects();
    projectId = projects[0]?.id ?? "";
  });
  $effect(() => {
    if (!projectId) { plans = []; return; }
    void api.listPlans(projectId).then((p) => { plans = p; planId = p[0]?.id ?? ""; });
  });

  async function seed() {
    busy = true;
    const started = performance.now();
    try {
      const n = await api.devSeedObservations(planId, count, annotate);
      add(`seeded ${n} observations${annotate ? " with annotations" : ""} in ${Math.round(performance.now() - started)} ms`);
    } catch (e) {
      add(`error: ${JSON.stringify(e)}`);
    } finally {
      busy = false;
    }
  }
</script>

<section>
  <h2>Seed observations (debug builds)</h2>
  <p class="muted">Needs one observation with a photo in the project already; its photos are copied.</p>
  <div class="row">
    <select bind:value={projectId} disabled={busy}>
      {#each projects as p (p.id)}<option value={p.id}>{p.name}</option>{/each}
    </select>
    <select bind:value={planId} disabled={busy || plans.length === 0}>
      {#each plans as p (p.id)}<option value={p.id}>{p.title}</option>{/each}
    </select>
    <input type="number" min="1" max="500" bind:value={count} disabled={busy} />
    <label class="check"><input type="checkbox" bind:checked={annotate} disabled={busy} /> with annotations</label>
    <button class="primary" onclick={seed} disabled={busy || !planId}>Seed</button>
  </div>
  {#if busy}<p>seeding…</p>{/if}
  <pre>{log.join("\n")}</pre>
</section>

<style>
  section { padding: 0.75rem; overflow: auto; height: 100%; }
  .row { display: flex; gap: 0.5rem; flex-wrap: wrap; align-items: center; }
  input { width: 5rem; }
  .check { flex-direction: row; align-items: center; gap: 0.3rem; margin: 0; }
  .check input { width: auto; min-height: 0; }
</style>
