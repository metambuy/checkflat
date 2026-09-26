<script lang="ts">
  import { onMount } from "svelte";
  import { loadLang, t } from "./lib/i18n.svelte";
  import { onBackButtonPress } from "@tauri-apps/api/app";
  import type { PluginListener } from "@tauri-apps/api/core";
  import { back, go, screen } from "./lib/nav.svelte";
  import LanguageSwitch from "./lib/components/LanguageSwitch.svelte";
  import ProjectsScreen from "./lib/screens/ProjectsScreen.svelte";
  import ProjectScreen from "./lib/screens/ProjectScreen.svelte";
  import PlanScreen from "./lib/screens/PlanScreen.svelte";
  // Dev screen only in spike builds (see lib/devlog.ts). Kept inline so the bundler folds it and a
  // normal release bundle drops the Dev chunk (an imported constant is not folded).
  const SPIKES = import.meta.env.DEV || import.meta.env.VITE_SPIKES === "1";
  const devScreen = SPIKES ? import("./lib/dev/DevScreen.svelte") : null;

  let ready = $state(false);
  onMount(async () => {
    await loadLang();
    ready = true;
  });
  const s = $derived(screen());

  // Android system back: listen only below the root. With a listener registered Tauri never lets
  // Back leave the app, so on the projects list the platform default (close/background) applies.
  let backListener: Promise<PluginListener> | null = null;
  $effect(() => {
    const atRoot = s.name === "projects";
    if (!atRoot && !backListener) {
      backListener = onBackButtonPress(() => back()).catch(() => null as unknown as PluginListener); // desktop: no-op
    } else if (atRoot && backListener) {
      const l = backListener;
      backListener = null;
      void l.then((x) => x?.unregister());
    }
  });
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
  {:else if s.name === "plan"}
    {#key s.planId}<PlanScreen projectId={s.projectId} planId={s.planId} />{/key}
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
