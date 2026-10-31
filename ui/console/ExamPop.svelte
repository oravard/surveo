<script lang="ts">
  // Mode examen de la séance (SPEC.md, EXAM-1 à EXAM-3) : liste
  // d'autorisation vide par défaut, l'enseignant ajoutant les sites
  // permis ; applications interdites de la séance fermées ; heure de fin,
  // au plus tard celle de la séance.
  import { hhmm } from '$shared/time';
  import { ui, call } from './store.svelte';
  import Pop from './Pop.svelte';

  let { onclose }: { onclose: () => void } = $props();

  const session = ui.view!.session!;
  const exam = session.exam;
  const lists = $derived(ui.view!.lists.sites);
  let time = $state(hhmm(exam?.end_ms ?? session.end_ms));
  let text = $state(exam?.sites.join('\n') ?? '');
  const sites = $derived(text.split(/[\s,;]+/).map((s) => s.trim()).filter(Boolean));

  /** Heure choisie, le jour de la séance. */
  const end = $derived.by(() => {
    const [h, m] = time.split(':').map(Number);
    const d = new Date(session.opened_at_ms);
    d.setHours(h, m, 0, 0);
    if (d.getTime() < session.opened_at_ms) d.setDate(d.getDate() + 1);
    return d.getTime();
  });
  const late = $derived(end > session.end_ms);

  function pick(e: Event) {
    const l = lists.find((x) => x.name === (e.currentTarget as HTMLSelectElement).value);
    if (l) text = l.sites.join('\n');
  }

  async function submit(e: SubmitEvent) {
    e.preventDefault();
    if (await call('set_exam', { endMs: end, sites })) onclose();
  }
</script>

<Pop title={exam ? 'Mode examen' : 'Passer la séance en mode examen'} sub="Pour toute la salle. L'élève voit en permanence « Séance d'examen en cours jusqu'à … »." {onclose}>
  <form onsubmit={submit} class="form">
    <label class="field">
      Fin de l'examen
      <input type="time" bind:value={time} required />
      <small>{late ? `Au plus tard ${hhmm(session.end_ms)}, fin de la séance.` : 'Le poste revient ensuite au réglage de la séance.'}</small>
    </label>
    <label class="field">
      Sites autorisés, un par ligne
      <textarea bind:value={text} placeholder="Aucun site : tout le web est bloqué"></textarea>
      {#if lists.length}
        <select onchange={pick} aria-label="Reprendre une liste enregistrée">
          <option value="">Reprendre une liste enregistrée…</option>
          {#each lists as l (l.name)}<option value={l.name}>{l.name}</option>{/each}
        </select>
      {/if}
    </label>
    <p class="note">
      Applications interdites de la séance :
      {session.apps_all.length ? session.apps_all.join(', ') : 'aucune'} ; elles sont fermées dès qu'elles s'ouvrent.
    </p>
    <div class="row">
      {#if exam}<button type="button" class="btn" onclick={() => call('set_exam', { endMs: null, sites: [] }).then((ok) => ok && onclose())}>Quitter le mode examen</button>{/if}
      <span class="grow"></span>
      <button type="button" class="btn ghost" onclick={onclose}>Annuler</button>
      <button class="btn primary" disabled={late}>{exam ? 'Modifier' : 'Passer en mode examen'}</button>
    </div>
  </form>
</Pop>

<style>
  .form { display: flex; flex-direction: column; gap: 10px; }
  .field input[type='time'] { width: 9em; font-family: var(--mono); }
  .grow { flex: 1; }
  .note { margin: 0; }
</style>
