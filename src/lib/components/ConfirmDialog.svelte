<script lang="ts">
  import { t } from "../i18n.svelte";
  let {
    open,
    message,
    confirmLabel = t("common.confirm"),
    danger = false,
    onconfirm,
    oncancel,
  }: { open: boolean; message: string; confirmLabel?: string; danger?: boolean; onconfirm: () => void; oncancel: () => void } = $props();
</script>

{#if open}
  <!-- Click on the backdrop (not the dialog) cancels; Escape cancels. -->
  <div class="backdrop" role="presentation" onclick={(e) => { if (e.target === e.currentTarget) oncancel(); }}>
    <div class="dialog" role="dialog" aria-modal="true" tabindex="-1" onkeydown={(e) => { if (e.key === "Escape") oncancel(); }}>
      <p>{message}</p>
      <div class="actions">
        <button onclick={oncancel}>{t("common.cancel")}</button>
        <button class:danger onclick={onconfirm}>{confirmLabel}</button>
      </div>
    </div>
  </div>
{/if}

<style>
  .backdrop { position: fixed; inset: 0; background: rgba(0, 0, 0, 0.45); display: flex; align-items: center; justify-content: center; padding: 16px; z-index: 10; }
  .dialog { background: #fff; border-radius: 10px; padding: 1rem; width: min(100%, 420px); box-shadow: 0 8px 30px rgba(0,0,0,0.3); }
  .actions { display: flex; justify-content: flex-end; gap: 0.5rem; margin-top: 1rem; }
  button { min-height: 44px; }
  .danger { background: #b3261e; color: #fff; border-color: #b3261e; }
</style>
