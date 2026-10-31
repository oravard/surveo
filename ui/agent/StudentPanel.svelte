<script lang="ts">
  // Panneau élève (SPEC.md, AGENT-3, TP-5) : le TP et ses étapes, les
  // consignes de l'étape en cours, le temps indicatif, les fichiers joints,
  // la zone de dépôt des rendus et leurs accusés (RENDU-1 à RENDU-3), les
  // fichiers reçus (DISTRIB-3) et le sondage (SONDAGE-2, SONDAGE-5). Des
  // textes courts, pour un collégien (AGENT-5).
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import { getCurrentWebview } from '@tauri-apps/api/webview';
  import Icon from '$shared/Icon.svelte';
  import { hhmm, duration } from '$shared/time';
  import { markdown } from '$shared/markdown';
  import type { PanelView } from '$shared/bindings/PanelView';
  import type { PanelRendu } from '$shared/bindings/PanelRendu';
  import PollQuestion from './PollQuestion.svelte';

  let v = $state<PanelView | null>(null);
  let now = $state(Date.now());
  let hover = $state(false);
  let open: number | null = $state(null);

  invoke<PanelView | null>('panel_state').then((p) => (v = p));
  listen<PanelView>('panel', (e) => (v = e.payload));
  setInterval(() => (now = Date.now()), 15000);
  getCurrentWebview().onDragDropEvent((e) => (hover = e.payload.type === 'enter' || e.payload.type === 'over'));

  const tp = $derived(v?.tp ?? null);
  const current = $derived(tp && tp.current !== null ? tp.steps[tp.current] : null);
  const left = $derived(current && current.minutes ? tp!.current_since_ms + current.minutes * 60000 - now : null);

  function rendu(r: PanelRendu): string {
    switch (r.state.t) {
      case 'waiting': return `« ${r.name} » en attente d'envoi : la connexion avec l'enseignant est interrompue.`;
      case 'sending': return `« ${r.name} » : envoi en cours…`;
      case 'received': return `« ${r.name} » reçu par l'enseignant à ${hhmm(r.state.at_ms)}.`;
      case 'refused': return r.state.reason;
    }
  }

  function ended(): string {
    const e = v!.ended!;
    const head = e.t === 'session' ? 'Séance terminée.' : `Le TP « ${e.title} » est terminé.`;
    return e.pending === 0
      ? `${head} Vos rendus envoyés ont bien été reçus.`
      : `${head} ${e.pending} rendu${e.pending > 1 ? 's' : ''} ${e.pending > 1 ? 'seront envoyés' : 'sera envoyé'} dès que la connexion avec l'enseignant sera rétablie.`;
  }

  /** Un lien d'une consigne s'ouvre dans le navigateur, jamais ici. */
  function links(e: MouseEvent) {
    const a = (e.target as Element).closest?.('a[data-url]');
    if (a) {
      e.preventDefault();
      invoke('open_link', { url: a.getAttribute('data-url') });
    }
  }
</script>

