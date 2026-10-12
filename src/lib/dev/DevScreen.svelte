<script lang="ts">
  // Dev builds, or release builds with VITE_SPIKES=1: the Sprint 0 camera spike (still used to
  // try the plugin) and the Sprint 5 seed tool. The viewer spike was replaced by the plan screen
  // (D-014), the report spike by the real report (D-024).
  import CameraSpike from "./CameraSpike.svelte";
  import SeedScreen from "./SeedScreen.svelte";
  type Tab = "camera" | "seed";
  let tab = $state<Tab>("camera");
</script>

<div class="dev">
  <nav>
    <button class:active={tab === "camera"} onclick={() => (tab = "camera")}>Camera</button>
    <button class:active={tab === "seed"} onclick={() => (tab = "seed")}>Seed</button>
  </nav>
  <div class="body">
    {#if tab === "camera"}<CameraSpike />{:else}<SeedScreen />{/if}
  </div>
</div>

<style>
  .dev { display: flex; flex-direction: column; height: 100%; }
  nav { display: flex; gap: 0.4rem; padding: 0.5rem 0.75rem; background: #e9eef6; }
  nav button.active { background: #143c78; color: #fff; }
  .body { flex: 1; min-height: 0; position: relative; }
</style>
