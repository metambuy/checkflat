<script lang="ts">
  import { lang, setLang, t } from "../i18n.svelte";
</script>

<div class="switch" role="group" aria-label="language">
  <button class:active={lang() === "en"} onclick={() => setLang("en")}>{t("lang.en")}</button>
  <button class:active={lang() === "pt"} onclick={() => setLang("pt")}>{t("lang.pt")}</button>
</div>

<style>
  .switch { display: inline-flex; gap: 2px; }
  button { min-height: 36px; padding: 0.2rem 0.6rem; background: #1f4f96; color: #fff; border-color: #2f62ac; }
  button.active { background: #fff; color: #143c78; }
</style>
