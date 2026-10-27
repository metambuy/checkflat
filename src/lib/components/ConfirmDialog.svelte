<script lang="ts">
  import { t } from "../i18n.svelte";
  let {
    open,
    message,
    confirmLabel = t("common.confirm"),
    cancelLabel = t("common.cancel"),
    danger = false,
    onconfirm,
    oncancel,
  }: { open: boolean; message: string; confirmLabel?: string; cancelLabel?: string; danger?: boolean; onconfirm: () => void; oncancel: () => void } = $props();
  let box = $state<HTMLDivElement | null>(null);
  // Focus goes into the dialog when it opens: its Escape handler only hears a key press with the
  // focus inside it, and a dialog opened over an inert screen would otherwise leave the focus nowhere.
  $effect(() => {
    if (open) box?.focus();
  });
</script>

{#if open}
  <!-- Click on the backdrop (not the dialog) cancels; Escape cancels and is marked handled, so a
       screen's own Escape handler (Back) does not act on the same key press. -->
  <div class="backdrop" role="presentation" onclick={(e) => { if (e.target === e.currentTarget) oncancel(); }}>
    <div bind:this={box} class="dialog" role="dialog" aria-modal="true" tabindex="-1" onkeydown={(e) => { if (e.key === "Escape") { e.preventDefault(); oncancel(); } }}>
      <p>{message}</p>
      <div class="actions">
        <button onclick={oncancel}>{cancelLabel}</button>
        <button class:danger onclick={onconfirm}>{confirmLabel}</button>
      </div>
    </div>
  </div>
{/if}

<style>
  .actions { margin-top: 1rem; }
  .danger { background: #b3261e; color: #fff; border-color: #b3261e; }
</style>
