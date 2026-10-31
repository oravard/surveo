<script lang="ts">
  // Distribuer des fichiers aux postes visés (SPEC.md, DISTRIB-1 à
  // DISTRIB-4). On peut aussi les glisser sur le plan ou sur une tuile.
  import Icon from '$shared/Icon.svelte';
  import { ui, call, pickFiles } from './store.svelte';
  import Pop from './Pop.svelte';
  import { sizeText } from './peda';

  let { members, scope, onclose }: { members: string[]; scope: string; onclose: () => void } = $props();
  const sent = $derived(ui.view!.session?.files ?? []);

  async function choose() {
    const files = await pickFiles(`Fichiers à distribuer à ${scope}`);
    if (files && (await call('distribute', { members, files }))) onclose();
  }
</script>

<Pop title={`Distribuer des fichiers à ${scope}`} sub="Ils arrivent dans le dossier Surveo de l'élève, sans jamais écraser un de ses fichiers, et sont retirés des postes de salle à la fin de la séance. Vous pouvez aussi glisser un fichier sur le plan ou sur une tuile." {onclose}>
  <div class="row"><button type="button" class="btn primary" onclick={choose}><Icon name="file" />Choisir des fichiers…</button></div>
  {#if sent.length}
    <p class="sub">Déjà distribués pendant cette séance :</p>
    {#each sent as f (f.id)}
      <div class="opt static"><Icon name="file" /><span>{f.name}<small>{sizeText(f.size)} · {f.all ? 'toute la séance' : `${f.targets} poste${f.targets > 1 ? 's' : ''}`} · reçu sur {f.received}/{f.targets}</small></span></div>
    {/each}
  {/if}
</Pop>

<style>
  .static { cursor: default !important; }
</style>
