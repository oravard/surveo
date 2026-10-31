<script lang="ts">
  // Indicateur de séance (SPEC.md, REJ-4, AGENT-2) : toujours visible
  // pendant la séance ; l'élève peut le réduire ou le déplacer, pas le
  // fermer.
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import Icon from '$shared/Icon.svelte';
  import { hhmm } from '$shared/time';

  type SessionView = { teacher: string; ends_at_ms: number };

  let session: SessionView | null = $state(null);
  let compact = $state(false);

  invoke<SessionView | null>('indicator_state').then((s) => (session = s));
  listen<SessionView | null>('session', (e) => (session = e.payload));

  function toggle() {
    compact = !compact;
    invoke('indicator_compact', { compact });
  }
</script>

<div class="bar" data-tauri-drag-region role="status">
  <span class="dot" aria-hidden="true"></span>
  {#if session}
    {#if compact}
      <span class="text" data-tauri-drag-region>Séance · <span class="mono">{hhmm(session.ends_at_ms)}</span></span>
    {:else}
      <span class="text" data-tauri-drag-region>
        Ce poste est relié à la séance de <b>{session.teacher}</b> jusqu'à
        <span class="mono">{hhmm(session.ends_at_ms)}</span>.
      </span>
    {/if}
  {/if}
  <button type="button" onclick={toggle} aria-label={compact ? 'Agrandir l’indicateur' : 'Réduire l’indicateur'} title={compact ? 'Agrandir' : 'Réduire'}>
    <Icon name={compact ? 'plus' : 'minus'} size={16} />
  </button>
</div>

<style>
  :global(body) { background: transparent; overflow: hidden; }
  .bar {
    height: 100vh; display: flex; align-items: center; gap: 9px; padding: 0 6px 0 14px;
    background: var(--bar); color: var(--bar-ink); border-radius: 10px; font-size: 14px;
    cursor: move; user-select: none; white-space: nowrap; overflow: hidden;
  }
  .dot { width: 9px; height: 9px; border-radius: 50%; background: #3fb950; flex: none; }
  .text { flex: 1; overflow: hidden; text-overflow: ellipsis; }
  button {
    border: 0; background: var(--bar-2); color: var(--bar-ink); border-radius: 7px; width: 30px; height: 30px;
    display: inline-flex; align-items: center; justify-content: center; cursor: pointer; flex: none;
  }
</style>
