<script lang="ts">
  // Applications ouvertes d'un poste, et leur fermeture (SPEC.md, APP-1) :
  // l'élève est prévenu de chaque fermeture.
  import { onMount } from 'svelte';
  import Icon from '$shared/Icon.svelte';
  import type { StationView } from '$shared/bindings/StationView';
  import { ui, call } from './store.svelte';
  import Pop from './Pop.svelte';

  let { station, onclose }: { station: StationView; onclose: () => void } = $props();

  const teacher = ui.view!.setup?.teacher ?? '';
  const refresh = () => call('list_apps', { member: station.member });

  // Une seule demande à l'ouverture : la réponse met à jour `station`, qui
  // ne doit pas relancer la demande.
  onMount(() => {
    refresh();
  });
</script>

<Pop title={`Applications ouvertes · poste ${station.number}`} sub={`L'élève voit : « … a été fermée par ${teacher}. »`} {onclose}>
  {#if station.apps === null}
    <p class="note">Demande au poste…</p>
  {:else if station.apps.length === 0}
    <p class="note">Aucune application ouverte.</p>
  {:else}
    <div class="list">
      {#each station.apps as a (a.id)}
        <div class="opt app"><Icon name="appx" /><span class="grow">{a.name}</span>
          <button type="button" class="btn" onclick={() => call('close_app', { member: station.member, id: a.id })}>Fermer</button>
        </div>
      {/each}
    </div>
  {/if}
  <div class="row">
    <button type="button" class="btn ghost" onclick={refresh}>Actualiser</button>
    <button type="button" class="btn" onclick={onclose}>Fermer la liste</button>
  </div>
</Pop>

<style>
  .list { display: flex; flex-direction: column; gap: 6px; }
  .app { cursor: default; }
  .grow { flex: 1; }
</style>
