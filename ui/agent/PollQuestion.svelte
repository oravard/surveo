<script lang="ts">
  // Question d'un sondage et sa réponse (SPEC.md, SONDAGE-2) : l'élève la
  // change tant que le sondage est ouvert. Les résultats montrés sont ceux
  // de la classe, sans aucun nom (SONDAGE-5).
  import { invoke } from '@tauri-apps/api/core';
  import { untrack } from 'svelte';
  import { hhmm } from '$shared/time';
  import type { PanelPoll } from '$shared/bindings/PanelPoll';

  let { poll, onanswer }: { poll: PanelPoll; onanswer?: () => void } = $props();

  let chosen: number[] = $state([]);
  // La réponse connue (donnée ici, dans la fenêtre du sondage, ou avant un
  // redémarrage) remplace le choix en cours dès qu'elle change.
  const known = $derived(JSON.stringify(poll.answer ?? []));
  $effect(() => {
    known;
    chosen = untrack(() => [...(poll.answer ?? [])]);
  });
  const changed = $derived(JSON.stringify([...chosen].sort()) !== JSON.stringify([...(poll.answer ?? [])].sort()));
  const total = $derived(poll.results ? poll.results.reduce((a, b) => a + b, 0) : 0);

  function toggle(i: number, on: boolean) {
    chosen = poll.multiple ? (on ? [...chosen, i] : chosen.filter((c) => c !== i)) : [i];
  }

  async function send() {
    await invoke('answer_poll', { choices: chosen });
    onanswer?.();
  }
</script>

<p class="q">{poll.question}</p>
{#if poll.open}
  <div class="opts" role={poll.multiple ? 'group' : 'radiogroup'} aria-label={poll.question}>
    {#each poll.options as o, i}
      <label class="opt" class:on={chosen.includes(i)}>
        <input type={poll.multiple ? 'checkbox' : 'radio'} name={`p-${poll.id}`} checked={chosen.includes(i)} onchange={(e) => toggle(i, e.currentTarget.checked)} />
        {o}
      </label>
    {/each}
  </div>
  {#if poll.multiple}<p class="note">Vous pouvez cocher plusieurs réponses.</p>{/if}
  <div class="row">
    <button type="button" class="btn primary" onclick={send} disabled={!chosen.length || (!changed && poll.answer !== null)}>
      {poll.answer ? 'Changer ma réponse' : 'Envoyer ma réponse'}
    </button>
    {#if poll.answer && !changed}<span class="note">Réponse envoyée{poll.closes_at_ms ? ` · le sondage se termine à ${hhmm(poll.closes_at_ms)}` : ''}.</span>{/if}
  </div>
{:else if !poll.results}
  <p class="note">Le sondage est terminé.</p>
{/if}
{#if poll.results}
  <div class="bars" aria-label="Résultats de la classe">
    {#each poll.options as o, i}
      <span>{o}</span><span class="mono">{poll.results[i]}</span>
      <span class="track"><span class="fill" style:width={`${total ? (poll.results[i] / total) * 100 : 0}%`}></span></span>
    {/each}
  </div>
  <p class="note">Résultats de la classe, sans aucun nom.</p>
{/if}

<style>
  p { margin: 0; }
  .q { font-weight: 700; font-size: 15.5px; }
  .opts { display: flex; flex-direction: column; gap: 6px; }
  .opt { display: flex; align-items: center; gap: 9px; border: 1px solid var(--line); border-radius: 9px; padding: 8px 10px; cursor: pointer; }
  .opt.on { border-color: var(--accent); background: var(--accent-soft); }
  .opt input { accent-color: var(--accent); width: 16px; height: 16px; }
  .row { display: flex; gap: 10px; align-items: center; flex-wrap: wrap; }
  .bars { display: grid; grid-template-columns: auto auto 1fr; gap: 4px 8px; align-items: center; font-size: 14px; }
  .track { height: 8px; border-radius: 99px; background: var(--surface-2); overflow: hidden; }
  .fill { display: block; height: 100%; background: var(--accent); }
</style>
