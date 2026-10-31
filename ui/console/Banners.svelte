<script lang="ts">
  // Bandeaux des restrictions posées sur toute la salle (SPEC.md, ERGO-4) :
  // elles se lisent d'un coup d'œil et se lèvent d'un geste (MAIT-2).
  import Icon from '$shared/Icon.svelte';
  import type { WebPolicy } from '$shared/bindings/WebPolicy';
  import { hhmm } from '$shared/time';
  import { ui, call } from './store.svelte';

  const sitesText = (sites: string[]) =>
    sites.length === 0 ? 'aucun site autorisé' : sites.length <= 3 ? `seuls ${sites.join(', ')}` : `${sites.length} sites autorisés`;

  const session = $derived(ui.view!.session);
  const any = $derived(
    session !== null && (session.exam !== null || session.pause_all !== null || session.web_all !== null || session.apps_all.length > 0),
  );

  function webText(p: WebPolicy): string {
    if (p.t === 'all') return 'tout bloquer';
    const n = p.sites.length;
    const list = n <= 3 ? p.sites.join(', ') : `${p.sites.slice(0, 3).join(', ')}…`;
    return `${p.t === 'allow' ? 'seuls sont accessibles' : 'sont bloqués'} ${list}`;
  }
</script>

{#if session && any}
  <div class="banners">
    {#if session.exam}
      <div class="banner exam">
        <Icon name="exam" /><span class="grow"><b>Mode examen</b> jusqu'à <span class="mono">{hhmm(session.exam.end_ms)}</span> · web : {sitesText(session.exam.sites)}{session.apps_all.length ? ' · applications interdites fermées' : ''}</span>
        <button type="button" class="btn" data-pop-toggle onclick={() => (ui.pop = 'exam')}>Modifier…</button>
        <button type="button" class="btn" onclick={() => call('set_exam', { endMs: null, sites: [] })}>Quitter le mode examen</button>
      </div>
    {/if}
    {#if session.pause_all !== null}
      <div class="banner">
        <Icon name="pause" /><span class="grow"><b>Écrans en pause</b> · « {session.pause_all} »</span>
        <button type="button" class="btn" onclick={() => call('set_pause', { members: [], message: null })}><Icon name="play" />Reprendre</button>
      </div>
    {/if}
    {#if session.web_all !== null}
      <div class="banner">
        <Icon name="globe" /><span class="grow"><b>Filtrage web</b> · {webText(session.web_all)}{session.exam ? ', de nouveau après l’examen' : ''} · cesse au plus tard à <span class="mono">{hhmm(session.end_ms)}</span></span>
        <button type="button" class="btn" onclick={() => call('set_web', { members: [], policy: null })}>Lever le filtrage</button>
      </div>
    {/if}
    {#if session.apps_all.length}
      <div class="banner">
        <Icon name="appx" /><span class="grow"><b>Applications interdites</b> · {session.apps_all.join(', ')}</span>
        <button type="button" class="btn" onclick={() => call('set_banned_apps', { members: [], apps: [] })}>Lever l'interdiction</button>
      </div>
    {/if}
  </div>
{/if}

<style>
  .banners { display: flex; flex-direction: column; gap: 6px; }
  .banner {
    display: flex; flex-wrap: wrap; align-items: center; gap: 6px 12px; padding: 7px 12px; border-radius: 10px; font-size: 14px;
    border: 1px solid var(--line); background: var(--surface);
  }
  .banner :global(.ic) { color: var(--restrict); }
  .banner.exam { background: var(--restrict-soft); border-color: color-mix(in srgb, var(--restrict) 40%, transparent); }
  .banner .btn :global(.ic) { color: inherit; }
  .grow { flex: 1; }
</style>
