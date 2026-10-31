<script lang="ts">
  // Activités pédagogiques de la séance dans le panneau (SPEC.md,
  // PANNEAU-1) : synthèse de chaque TP (TP-8), fichiers distribués et leur
  // réception (DISTRIB-4), sondage et ses résultats en direct (SONDAGE-3 à
  // SONDAGE-5).
  import Icon from '$shared/Icon.svelte';
  import { hhmm } from '$shared/time';
  import { ui, call, selection } from './store.svelte';

  const session = $derived(ui.view!.session!);
  const stations = $derived(ui.view!.room?.stations ?? []);
  let details = $state(false);

  function selectLate(run: string) {
    selection.clear();
    for (const s of stations) if (s.tp?.run === run && s.tp.late) selection.add(s.member);
  }

  function stop(run: string) {
    const members = stations.filter((s) => s.tp?.run === run).map((s) => s.member);
    const all = session.tps.find((t) => t.id === run)?.all && members.length === stations.filter((s) => s.tp).length;
    call('stop_tp', { members: all ? [] : members });
  }

  const pct = (n: number, d: number) => (d ? Math.round((n / d) * 100) : 0);
  const poll = $derived(session.poll);
  const votes = $derived(poll ? poll.counts.reduce((a, b) => a + b, 0) : 0);
</script>

{#each session.tps as t (t.id)}
  <section class="psec">
    <h3><Icon name="list" size={15} />TP « {t.title} »<span class="count">{t.stations} poste{t.stations > 1 ? 's' : ''}</span></h3>
    <div class="bars">
      {#each t.steps as s, i}
        <span class="lbl mono">{i + 1}</span>
        <span class="track"><span class="fill" style:width={`${pct(t.at_step[i], t.stations)}%`}></span></span>
        <span class="n mono">{t.at_step[i]}</span>
        <span class="obj">{s}</span>
      {/each}
    </div>
    <ul class="kv">
      {#if t.finished}<li>{t.finished} poste{t.finished > 1 ? 's ont' : ' a'} terminé toutes les étapes</li>{/if}
      {#if t.expected}<li>Rendus complets : <span class="mono">{t.complete}/{t.stations}</span></li>{/if}
      {#if t.late}<li class="late"><Icon name="clock" size={13} />{t.late} en retard</li>{/if}
    </ul>
    <div class="tools">
      {#if t.late}<button type="button" class="btn" onclick={() => selectLate(t.id)}>Sélectionner les postes en retard</button>{/if}
      <button type="button" class="btn" onclick={() => call('open_rendus', { member: null })}><Icon name="folder" size={15} />Rendus</button>
      <button type="button" class="btn ghost" onclick={() => stop(t.id)}>Arrêter le TP</button>
    </div>
  </section>
{/each}

{#if session.files.length}
  <section class="psec">
    <h3><Icon name="file" size={15} />Fichiers distribués</h3>
    <ul class="kv">
      {#each session.files as f (f.id)}
        <li class:late={f.received < f.targets}>
          {f.name} · <span class="mono">{hhmm(f.at_ms)}</span> · reçu sur <span class="mono">{f.received}/{f.targets}</span>{f.received < f.targets ? ', les autres à leur retour' : ''}
        </li>
      {/each}
    </ul>
  </section>
{/if}

{#if poll}
  <section class="psec">
    <h3><Icon name="poll" size={15} />Sondage<span class="count mono">{poll.answered}/{poll.targets}</span></h3>
    <b>{poll.question}</b>
    <div class="bars poll">
      {#each poll.options as o, i}
        <span class="obj full">{o}</span>
        <span class="track"><span class="fill" style:width={`${pct(poll.counts[i], votes)}%`}></span></span>
        <span class="n mono">{poll.counts[i]}</span>
      {/each}
    </div>
    <p class="note">
      Taux de réponse : {pct(poll.answered, poll.targets)} % ·
      {poll.open ? (poll.closes_at_ms ? `se clôt à ${hhmm(poll.closes_at_ms)}` : 'ouvert') : 'clos'}
    </p>
    {#if poll.details.length}
      <button type="button" class="link" aria-expanded={details} onclick={() => (details = !details)}>{details ? 'Masquer' : 'Voir'} le détail par poste</button>
      {#if details}
        <ul class="kv">
          {#each poll.details as d}<li>{d.station} : {d.choices.map((c) => poll.options[c]).join(', ')}</li>{/each}
        </ul>
      {/if}
    {/if}
    <div class="tools">
      {#if poll.open}<button type="button" class="btn" onclick={() => call('close_poll')}>Clore le sondage</button>{/if}
      <button type="button" class="btn" aria-pressed={poll.shown} onclick={() => call('show_poll', { shown: !poll.shown })}>{poll.shown ? 'Résultats montrés' : 'Montrer aux élèves'}</button>
      {#if !poll.open}<button type="button" class="btn ghost" onclick={() => call('clear_poll')}>Retirer</button>{/if}
    </div>
    {#if poll.shown}<p class="note">Les élèves voient les résultats de la classe, sans aucun nom.</p>{/if}
  </section>
{/if}

<style>
  .psec {
    background: var(--surface); border: 1px solid var(--line); border-radius: 12px; box-shadow: var(--shadow); padding: 10px 12px;
    display: flex; flex-direction: column; gap: 8px; font-size: 14px;
  }
  h3 { margin: 0; font-size: 14.5px; display: flex; align-items: center; gap: 7px; }
  .count { margin-left: auto; font-size: 12.5px; color: var(--muted); font-weight: 400; }
  .bars { display: grid; grid-template-columns: auto 1fr auto; gap: 2px 8px; align-items: center; }
  .lbl { font-size: 12.5px; color: var(--muted); }
  .track { height: 8px; border-radius: 99px; background: var(--surface-2); overflow: hidden; }
  .fill { display: block; height: 100%; background: var(--accent); border-radius: 99px; }
  .n { font-size: 12.5px; text-align: right; min-width: 1.5em; }
  .obj { grid-column: 2 / 4; font-size: 12.5px; color: var(--muted); margin: -1px 0 3px; }
  .bars.poll { grid-template-columns: 1fr auto; }
  .obj.full { grid-column: 1 / 3; color: var(--ink); margin: 3px 0 0; }
  .kv { margin: 0; padding-left: 18px; display: flex; flex-direction: column; gap: 3px; }
  .late { color: var(--late); }
  .late :global(.ic) { vertical-align: -2px; margin-right: 3px; }
  .tools { display: flex; flex-wrap: wrap; gap: 6px; }
  p { margin: 0; }
  .link { align-self: flex-start; background: none; border: 0; padding: 0; color: var(--accent); text-decoration: underline; cursor: pointer; font-size: 13px; }
</style>
