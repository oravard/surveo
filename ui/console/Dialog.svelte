<script lang="ts">
  // Fenêtre de dialogue dans la console (jamais de fenêtre séparée,
  // PANNEAU-3). Échap ferme.
  import type { Snippet } from 'svelte';

  let { title, onclose, children }: { title: string; onclose: () => void; children: Snippet } = $props();
  let box: HTMLDivElement;

  $effect(() => {
    box.querySelector<HTMLElement>('input, select, button.primary')?.focus();
  });
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="backdrop" onkeydown={(e) => e.key === 'Escape' && onclose()} onclick={(e) => e.target === e.currentTarget && onclose()}>
  <div class="dialog" role="dialog" aria-modal="true" aria-label={title} bind:this={box}>
    <h2>{title}</h2>
    {@render children()}
  </div>
</div>

<style>
  .backdrop {
    position: fixed; inset: 0; background: rgba(1, 4, 9, 0.55); display: flex; align-items: flex-start;
    justify-content: center; padding: 12vh 16px 16px; z-index: 60;
  }
  .dialog {
    background: var(--surface); border: 1px solid var(--line); border-radius: 14px; box-shadow: 0 20px 60px rgba(0, 0, 0, 0.3);
    width: min(480px, 100%); padding: 18px; display: flex; flex-direction: column; gap: 12px;
  }
  h2 { margin: 0; font-size: 18px; text-wrap: balance; }
  .dialog :global(p) { margin: 0; color: var(--muted); }
  .dialog :global(.row) { display: flex; gap: 8px; justify-content: flex-end; flex-wrap: wrap; }
</style>
