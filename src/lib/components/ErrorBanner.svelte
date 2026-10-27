<script lang="ts">
  import type { AppError } from "../api";
  import { errorText } from "../errors";
  import { t } from "../i18n.svelte";
  let { error, ondismiss }: { error: AppError | null; ondismiss: () => void } = $props();
</script>

{#if error}
  <div class="banner" role="alert">
    <span>{errorText(error, t)}</span>
    <button onclick={ondismiss} aria-label="dismiss">×</button>
  </div>
{/if}

<style>
  .banner { display: flex; justify-content: space-between; gap: 0.5rem; align-items: center; background: #fde7e9; color: #7a1c1c; border: 1px solid #f5b5b9; border-radius: 8px; padding: 0.5rem 0.75rem; margin: 0.5rem 0; }
  button { min-height: 36px; min-width: 36px; }
</style>
