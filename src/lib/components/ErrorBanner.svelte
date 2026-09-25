<script lang="ts">
  import type { AppError } from "../api";
  import { t } from "../i18n.svelte";
  let { error, ondismiss }: { error: AppError | null; ondismiss: () => void } = $props();

  function text(e: AppError): string {
    switch (e.code) {
      case "multi_page": return t("import.multi_page", { pages: e.pages ?? "?" });
      case "unreadable_pdf": return t("import.unreadable_pdf");
      case "source_unreadable": return t("import.source_unreadable");
      case "plan_has_observations": return t("plan.delete_blocked", { count: e.count ?? "?" });
      case "validation": return t("error.validation");
      case "not_found": return t("error.not_found");
      case "db": return t("error.db");
      case "io": return t("error.io");
      default: return `${t("error.internal")} (${e.message})`;
    }
  }
</script>

{#if error}
  <div class="banner" role="alert">
    <span>{text(error)}</span>
    <button onclick={ondismiss} aria-label="dismiss">×</button>
  </div>
{/if}

<style>
  .banner { display: flex; justify-content: space-between; gap: 0.5rem; align-items: center; background: #fde7e9; color: #7a1c1c; border: 1px solid #f5b5b9; border-radius: 8px; padding: 0.5rem 0.75rem; margin: 0.5rem 0; }
  button { min-height: 36px; min-width: 36px; }
</style>
