<script lang="ts">
  // Modifier un sondage de la bibliothèque (SPEC.md, SONDAGE-1,
  // SONDAGE-6) : une question, des réponses proposées, choix unique ou
  // multiple.
  import { invoke } from '@tauri-apps/api/core';
  import Icon from '$shared/Icon.svelte';
  import type { PollModel } from '$shared/bindings/PollModel';
  import { toast } from './store.svelte';

  let { poll, onclose }: { poll: PollModel; onclose: () => void } = $props();
  // svelte-ignore state_referenced_locally
  let draft: PollModel = $state(structuredClone($state.snapshot(poll)));

  async function submit(e: SubmitEvent) {
    e.preventDefault();
    try {
      await invoke('save_poll', { poll: $state.snapshot(draft) });
      onclose();
    } catch (err) {
      toast({ text: String(err), detail: null, warning: true }, 8000);
    }
  }
</script>

<form class="editor" onsubmit={submit}>
  <label class="field">Question
    <input bind:value={draft.question} placeholder="Par exemple « Où en êtes-vous de l'étape 2 ? »" required />
  </label>
  <fieldset>
    <legend>Réponses proposées</legend>
    {#each draft.options as _, i}
      <div class="opt">
        <input bind:value={draft.options[i]} aria-label={`Réponse ${i + 1}`} placeholder={`Réponse ${i + 1}`} />
        <button type="button" class="btn ghost" aria-label="Retirer cette réponse" onclick={() => draft.options.splice(i, 1)} disabled={draft.options.length <= 2}><Icon name="x" size={14} /></button>
      </div>
    {/each}
    <div><button type="button" class="btn" onclick={() => draft.options.push('')} disabled={draft.options.length >= 10}><Icon name="plus" size={15} />Ajouter une réponse</button></div>
  </fieldset>
  <label class="check"><input type="checkbox" bind:checked={draft.multiple} /> Choix multiple : l'élève peut cocher plusieurs réponses</label>
  <div class="foot">
    <button type="button" class="btn ghost" onclick={onclose}>Annuler</button>
    <button class="btn primary">Enregistrer le sondage</button>
  </div>
</form>

<style>
  .editor { display: flex; flex-direction: column; gap: 12px; max-width: 620px; }
  fieldset { border: 1px solid var(--line); border-radius: 10px; padding: 10px 12px; display: flex; flex-direction: column; gap: 6px; margin: 0; }
  legend { font-size: 14px; padding: 0 4px; }
  .opt { display: flex; gap: 6px; }
  .opt input { flex: 1; border: 1px solid var(--line); border-radius: 8px; padding: 6px 9px; background: var(--surface); }
  .check { display: flex; align-items: center; gap: 8px; font-size: 14px; }
  .foot { display: flex; gap: 8px; justify-content: flex-end; }
</style>