<!-- Clic délégué aux liens des consignes, qui restent des liens au clavier. -->
<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_noninteractive_element_interactions -->
<main onclick={links}>
  {#if !v}
    <p class="note">Chargement…</p>
  {:else}
    {#if v.ended}
      <section class="ended" role="status"><Icon name="check" size={18} /><p>{ended()}</p></section>
    {/if}

    {#if tp}
      <section>
        <h1>TP « {tp.title} »</h1>
        <ol class="steps">
          {#each tp.steps as s, i}
            <li class:cur={i === tp.current} class:done={s.done}>
              <label>
                <input type="checkbox" checked={s.done} onchange={(e) => invoke('set_step', { step: i, done: e.currentTarget.checked })} />
                <span>{s.objective}</span>
              </label>
              {#if i === tp.current}
                {#if left !== null}
                  <span class="time" class:late={tp.late}>
                    {tp.late ? 'Temps indicatif dépassé : continuez à votre rythme.' : `Temps indicatif restant : ${duration(left)}`}
                  </span>
                {/if}
                {#if s.instructions.trim()}<div class="md consigne">{@html markdown(s.instructions)}</div>{/if}
              {:else if s.instructions.trim()}
                <button type="button" class="link" onclick={() => (open = open === i ? null : i)}>{open === i ? 'Masquer les consignes' : 'Voir les consignes'}</button>
                {#if open === i}<div class="md consigne">{@html markdown(s.instructions)}</div>{/if}
              {/if}
            </li>
          {/each}
        </ol>
        {#if tp.current === null}<p class="ok">Toutes les étapes sont faites. Bravo !</p>{/if}
        <p class="note">Vous cochez vous-même les étapes terminées.</p>
        {#if tp.files.length}
          <h2>Fichiers du TP</h2>
          {#each tp.files as f (f.id)}
            <div class="file"><Icon name="file" size={16} /><span class="grow">{f.name}<small>{f.size}</small></span>
              {#if f.received}<button type="button" class="btn" onclick={() => invoke('open_received', { id: f.id })}>Ouvrir</button>{:else}<span class="note">réception…</span>{/if}
            </div>
          {/each}
        {/if}
      </section>
    {/if}

    {#if tp || v.rendus.length}
      <section>
        <h2>Rendre mon travail{tp && tp.expected ? ` · ${tp.received}/${tp.expected}` : ''}</h2>
        {#if v.can_submit}
          <div class="drop" class:hover>
            <Icon name="file" size={20} />
            <span>Glissez un fichier ici</span>
            <button type="button" class="btn" onclick={() => invoke('pick_rendu')}>Choisir un fichier…</button>
            <small>{v.limit} au plus. Votre fichier reste à sa place : Surveo en envoie une copie.</small>
          </div>
        {/if}
        {#if v.error}<p class="warn" role="alert">{v.error}</p>{/if}
        {#each v.rendus as r (r.id)}
          <p class="rendu" class:warn={r.state.t === 'refused' || r.state.t === 'waiting'}>
            <Icon name={r.state.t === 'received' ? 'check' : r.state.t === 'refused' ? 'x' : 'clock'} size={15} />{rendu(r)}
          </p>
        {/each}
      </section>
    {/if}

    {#if v.files.length}
      <section>
        <h2>Fichiers reçus</h2>
        {#each v.files as f (f.id)}
          <div class="file"><Icon name="file" size={16} /><span class="grow">{f.name}<small>{f.size}</small></span>
            {#if f.received}<button type="button" class="btn" onclick={() => invoke('open_received', { id: f.id })}>Ouvrir</button>{:else}<span class="note">réception…</span>{/if}
          </div>
        {/each}
        <div class="row">
          <button type="button" class="btn ghost" onclick={() => invoke('open_received', { id: null })}><Icon name="folder" size={15} />Ouvrir le dossier</button>
        </div>
        {#if v.removed_at_end}<p class="note">Ces fichiers seront retirés de ce poste à la fin de la séance : rendez votre travail avant.</p>{/if}
      </section>
    {/if}

    {#if v.poll}
      <section>
        <h2>Sondage de {v.teacher}</h2>
        <PollQuestion poll={v.poll} />
      </section>
    {/if}

    {#if !tp && !v.ended && !v.files.length && !v.poll && !v.rendus.length}
      <p class="note empty">Rien pour l'instant. Le TP, les fichiers et les sondages de {v.teacher || 'l’enseignant'} apparaîtront ici.</p>
    {/if}
    {#if v.ends_at_ms}<p class="note foot">Séance jusqu'à {hhmm(v.ends_at_ms)}{v.connected ? '' : ' · connexion avec l’enseignant interrompue'}</p>{/if}
  {/if}
</main>

<style>
  main { padding: 14px 16px; display: flex; flex-direction: column; gap: 14px; min-height: 100vh; }
  section { display: flex; flex-direction: column; gap: 8px; }
  h1 { margin: 0; font-size: 18px; }
  h2 { margin: 0; font-size: 15.5px; }
  p { margin: 0; }
  .steps { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 6px; }
  .steps li { border: 1px solid var(--line); border-radius: 9px; padding: 8px 10px; display: flex; flex-direction: column; gap: 6px; }
  .steps li.cur { border-color: var(--accent); background: var(--accent-soft); }
  .steps li.done span { color: var(--muted); text-decoration: line-through; }
  label { display: flex; align-items: flex-start; gap: 9px; cursor: pointer; font-weight: 700; }
  label input { margin-top: 3px; width: 17px; height: 17px; accent-color: var(--accent); flex: none; }
  .time { font-size: 13px; color: var(--muted); }
  .time.late { color: var(--late); font-weight: 700; }
  .consigne { background: var(--surface); border: 1px solid var(--line); border-radius: 8px; padding: 8px 10px; }
  .link { align-self: flex-start; background: none; border: 0; padding: 0; color: var(--accent); text-decoration: underline; cursor: pointer; font-size: 13px; }
  .ok { color: var(--ok); font-weight: 700; }
  .file { display: flex; align-items: center; gap: 8px; border: 1px solid var(--line); border-radius: 8px; padding: 6px 6px 6px 10px; font-size: 14px; }
  .file small { display: block; color: var(--muted); font-size: 12px; }
  .grow { flex: 1; min-width: 0; overflow-wrap: anywhere; }
  .drop {
    border: 2px dashed var(--line); border-radius: 10px; padding: 14px; display: flex; flex-direction: column; align-items: center; gap: 8px;
    text-align: center; color: var(--muted);
  }
  .drop.hover { border-color: var(--accent); background: var(--accent-soft); color: var(--accent); }
  .drop small { font-size: 12.5px; }
  .rendu { display: flex; gap: 6px; font-size: 14px; }
  .rendu :global(.ic) { margin-top: 2px; color: var(--ok); }
  .warn { color: var(--late); }
  .rendu.warn :global(.ic) { color: var(--late); }
  .ended { flex-direction: row; align-items: flex-start; gap: 8px; background: var(--ok-soft); border-radius: 10px; padding: 10px 12px; font-weight: 700; }
  .ended :global(.ic) { color: var(--ok); margin-top: 2px; }
  .row { display: flex; gap: 6px; }
  .empty { margin-top: 20px; text-align: center; }
  .foot { margin-top: auto; }
</style>
