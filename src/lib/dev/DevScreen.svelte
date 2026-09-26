<script lang="ts">
  // Sprint 0 spikes still in use (camera, report engines): dev builds, or release builds with
  // VITE_SPIKES=1. The viewer spike was replaced by the plan screen (D-014).
  import CameraSpike from "./CameraSpike.svelte";
  import ReportSpike from "./ReportSpike.svelte";
  type Tab = "camera" | "report";
  let tab = $state<Tab>("camera");
</script>

<div class="dev">
  <nav>
    <button class:active={tab === "camera"} onclick={() => (tab = "camera")}>Camera</button>
    <button class:active={tab === "report"} onclick={() => (tab = "report")}>Report</button>
  </nav>
  <div class="body">
    {#if tab === "camera"}<CameraSpike />{:else}<ReportSpike />{/if}
  </div>
</div>

<style>
  .dev { display: flex; flex-direction: column; height: 100%; }
  nav { display: flex; gap: 0.4rem; padding: 0.5rem 0.75rem; background: #e9eef6; }
  nav button.active { background: #143c78; color: #fff; }
  .body { flex: 1; min-height: 0; position: relative; }
</style>
