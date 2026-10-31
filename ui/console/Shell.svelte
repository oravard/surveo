<script lang="ts">
  // Espaces de la console (SPEC.md, ESPACE-1, ESPACE-2) : Séance (plan de
  // la salle, panneau latéral, barre d'actions) et Préparer (la
  // bibliothèque). Changer d'espace ne ferme pas la séance, qu'un bandeau
  // rappelle (ESPACE-4). Administrer viendra avec le chapitre 6.
  import Icon from '$shared/Icon.svelte';
  import { hhmm } from '$shared/time';
  import { ui, selection, type DialogKind } from './store.svelte';
  import TopBar from './TopBar.svelte';
  import Banners from './Banners.svelte';
  import Plan from './Plan.svelte';
  import Panel from './Panel.svelte';
  import ActionBar from './ActionBar.svelte';
  import OpenSession from './OpenSession.svelte';
  import CloseSession from './CloseSession.svelte';
  import RoomDialog from './RoomDialog.svelte';
  import PackageDialog from './PackageDialog.svelte';
  import Prepare from './Prepare.svelte';

  let dialog: DialogKind = $state(null);
  const show = (d: DialogKind) => (dialog = d);
  const hide = () => (dialog = null);

  const view = $derived(ui.view!);

  function onkeydown(e: KeyboardEvent) {
    if (e.key !== 'Escape' || dialog) return;
    // Échap ferme d'abord la fenêtre d'action ouverte, puis désélectionne.
    if (ui.pop) ui.pop = null;
    else {
      ui.placing = null;
      selection.clear();
    }
  }
</script>

<svelte:window {onkeydown} />

<div class="shell">
  <TopBar {show} />
  {#if ui.space === 'preparer'}
    {#if view.session}
      <div class="reminder" role="status">
        <span class="live" aria-hidden="true"></span>
        <span class="grow">Séance en cours en <b>{view.session.room_name}</b> jusqu'à <span class="mono">{hhmm(view.session.end_ms)}</span>.</span>
        <button type="button" class="btn" onclick={() => (ui.space = 'seance')}><Icon name="room" />Revenir à la séance</button>
      </div>
    {/if}
    <Prepare />
  {:else}
  <Banners />
  <div class="main">
    {#if view.room}
      <Plan {show} />
    {:else}
      <section class="stage empty">
        <h2>Aucune salle</h2>
        <p class="note">Créez la salle où vous faites cours : vous y placerez ensuite les postes comme dans la salle réelle.</p>
        <div><button class="btn primary" onclick={() => show('room-new')}>Créer une salle</button></div>
      </section>
    {/if}
    <Panel {show} />
  </div>
  {#if view.room}
    <ActionBar />
  {/if}
  {/if}
</div>

{#if dialog === 'open'}
  <OpenSession onclose={hide} />
{:else if dialog === 'close'}
  <CloseSession onclose={hide} />
{:else if dialog === 'room-new' || dialog === 'room-edit'}
  <RoomDialog edit={dialog === 'room-edit'} onclose={hide} />
{:else if dialog === 'package'}
  <PackageDialog onclose={hide} />
{/if}

<style>
  .shell { height: 100vh; display: flex; flex-direction: column; gap: 10px; padding: 10px 16px 12px; }
  .main { flex: 1; min-height: 0; display: grid; grid-template-columns: minmax(0, 1fr) 330px; gap: 10px; }
  .stage {
    min-height: 0; background: var(--surface); border: 1px solid var(--line); border-radius: 12px; box-shadow: var(--shadow);
  }
  .reminder {
    display: flex; align-items: center; gap: 10px; padding: 7px 12px; border-radius: 10px; font-size: 14px;
    background: var(--ok-soft); border: 1px solid color-mix(in srgb, var(--ok) 35%, transparent);
  }
  .reminder .live { width: 9px; height: 9px; border-radius: 50%; background: var(--ok); }
  .grow { flex: 1; }
  .empty { padding: 24px; display: flex; flex-direction: column; gap: 10px; }
  .empty h2 { margin: 0; font-size: 18px; }
  .empty p { margin: 0; }
  @media (max-width: 1050px) {
    .main { grid-template-columns: minmax(0, 1fr) 280px; }
  }
</style>
