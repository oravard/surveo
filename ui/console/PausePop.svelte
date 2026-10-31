<script lang="ts">
  // Mettre en pause l'écran des postes visés, avec un message (SPEC.md,
  // ECRAN-1, ECRAN-2) : sans texte, le message par défaut.
  import { ui, call } from './store.svelte';
  import Pop from './Pop.svelte';

  let { members, scope, onclose }: { members: string[]; scope: string; onclose: () => void } = $props();

  const teacher = ui.view!.setup?.teacher ?? '';
  const PRESETS = ['Regardez le tableau, s’il vous plaît.', 'Écoutez les consignes.', 'Pause : on reprend dans quelques minutes.'];
  let text = $state('');

  async function submit(e: SubmitEvent) {
    e.preventDefault();
    if (await call('set_pause', { members, message: text })) onclose();
  }
</script>

<Pop title={`Mettre en pause l'écran de ${scope}`} sub="L'écran est recouvert par votre message ; le travail de l'élève reste tel quel." {onclose}>
  <form onsubmit={submit} class="form">
    <div class="chips">
      {#each PRESETS as p}<button type="button" onclick={() => (text = p)}>{p}</button>{/each}
    </div>
    <textarea bind:value={text} aria-label="Message affiché" placeholder={`Écran mis en pause par ${teacher}.`}
      onkeydown={(e) => e.key === 'Enter' && !e.shiftKey && (e.preventDefault(), e.currentTarget.form?.requestSubmit())}></textarea>
    <div class="row">
      <button type="button" class="btn ghost" onclick={onclose}>Annuler</button>
      <button class="btn primary">Mettre en pause</button>
    </div>
  </form>
</Pop>

<style>
  .form { display: flex; flex-direction: column; gap: 10px; }
</style>
