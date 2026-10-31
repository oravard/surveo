<script lang="ts">
  // Lancer un sondage sur les postes visés (SPEC.md, SONDAGE-1, SONDAGE-2,
  // SONDAGE-4) : repris de la bibliothèque ou écrit sur le moment, il y est
  // enregistré (SONDAGE-6). Un seul sondage à la fois.
  import Icon from '$shared/Icon.svelte';
  import type { PollModel } from '$shared/bindings/PollModel';
  import { ui, call } from './store.svelte';
  import Pop from './Pop.svelte';

  let { members, scope, onclose }: { members: string[]; scope: string; onclose: () => void } = $props();

  const current = $derived(ui.view!.session?.poll ?? null);
  const saved = $derived(ui.view!.library.polls);
  let from: PollModel | null = $state(null);
  let question = $state('');
  let options: string[] = $state(['', '']);
  let multiple = $state(false);
  let minutes = $state(0);

  function pick(p: PollModel) {
    from = p;
    question = p.question;
    options = [...p.options];
    multiple = p.multiple;
  }

  async function launch(e: SubmitEvent) {
    e.preventDefault();
    const same = from && from.question === question && from.multiple === multiple && JSON.stringify(from.options) === JSON.stringify(options);
    const poll = { id: same ? from!.id : '', question, options, multiple };
    if (await call('launch_poll', { members, poll, minutes: minutes || null })) onclose();
  }
</script>

{#if current?.open}
  <Pop title="Un sondage est en cours" sub={`« ${current.question} » · ${current.answered}/${current.targets} réponses`} {onclose}>
    <div class="row">
      <button type="button" class="btn ghost" onclick={onclose}>Fermer</button>
      <button type="button" class="btn primary" onclick={async () => (await call('close_poll')) && onclose()}>Clore le sondage</button>
    </div>
  </Pop>
{:else}
  <Pop title={`Sondage pour ${scope}`} sub="La question s'affiche au premier plan chez l'élève ; il peut changer sa réponse tant que le sondage est ouvert." {onclose}>
    {#if saved.length}
      <div class="chips">
        {#each saved as p (p.id)}<button type="button" aria-pressed={from?.id === p.id} onclick={() => pick(p)}>{p.question}</button>{/each}
      </div>
    {/if}
    <form class="form" onsubmit={launch}>
      <input class="q" bind:value={question} placeholder="Question" aria-label="Question" required />
      {#each options as _, i}
        <div class="o">
          <input bind:value={options[i]} placeholder={`Réponse ${i + 1}`} aria-label={`Réponse ${i + 1}`} />
          <button type="button" class="btn ghost" aria-label="Retirer cette réponse" onclick={() => options.splice(i, 1)} disabled={options.length <= 2}><Icon name="x" size={14} /></button>
        </div>
      {/each}
      <div class="line">
        <button type="button" class="btn ghost" onclick={() => options.push('')} disabled={options.length >= 10}><Icon name="plus" size={14} />Réponse</button>
        <label><input type="checkbox" bind:checked={multiple} /> Choix multiple</label>
        <label>Durée
          <select bind:value={minutes}>
            <option value={0}>sans limite</option>
            {#each [1, 2, 3, 5, 10] as m}<option value={m}>{m} min</option>{/each}
          </select>
        </label>
      </div>
      <div class="row">
        <button type="button" class="btn ghost" onclick={onclose}>Annuler</button>
        <button class="btn primary">Lancer le sondage</button>
      </div>
    </form>
  </Pop>
{/if}

<style>
  .form { display: flex; flex-direction: column; gap: 6px; }
  input:not([type='checkbox']), select { border: 1px solid var(--line); border-radius: 8px; padding: 6px 9px; background: var(--surface); }
  .q { font-weight: 700; }
  .o { display: flex; gap: 4px; }
  .o input { flex: 1; }
  .line { display: flex; align-items: center; gap: 12px; flex-wrap: wrap; font-size: 14px; }
  .line label { display: inline-flex; align-items: center; gap: 6px; }
  .chips button[aria-pressed='true'] { border-color: var(--accent); color: var(--accent); background: var(--accent-soft); }
</style>
