<script lang="ts">
  // Ouvrir une séance : la salle choisie et une heure de fin (SPEC.md,
  // SEANCE-1, SEANCE-4).
  import { duration, hhmm } from '$shared/time';
  import { ui, call } from './store.svelte';
  import Dialog from './Dialog.svelte';

  let { onclose }: { onclose: () => void } = $props();

  const view = ui.view!;
  const limits = view.limits;
  const fiveMin = 5 * 60_000;
  const defaultEnd = Math.ceil((Date.now() + limits.default_duration_ms) / fiveMin) * fiveMin;
  let time = $state(hhmm(defaultEnd));

  /** Heure choisie, aujourd'hui, ou demain si elle est déjà passée. */
  const end = $derived.by(() => {
    const [h, m] = time.split(':').map(Number);
    const d = new Date();
    d.setHours(h, m, 0, 0);
    if (d.getTime() < Date.now()) d.setDate(d.getDate() + 1);
    return d.getTime();
  });
  const length = $derived(end - Date.now());
  const tooLong = $derived(length > limits.max_duration_ms);

  async function submit(e: SubmitEvent) {
    e.preventDefault();
    if (await call('open_session', { endMs: end })) onclose();
  }
</script>

<Dialog title={`Ouvrir une séance en ${view.room?.name}`} {onclose}>
  <form onsubmit={submit} class="form">
    <label class="field">
      Heure de fin
      <input type="time" bind:value={time} required step="300" />
      <small>
        {#if tooLong}
          Une séance dure au plus {duration(limits.max_duration_ms)}, prolongations comprises.
        {:else}
          Durée : {duration(length)}. Sans clôture, la séance se termine d'elle-même {duration(limits.margin_ms)} après.
        {/if}
      </small>
    </label>
    <p>Les postes allumés de la salle rejoignent la séance aussitôt, et chaque élève en est informé sur son écran.</p>
    <div class="row">
      <button type="button" class="btn ghost" onclick={onclose}>Annuler</button>
      <button class="btn primary" disabled={tooLong}>Ouvrir la séance</button>
    </div>
  </form>
</Dialog>

<style>
  .form { display: flex; flex-direction: column; gap: 12px; }
  .field input { width: 9em; font-family: var(--mono); font-size: 16px; }
</style>
