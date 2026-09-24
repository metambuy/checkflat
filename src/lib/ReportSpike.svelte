<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";

  interface ReportResult { engine: string; path: string; bytes: number; elapsed_ms: number; base64: string }
  let results = $state<Record<string, ReportResult | string>>({});
  let busy = $state<string | null>(null);

  async function run(engine: string) {
    busy = engine;
    try {
      results[engine] = await invoke<ReportResult>("generate_report", { engine });
    } catch (e) {
      results[engine] = `error: ${e}`;
    } finally {
      busy = null;
    }
  }

  function download(r: ReportResult) {
    const a = document.createElement("a");
    a.href = `data:application/pdf;base64,${r.base64}`;
    a.download = `report-${r.engine}.pdf`;
    a.click();
  }
</script>

<section>
  <h2>Spike C — report PDF from Rust</h2>
  <p class="muted">Same content (title, JPEG, PT paragraph) through both engines. Time is the engine call only.</p>
  <div class="row">
    <button onclick={() => run("typst")} disabled={busy !== null}>Generate with Typst</button>
    <button onclick={() => run("printpdf")} disabled={busy !== null}>Generate with printpdf</button>
  </div>
  {#if busy}<p>rendering with {busy}…</p>{/if}
  {#each Object.entries(results) as [engine, r] (engine)}
    <div class="card">
      <strong>{engine}</strong>
      {#if typeof r === "string"}
        <pre>{r}</pre>
      {:else}
        <pre>{r.bytes} bytes · {r.elapsed_ms} ms
{r.path}</pre>
        <button onclick={() => download(r)}>Open / download PDF</button>
      {/if}
    </div>
  {/each}
</section>

<style>
  section { padding: 0.75rem; overflow: auto; height: 100%; }
  .row { display: flex; gap: 0.5rem; flex-wrap: wrap; }
  .card { margin-top: 0.75rem; padding: 0.5rem; background: #fff; border: 1px solid #ddd; border-radius: 6px; }
</style>
