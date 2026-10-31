<script lang="ts">
  // Sondage au premier plan (SPEC.md, SONDAGE-2, AGENT-4) : il reste tant
  // que l'élève n'a pas répondu ; « Plus tard » le range dans le panneau
  // élève.
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import Icon from '$shared/Icon.svelte';
  import type { PanelView } from '$shared/bindings/PanelView';
  import PollQuestion from './PollQuestion.svelte';

  let v = $state<PanelView | null>(null);
  invoke<PanelView | null>('panel_state').then((p) => (v = p));
  listen<PanelView>('panel', (e) => (v = e.payload));
</script>

{#if v?.poll}
  <div class="card" role="dialog" aria-labelledby="poll-title" data-tauri-drag-region>
    <h1 id="poll-title" data-tauri-drag-region><Icon name="poll" size={18} />Sondage de {v.teacher}</h1>
    {#key v.poll.id}<PollQuestion poll={v.poll} />{/key}
    <div class="later"><button type="button" class="btn ghost" onclick={() => invoke('dismiss_poll')}>Répondre plus tard</button></div>
  </div>
{/if}

<style>
  :global(body) { overflow: auto; }
  .card { min-height: 100vh; padding: 16px 18px; display: flex; flex-direction: column; gap: 10px; border: 2px solid var(--accent); border-radius: 12px; }
  h1 { margin: 0; font-size: 16px; display: flex; align-items: center; gap: 8px; color: var(--accent); }
  .later { margin-top: auto; display: flex; justify-content: flex-end; }
</style>
