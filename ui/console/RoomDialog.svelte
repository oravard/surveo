<script lang="ts">
  // Créer une salle, ou changer la taille de son plan (SPEC.md, SALLE-2).
  import { untrack } from 'svelte';
  import { ui, call } from './store.svelte';
  import Dialog from './Dialog.svelte';

  let { edit, onclose }: { edit: boolean; onclose: () => void } = $props();

  // Valeurs de départ du formulaire, lues une fois à l'ouverture.
  const room = untrack(() => (edit ? ui.view!.room : null));
  let name = $state(room?.name ?? '');
  let cols = $state(room?.cols ?? 6);
  let rows = $state(room?.rows ?? 5);

  async function submit(e: SubmitEvent) {
    e.preventDefault();
    const ok = edit ? await call('resize_room', { cols, rows }) : await call('create_room', { name, cols, rows });
    if (ok) onclose();
  }
</script>

<Dialog title={edit ? `Plan de ${room?.name}` : 'Nouvelle salle'} {onclose}>
  <form onsubmit={submit} class="form">
    {#if !edit}
      <label class="field">Nom de la salle<input bind:value={name} required placeholder="B204" autocomplete="off" /></label>
    {/if}
    <div class="grid">
      <label class="field">Places par rangée<input type="number" min="1" max="12" bind:value={cols} required /></label>
      <label class="field">Rangées<input type="number" min="1" max="10" bind:value={rows} required /></label>
    </div>
    <p>
      {edit
        ? 'Les postes qui ne tiennent plus dans le plan retournent dans « postes à placer ».'
        : 'Le plan appartient à la salle : il sera le même pour tous les enseignants qui y feront cours.'}
    </p>
    <div class="row">
      <button type="button" class="btn ghost" onclick={onclose}>Annuler</button>
      <button class="btn primary">{edit ? 'Enregistrer' : 'Créer la salle'}</button>
    </div>
  </form>
</Dialog>

<style>
  .form { display: flex; flex-direction: column; gap: 12px; }
  .grid { display: grid; grid-template-columns: 1fr 1fr; gap: 10px; }
</style>
