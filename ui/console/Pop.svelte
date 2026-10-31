<script lang="ts">
  // Fenêtre d'une action, ouverte au-dessus de la barre d'actions (jamais
  // de fenêtre séparée, PANNEAU-3). Échap ou un clic ailleurs la ferme.
  import type { Snippet } from 'svelte';
  import { outside } from './outside';

  let { title, sub, onclose, children }: { title: string; sub?: string; onclose: () => void; children: Snippet } = $props();
  let box: HTMLDivElement;

  $effect(() => {
    box.querySelector<HTMLElement>('textarea, input:not([type=radio]), select, button.primary')?.focus();
  });
</script>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<div class="pop" role="dialog" aria-label={title} bind:this={box} use:outside={onclose} tabindex="-1" onkeydown={(e) => e.key === 'Escape' && (e.stopPropagation(), onclose())}>
  <h3>{title}</h3>
  {#if sub}<p class="sub">{sub}</p>{/if}
  {@render children()}
</div>

<style>
  .pop {
    position: absolute; bottom: calc(100% + 8px); right: 10px; width: min(440px, calc(100vw - 32px)); background: var(--surface);
    color: var(--ink); border: 1px solid var(--line); border-radius: 12px; box-shadow: 0 12px 40px rgba(0, 0, 0, 0.22); padding: 12px;
    display: flex; flex-direction: column; gap: 10px; z-index: 30; max-height: min(72vh, 600px); overflow: auto;
  }
  h3 { margin: 0; font-size: 15px; }
  .sub { font-size: 13px; color: var(--muted); margin: 0; }
  .pop :global(.row) { display: flex; flex-wrap: wrap; gap: 6px; justify-content: flex-end; }
  .pop :global(.opt) {
    display: flex; align-items: center; gap: 10px; padding: 8px 10px; border: 1px solid var(--line); border-radius: 9px;
    background: var(--surface); cursor: pointer; text-align: left; width: 100%; font-size: 14px;
  }
  .pop :global(.opt:hover) { border-color: var(--muted); }
  .pop :global(.opt .ic) { color: var(--muted); }
  .pop :global(.opt small) { display: block; color: var(--muted); font-size: 12.5px; }
  .pop :global(.radio) { display: flex; align-items: flex-start; gap: 8px; padding: 7px 9px; border: 1px solid var(--line); border-radius: 9px; cursor: pointer; font-size: 14px; }
  .pop :global(.radio input) { margin-top: 3px; accent-color: var(--accent); }
  .pop :global(.radio small) { display: block; color: var(--muted); font-size: 12.5px; }
  .pop :global(.chips) { display: flex; flex-wrap: wrap; gap: 6px; }
  .pop :global(.chips button) {
    border: 1px solid var(--line); background: var(--surface-2); border-radius: 99px; padding: 4px 10px; font-size: 13px; cursor: pointer;
  }
  .pop :global(textarea) {
    border: 1px solid var(--line); border-radius: 8px; padding: 7px 10px; background: var(--surface); min-height: 4.5em; resize: vertical;
    font: inherit; color: inherit;
  }
</style>
