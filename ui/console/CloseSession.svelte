<script lang="ts">
  // Clôturer : action de grande portée, confirmée en énonçant sa portée
  // (SPEC.md, RETOUR-4, SEANCE-5).
  import { ui, call, selection } from './store.svelte';
  import Dialog from './Dialog.svelte';

  let { onclose }: { onclose: () => void } = $props();

  const n = (ui.view!.room?.stations ?? []).filter((s) => s.state === 'in_session').length;

  async function confirm() {
    if (await call('close_session')) {
      selection.clear();
      onclose();
    }
  }
</script>

<Dialog title="Clôturer la séance ?" {onclose}>
  <p>
    {n === 0 ? 'Aucun poste n’est dans la séance.' : `Les ${n} poste${n > 1 ? 's' : ''} de la séance reviendront à leur état d'origine.`}
    Un poste injoignable y reviendra de lui-même.
  </p>
  <div class="row">
    <button type="button" class="btn ghost" onclick={onclose}>Annuler</button>
    <button type="button" class="btn primary" onclick={confirm}>Clôturer la séance</button>
  </div>
</Dialog>
