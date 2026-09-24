<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import PdfViewer from "./lib/PdfViewer.svelte";
  import CameraSpike from "./lib/CameraSpike.svelte";
  import ReportSpike from "./lib/ReportSpike.svelte";

  type Tab = "plan" | "camera" | "report";
  let tab = $state<Tab>("plan");
  let platform = $state<string>("…");

  invoke<{ os: string; arch: string; debug: boolean; engines: string[] }>("platform_info")
    .then((p) => (platform = `${p.os}/${p.arch}${p.debug ? " debug" : ""} · engines: ${p.engines.join(", ") || "none"}`))
    .catch((e) => (platform = `platform_info failed: ${e}`));
</script>

<header>
  <nav>
    <button class:active={tab === "plan"} onclick={() => (tab = "plan")}>Plan</button>
    <button class:active={tab === "camera"} onclick={() => (tab = "camera")}>Camera</button>
    <button class:active={tab === "report"} onclick={() => (tab = "report")}>Report</button>
  </nav>
  <span class="muted">{platform}</span>
</header>

<main>
  {#if tab === "plan"}
    <PdfViewer />
  {:else if tab === "camera"}
    <CameraSpike />
  {:else}
    <ReportSpike />
  {/if}
</main>

<style>
  header {
    display: flex; align-items: center; gap: 0.75rem; padding: 0.5rem 0.75rem;
    padding-top: max(0.5rem, env(safe-area-inset-top)); background: #143c78; color: #fff;
    flex-wrap: wrap;
  }
  header .muted { color: #cfd8e6; }
  nav { display: flex; gap: 0.4rem; }
  nav button { background: #1f4f96; color: #fff; border-color: #2f62ac; }
  nav button.active { background: #fff; color: #143c78; }
  main { flex: 1; min-height: 0; position: relative; }
</style>
