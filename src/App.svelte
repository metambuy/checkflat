<script lang="ts">
  import { onMount } from "svelte";
  import { loadLang, t } from "./lib/i18n.svelte";
  import { go, screen } from "./lib/nav.svelte";
  import LanguageSwitch from "./lib/components/LanguageSwitch.svelte";
  import ProjectsScreen from "./lib/screens/ProjectsScreen.svelte";
  import ProjectScreen from "./lib/screens/ProjectScreen.svelte";
  import DevScreen from "./lib/dev/DevScreen.svelte";

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
  {#if import.meta.env.DEV}
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
  {:else}
    <DevScreen />
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
