<script lang="ts">
  // « Sélectionner, puis agir » (SPEC.md, SEL-1 à SEL-3) : sans
  // sélection, l'action porte sur toute la salle, et la barre le dit. Les
  // actions courantes sont visibles, les autres sous « Plus ».
  import Icon from '$shared/Icon.svelte';
  import { ui, selection, call, targets, type PopKind } from './store.svelte';
  import PausePop from './PausePop.svelte';
  import WebPop from './WebPop.svelte';
  import AppsPop from './AppsPop.svelte';
  import OpenAppsPop from './OpenAppsPop.svelte';
  import ExamPop from './ExamPop.svelte';
  import { outside } from './outside';

  const room = $derived(ui.view!.room!);
  const session = $derived(ui.view!.session && ui.view!.session.room_id === room.id ? ui.view!.session : null);
  const count = $derived(selection.size);
  const online = $derived(room.stations.filter((s) => s.state !== 'off').length);
  const t = $derived(targets());
  const scope = $derived(t.all ? `toute la salle (${t.stations.length} poste${t.stations.length > 1 ? 's' : ''})` : `${count} poste${count > 1 ? 's' : ''}`);
  /** Tous les postes visés sont en pause : le bouton les reprend. */
  const allPaused = $derived(t.stations.length > 0 && t.stations.every((s) => s.pause !== null));

  const pop = $derived(ui.pop);
  const toggle = (k: PopKind) => (ui.pop = ui.pop === k ? null : k);
  const close = () => (ui.pop = null);
  /** Le seul poste sélectionné, s'il est dans la séance (APP-1). */
  const one = $derived(t.stations.length === 1 && !t.all && t.stations[0].state === 'in_session' ? t.stations[0] : null);

  function pause() {
    if (allPaused) {
      ui.pop = null;
      call('set_pause', { members: t.members, message: null });
    } else toggle('pause');
  }

  function identify() {
    ui.pop = null;
    call('identify', { members: [...selection] });
  }
</script>

<div class="actionbar">
  <div class="target">
    {#if count === 0}
      <b>Toute la salle</b> ({room.stations.length} poste{room.stations.length > 1 ? 's' : ''}, {online} allumé{online > 1 ? 's' : ''})
    {:else}
      <b>{count} poste{count > 1 ? 's' : ''} sélectionné{count > 1 ? 's' : ''}</b>
      <button type="button" class="link" onclick={() => selection.clear()}>Tout désélectionner</button>
    {/if}
    {#if count === 0 && room.stations.length > 0}
      <button type="button" class="link" onclick={() => room.stations.forEach((s) => selection.add(s.member))}>Tout sélectionner</button>
    {/if}
  </div>
  <div class="actions">
    {#if session}
      <button class="bbtn" data-pop-toggle onclick={pause} aria-expanded={pop === 'pause'}
        title={allPaused ? 'Reprendre l’écran des postes' : 'Mettre l’écran des postes en pause'}>
        <Icon name={allPaused ? 'play' : 'pause'} />{allPaused ? 'Reprendre' : 'Pause'}
      </button>
    {/if}
    <button class="bbtn" data-pop-toggle onclick={() => toggle('more')} aria-expanded={pop === 'more'}><Icon name="more" />Plus</button>
  </div>
  {#if pop === 'pause'}
    <PausePop members={t.members} {scope} onclose={close} />
  {:else if pop === 'web'}
    <WebPop members={t.members} stations={t.stations} {scope} onclose={close} />
  {:else if pop === 'apps'}
    <AppsPop members={t.members} stations={t.stations} {scope} onclose={close} />
  {:else if pop === 'open-apps' && one}
    <OpenAppsPop station={one} onclose={close} />
  {:else if pop === 'exam' && session}
    <ExamPop onclose={close} />
  {:else if pop === 'more'}
    <div class="menu" role="menu" use:outside={close} onkeydown={(e) => e.key === 'Escape' && close()} tabindex="-1">
      {#if session}
        <button class="opt" role="menuitem" onclick={() => (ui.pop = 'web')}><Icon name="globe" /><span>Filtrage web…<small>Liste d'autorisation, de blocage, ou tout bloquer</small></span></button>
        <button class="opt" role="menuitem" onclick={() => (ui.pop = 'apps')}><Icon name="appx" /><span>Applications interdites…<small>Fermées dès qu'elles s'ouvrent, jusqu'à la fin de la séance</small></span></button>
        <button class="opt" role="menuitem" onclick={() => (ui.pop = 'open-apps')} disabled={!one}>
          <Icon name="appx" /><span>Applications ouvertes…<small>{one ? `Voir et fermer les applications du poste ${one.number}` : 'Sélectionnez un seul poste de la séance'}</small></span>
        </button>
        <button class="opt" role="menuitem" onclick={() => (ui.pop = 'exam')}>
          <Icon name="exam" /><span>{session.exam ? 'Mode examen…' : 'Passer en mode examen…'}<small>Pour toute la salle : sites autorisés seulement, heure de fin</small></span>
        </button>
      {/if}
      <button class="opt" role="menuitem" onclick={identify} disabled={room.stations.length === 0}><Icon name="target" /><span>Identifier les postes<small>Afficher sur chaque poste le numéro de sa tuile</small></span></button>
    </div>
  {/if}
</div>

<style>
  .actionbar {
    position: relative; background: var(--bar); color: var(--bar-ink); border-radius: 12px; padding: 8px 10px; display: flex; flex-wrap: wrap;
    align-items: center; gap: 8px 14px; box-shadow: var(--shadow);
  }
  .target { font-size: 14px; display: flex; gap: 10px; align-items: baseline; flex-wrap: wrap; padding-left: 4px; }
  .link { background: transparent; border: 0; color: var(--bar-muted); text-decoration: underline; cursor: pointer; font-size: 13px; padding: 0; }
  .actions { display: flex; gap: 6px; margin-left: auto; flex-wrap: wrap; }
  .bbtn {
    background: var(--bar-2); border: 1px solid transparent; color: var(--bar-ink); border-radius: 8px; padding: 7px 12px; font-weight: 700;
    cursor: pointer; display: inline-flex; align-items: center; gap: 6px;
  }
  .bbtn[aria-expanded='true'] { border-color: var(--bar-ink); }
  .bbtn:disabled { opacity: 0.5; cursor: default; }
  .menu {
    position: absolute; bottom: calc(100% + 8px); right: 10px; width: min(360px, calc(100vw - 32px)); background: var(--surface);
    color: var(--ink); border: 1px solid var(--line); border-radius: 12px; box-shadow: 0 12px 40px rgba(0, 0, 0, 0.22); padding: 6px;
    display: flex; flex-direction: column; gap: 4px; z-index: 30;
  }
  .opt {
    display: flex; align-items: center; gap: 10px; padding: 8px 10px; border: 0; border-radius: 8px; background: transparent;
    cursor: pointer; text-align: left; width: 100%; font-size: 14px;
  }
  .opt:hover:not(:disabled) { background: var(--surface-2); }
  .opt:disabled { opacity: 0.5; cursor: default; }
  .opt :global(.ic) { color: var(--muted); }
  .opt small { display: block; color: var(--muted); font-size: 12.5px; }
</style>
