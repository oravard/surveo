<script lang="ts">
  // Lancer un TP sur les postes visés (SPEC.md, TP-1), en le choisissant
  // dans la bibliothèque sans quitter la séance (PANNEAU-3). Remplacer le
  // TP d'un poste se confirme, en énonçant la portée (TP-4, RETOUR-4).
  import Icon from '$shared/Icon.svelte';
  import type { StationView } from '$shared/bindings/StationView';
  import type { Model } from '$shared/bindings/Model';
  import { ui, call } from './store.svelte';
  import Pop from './Pop.svelte';
  import { modelSummary } from './peda';

  let { members, stations, scope, onclose }: { members: string[]; stations: StationView[]; scope: string; onclose: () => void } = $props();

  const models = $derived(ui.view!.library.models);
  const running = $derived(stations.filter((s) => s.tp !== null));
  let confirm: Model | null = $state(null);

  const plural = (n: number, w: string) => `${n} ${w}${n > 1 ? 's' : ''}`;

  async function launch(m: Model) {
    if (running.length && !confirm) {
      confirm = m;
      return;
    }
    if (await call('launch_tp', { members, model: m.id })) onclose();
  }

  async function stop() {
    if (await call('stop_tp', { members })) onclose();
  }

  const runningTitles = $derived([...new Set(running.map((s) => s.tp!.title))]);
</script>

{#if confirm}
  <Pop title={`Remplacer le TP en cours sur ${plural(running.length, 'poste')} ?`} {onclose}>
    <p class="sub">
      {runningTitles.length === 1 ? `Le TP « ${runningTitles[0]} »` : 'Les TP en cours'} {runningTitles.length === 1 ? 'sera remplacé' : 'seront remplacés'}
      par « {confirm.title} ». Les rendus déjà reçus sont conservés.
    </p>
    <div class="row">
      <button type="button" class="btn ghost" onclick={() => (confirm = null)}>Annuler</button>
      <button type="button" class="btn primary" onclick={() => launch(confirm!)}>Remplacer le TP</button>
    </div>
  </Pop>
{:else}
  <Pop title={`Lancer un TP sur ${scope}`} sub="Choisissez dans votre bibliothèque. Pour créer ou modifier un modèle, passez par Préparer." {onclose}>
    {#if models.length === 0}
      <p class="sub">Votre bibliothèque ne contient encore aucun modèle de TP.</p>
      <div class="row"><button type="button" class="btn primary" onclick={() => { ui.space = 'preparer'; onclose(); }}><Icon name="book" />Créer un modèle dans Préparer</button></div>
    {:else}
      {#each models as m (m.id)}
        {@const n = stations.filter((s) => s.tp?.title === m.title).length}
        <button type="button" class="opt" onclick={() => launch(m)}>
          <Icon name="list" /><span>{m.title}<small>{modelSummary(m)}{n ? ` · en cours sur ${plural(n, 'poste')}` : ''}</small></span>
        </button>
      {/each}
    {/if}
    {#if running.length}
      <div class="row">
        <button type="button" class="btn" onclick={stop}>Arrêter le TP sur {running.length === stations.length ? scope : plural(running.length, 'poste')}</button>
      </div>
    {/if}
  </Pop>
{/if}
