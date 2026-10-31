<script lang="ts">
  // Indicateur de séance (SPEC.md, REJ-4, AGENT-2) : toujours visible
  // pendant la séance ; l'élève peut le réduire ou le déplacer, pas le
  // fermer. Il énonce les restrictions en cours (MAIT-1) et le mode
  // examen (EXAM-1), et ouvre le panneau élève (AGENT-3).
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import Icon from '$shared/Icon.svelte';
  import { hhmm } from '$shared/time';

  type SessionView = { teacher: string; ends_at_ms: number };
  type RestrictionsView = { exam_ends_at_ms: number | null; items: string[] };

  let session: SessionView | null = $state(null);
  let restrictions: RestrictionsView = $state({ exam_ends_at_ms: null, items: [] });
  let compact = $state(false);
  const exam = $derived(restrictions.exam_ends_at_ms);

  invoke<{ session: SessionView | null; restrictions: RestrictionsView }>('indicator_state').then((s) => {
    session = s.session;
    restrictions = s.restrictions;
  });
  listen<SessionView | null>('session', (e) => (session = e.payload));
  listen<RestrictionsView>('restrictions', (e) => (restrictions = e.payload));

  function toggle() {
    compact = !compact;
    invoke('indicator_compact', { compact });
  }
</script>

<div class="bar" class:exam={exam !== null} data-tauri-drag-region role="status">
  <div class="line" data-tauri-drag-region>
    <span class="dot" aria-hidden="true"></span>
    {#if session}
      {#if compact}
        <span class="text" data-tauri-drag-region>
          {exam !== null ? 'Examen' : 'Séance'} · <span class="mono">{hhmm(exam ?? session.ends_at_ms)}</span>
          {#if restrictions.items.length}<Icon name="lock" size={13} />{/if}
        </span>
      {:else if exam !== null}
        <span class="text" data-tauri-drag-region>
          <Icon name="exam" size={15} /> Séance d'examen en cours jusqu'à <span class="mono">{hhmm(exam)}</span>.
        </span>
      {:else}
        <span class="text" data-tauri-drag-region>
          Ce poste est relié à la séance de <b>{session.teacher}</b> jusqu'à
          <span class="mono">{hhmm(session.ends_at_ms)}</span>.
        </span>
      {/if}
    {/if}
    <button type="button" class="panel" onclick={() => invoke('show_panel')} title="TP, fichiers reçus, sondage">
      {#if compact}<Icon name="list" size={16} /><span class="sr">Ouvrir le panneau Surveo</span>{:else}Surveo{/if}
    </button>
    <button type="button" onclick={toggle} aria-label={compact ? 'Agrandir l’indicateur' : 'Réduire l’indicateur'} title={compact ? 'Agrandir' : 'Réduire'}>
      <Icon name={compact ? 'plus' : 'minus'} size={16} />
    </button>
  </div>
  {#if !compact && restrictions.items.length}
    <div class="items" data-tauri-drag-region>
      <Icon name="lock" size={13} />
      <span data-tauri-drag-region>{restrictions.items.join(' · ')}</span>
    </div>
  {/if}
</div>

<style>
  :global(body) { background: transparent; overflow: hidden; }
  .bar {
    height: 100vh; display: flex; flex-direction: column; justify-content: center; gap: 2px; padding: 0 6px 0 14px;
    background: var(--bar); color: var(--bar-ink); border-radius: 10px; font-size: 14px;
    cursor: move; user-select: none; white-space: nowrap; overflow: hidden;
  }
  .bar.exam { box-shadow: inset 0 0 0 2px #bc8cff; }
  .line { display: flex; align-items: center; gap: 9px; min-height: 32px; }
  .dot { width: 9px; height: 9px; border-radius: 50%; background: #3fb950; flex: none; }
  .text { flex: 1; overflow: hidden; text-overflow: ellipsis; }
  .text :global(.ic) { vertical-align: -2px; }
  .items { display: flex; align-items: center; gap: 6px; font-size: 12.5px; color: var(--bar-muted); padding: 0 0 4px 18px; overflow: hidden; }
  .items span { overflow: hidden; text-overflow: ellipsis; }
  button.panel { width: auto; padding: 0 10px; font-weight: 700; font-size: 13.5px; }
  button {
    border: 0; background: var(--bar-2); color: var(--bar-ink); border-radius: 7px; width: 30px; height: 30px;
    display: inline-flex; align-items: center; justify-content: center; cursor: pointer; flex: none;
  }
</style>
