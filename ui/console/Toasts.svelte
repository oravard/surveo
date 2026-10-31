<script lang="ts">
  // Retours d'action discrets (SPEC.md, RETOUR-1, RETOUR-3, ERGO-7).
  import Icon from '$shared/Icon.svelte';
  import { ui, dismiss } from './store.svelte';
</script>

<div class="toasts" aria-live="polite">
  {#each ui.toasts as t (t.id)}
    <div class="toast" class:warn={t.warning}>
      <Icon name={t.warning ? 'wifioff' : 'check'} />
      <div class="tx">{t.text}{#if t.detail}<small>{t.detail}</small>{/if}</div>
      {#if t.undo}
        <button type="button" onclick={() => { t.undo?.(); dismiss(t.id); }}>Annuler</button>
      {/if}
      <button type="button" class="x" aria-label="Fermer" onclick={() => dismiss(t.id)}><Icon name="x" size={14} /></button>
    </div>
  {/each}
</div>

<style>
  .toasts {
    position: fixed; right: 16px; bottom: 84px; display: flex; flex-direction: column; gap: 8px; z-index: 40;
    max-width: min(460px, calc(100vw - 32px));
  }
  .toast {
    background: var(--bar); color: var(--bar-ink); border-radius: 10px; padding: 10px 12px;
    box-shadow: 0 10px 30px rgba(0, 0, 0, 0.25); display: flex; align-items: flex-start; gap: 10px; font-size: 14px;
  }
  .toast :global(.ic) { color: #3fb950; margin-top: 1px; }
  .toast.warn :global(.ic) { color: #d29922; }
  .tx { flex: 1; }
  small { display: block; color: var(--bar-muted); font-size: 12.5px; margin-top: 2px; }
  button {
    background: transparent; border: 1px solid var(--bar-muted); color: var(--bar-ink); border-radius: 7px;
    padding: 3px 9px; cursor: pointer; font-weight: 700; font-size: 13px;
  }
  button.x { border: 0; padding: 2px; }
  .toast button.x :global(.ic) { color: var(--bar-muted); margin: 0; }
</style>
