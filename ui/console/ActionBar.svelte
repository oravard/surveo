<script lang="ts">
  // « Sélectionner, puis agir » (SPEC.md, SEL-1, SEL-2) : sans sélection,
  // l'action porte sur toute la salle, et la barre le dit.
  import Icon from '$shared/Icon.svelte';
  import { ui, selection, call } from './store.svelte';

  const room = $derived(ui.view!.room!);
  const count = $derived(selection.size);
  const online = $derived(room.stations.filter((s) => s.state !== 'off').length);

  function identify() {
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
    <button class="bbtn" onclick={identify} disabled={room.stations.length === 0} title="Afficher sur chaque poste le numéro de sa tuile">
      <Icon name="target" />Identifier
    </button>
  </div>
</div>

<style>
  .actionbar {
    background: var(--bar); color: var(--bar-ink); border-radius: 12px; padding: 8px 10px; display: flex; flex-wrap: wrap;
    align-items: center; gap: 8px 14px; box-shadow: var(--shadow);
  }
  .target { font-size: 14px; display: flex; gap: 10px; align-items: baseline; flex-wrap: wrap; padding-left: 4px; }
  .link { background: transparent; border: 0; color: var(--bar-muted); text-decoration: underline; cursor: pointer; font-size: 13px; padding: 0; }
  .actions { display: flex; gap: 6px; margin-left: auto; flex-wrap: wrap; }
  .bbtn {
    background: var(--bar-2); border: 0; color: var(--bar-ink); border-radius: 8px; padding: 7px 12px; font-weight: 700;
    cursor: pointer; display: inline-flex; align-items: center; gap: 6px;
  }
  .bbtn:disabled { opacity: 0.5; cursor: default; }
</style>
