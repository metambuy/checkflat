<script lang="ts">
  import { onMount } from "svelte";
  import { loadLang, t } from "./lib/i18n.svelte";
  import { go, screen } from "./lib/nav.svelte";
  import LanguageSwitch from "./lib/components/LanguageSwitch.svelte";
  import ProjectsScreen from "./lib/screens/ProjectsScreen.svelte";
  import ProjectScreen from "./lib/screens/ProjectScreen.svelte";
  // Spikes (Dev screen): dev builds, or a release build made with VITE_SPIKES=1 (D-006/D-014 device
  // measurements). Both are replaced at build time, so a normal release bundle drops the Dev chunk.
  const SPIKES = import.meta.env.DEV || import.meta.env.VITE_SPIKES === "1";
  const devScreen = SPIKES ? import("./lib/dev/DevScreen.svelte") : null;

  let ready = $state(false);
  onMount(async () => {
    await loadLang();
    ready = true;
  });
  const s = $derived(screen());
</script>

<header>
  <button class="plain brand" onclick={() => go({ name: "projects" })}>{t("app.title")}</button>
  <span class="grow"></span>
  {#if SPIKES}
    <button class="dev" onclick={() => go({ name: "dev" })}>{t("nav.dev")}</button>
  {/if}
  <LanguageSwitch />
</header>

<main>
  {#if !ready}
    <p class="muted container">{t("common.loading")}</p>
  {:else if s.name === "projects"}
    <ProjectsScreen />
  {:else if s.name === "project"}
    {#key s.id}<ProjectScreen id={s.id} />{/key}
  {:else if devScreen}
    {#await devScreen then m}<m.default />{/await}
  {/if}
</main>

<style>
  header {
    display: flex; align-items: center; gap: 0.5rem; padding: 0.5rem 0.75rem;
    padding-top: max(0.5rem, env(safe-area-inset-top)); background: #143c78; color: #fff;
  }
  .brand { color: #fff; font-weight: 700; font-size: 1.1rem; padding: 0.3rem 0.2rem; }
  .dev { background: #1f4f96; color: #fff; border-color: #2f62ac; }
  main { flex: 1; min-height: 0; position: relative; overflow: auto; }
</style>
