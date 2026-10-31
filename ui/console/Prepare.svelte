<script lang="ts">
  // Espace Préparer (SPEC.md, ESPACE-1, PANNEAU-3) : la bibliothèque de
  // l'enseignant, modèles de TP et sondages. On y crée, modifie, duplique,
  // supprime, exporte et importe (MODELE-1, MODELE-4, SONDAGE-6) ; on les
  // lance depuis la séance.
  import { open, save } from '@tauri-apps/plugin-dialog';
  import { invoke } from '@tauri-apps/api/core';
  import Icon from '$shared/Icon.svelte';
  import type { Model } from '$shared/bindings/Model';
  import type { PollModel } from '$shared/bindings/PollModel';
  import { ui, call, toast } from './store.svelte';
  import ModelEditor from './ModelEditor.svelte';
  import PollEditor from './PollEditor.svelte';
  import { modelSummary } from './peda';

  type Section = 'tp' | 'poll';
  let section: Section = $state('tp');
  let selected = $state('');
  let editing: Model | null = $state(null);
  let editingPoll: PollModel | null = $state(null);
  let confirmDelete = $state('');

  const lib = $derived(ui.view!.library);
  const model = $derived(lib.models.find((m) => m.id === selected) ?? lib.models[0] ?? null);

  function choose(s: Section) {
    section = s;
    editing = null;
    editingPoll = null;
    confirmDelete = '';
  }

  function newModel() {
    editing = { id: '', title: '', expected: 1, files: [], steps: [{ objective: '', instructions: '', minutes: 15, overrun: 'nothing' }] };
  }

  async function duplicate(id: string) {
    try {
      selected = await invoke<string>('duplicate_model', { id });
    } catch (e) {
      toast({ text: String(e), detail: null, warning: true });
    }
  }

  async function remove(kind: 'model' | 'poll', id: string) {
    confirmDelete = '';
    await call(kind === 'model' ? 'delete_model' : 'delete_poll', { id });
  }

  /** Un fichier unique, fichiers joints compris (MODELE-4). */
  async function exportOne(kind: 'model' | 'poll', id: string, name: string) {
    const path = await save({ defaultPath: `${name.replace(/[\\/:*?"<>|]/g, '_')}.surveo`, filters: [{ name: 'Bibliothèque Surveo', extensions: ['surveo'] }] });
    if (!path) return;
    if (await call('export_library', { models: kind === 'model' ? [id] : [], polls: kind === 'poll' ? [id] : [], path })) {
      toast({ text: `« ${name} » exporté`, detail: 'Un collègue l’importe dans sa propre console, fichiers joints compris.', warning: false });
    }
  }

  async function importFile() {
    const path = await open({ multiple: false, directory: false, filters: [{ name: 'Bibliothèque Surveo', extensions: ['surveo'] }] });
    if (!path) return;
    try {
      const [m, p] = await invoke<[number, number]>('import_library', { path });
      const parts = [m && `${m} modèle${m > 1 ? 's' : ''} de TP`, p && `${p} sondage${p > 1 ? 's' : ''}`].filter(Boolean);
      toast({ text: parts.length ? `Importé : ${parts.join(' et ')}` : 'Rien à importer dans ce fichier', detail: null, warning: !parts.length });
      if (!m && p) section = 'poll';
    } catch (e) {
      toast({ text: String(e), detail: null, warning: true }, 8000);
    }
  }
</script>

<div class="space">
  <nav aria-label="Bibliothèque">
    <h2>Bibliothèque</h2>
    <button type="button" aria-current={section === 'tp'} onclick={() => choose('tp')}><Icon name="list" size={16} />Modèles de TP<span>{lib.models.length}</span></button>
    <button type="button" aria-current={section === 'poll'} onclick={() => choose('poll')}><Icon name="poll" size={16} />Sondages<span>{lib.polls.length}</span></button>
  </nav>

  <div class="content">
    {#if section === 'tp'}
      {#if editing}
        <ModelEditor model={editing} onclose={(id) => { editing = null; if (id) selected = id; }} />
      {:else}
        <h2 class="t">Modèles de TP</h2>
        <div class="toolbar">
          <button type="button" class="btn primary" onclick={newModel}><Icon name="plus" size={16} />Nouveau modèle</button>
          <button type="button" class="btn ghost" onclick={importFile}>Importer…</button>
        </div>
        {#if lib.models.length === 0}
          <p class="note">Aucun modèle encore. Un modèle décrit un TP réutilisable : ses étapes, ses consignes, ses fichiers joints et le nombre de rendus attendus. Vous le lancerez ensuite pendant la séance, sur toute la salle ou sur quelques postes.</p>
        {:else}
          <div class="cards">
            {#each lib.models as m (m.id)}
              <button type="button" class="card" class:cur={model?.id === m.id} onclick={() => { selected = m.id; confirmDelete = ''; }}>
                <h3>{m.title}</h3>
                <p>{modelSummary(m)}</p>
                <p>{m.files.length ? `Fichiers joints : ${m.files.map((f) => f.name).join(', ')}` : 'Aucun fichier joint'}</p>
              </button>
            {/each}
          </div>
          {#if model}
            <section class="detail">
              <div class="dhead">
                <h3>{model.title} · étapes</h3>
                <span class="grow"></span>
                <button type="button" class="btn" onclick={() => (editing = structuredClone($state.snapshot(model)))}>Modifier</button>
                <button type="button" class="btn ghost" onclick={() => duplicate(model.id)}>Dupliquer</button>
                <button type="button" class="btn ghost" onclick={() => exportOne('model', model.id, model.title)}>Exporter…</button>
                {#if confirmDelete === model.id}
                  <span class="confirm">Supprimer ce modèle ?
                    <button type="button" class="btn ghost" onclick={() => (confirmDelete = '')}>Non</button>
                    <button type="button" class="btn danger" onclick={() => remove('model', model.id)}>Supprimer</button>
                  </span>
                {:else}
                  <button type="button" class="btn ghost" onclick={() => (confirmDelete = model.id)}>Supprimer…</button>
                {/if}
              </div>
              <ol class="steps">
                {#each model.steps as s, i}
                  <li>
                    <span class="no mono">{i + 1}</span>
                    <span>{s.objective}</span>
                    <span class="du mono">{s.minutes ? `${s.minutes} min` : 'sans durée'}</span>
                    <span class="ac">Si dépassement : {s.overrun === 'show_instructions' ? 'afficher les consignes' : 'rien'}</span>
                  </li>
                {/each}
              </ol>
              <p class="note">Modifier un modèle ne change pas un TP déjà lancé : chaque lancement en fait une copie.</p>
            </section>
          {/if}
        {/if}
      {/if}
    {:else}
      <h2 class="t">Sondages</h2>
      {#if editingPoll}
        <PollEditor poll={editingPoll} onclose={() => (editingPoll = null)} />
      {:else}
        <div class="toolbar">
          <button type="button" class="btn primary" onclick={() => (editingPoll = { id: '', question: '', options: ['', ''], multiple: false })}><Icon name="plus" size={16} />Nouveau sondage</button>
          <button type="button" class="btn ghost" onclick={importFile}>Importer…</button>
        </div>
        {#if lib.polls.length === 0}
          <p class="note">Aucun sondage encore. Un sondage lancé pendant la séance s'enregistre ici, pour être relancé, modifié ou partagé.</p>
        {/if}
        <div class="polls">
          {#each lib.polls as p (p.id)}
            <div class="card static">
              <h3>{p.question}</h3>
              <p>{p.options.join(' · ')}</p>
              <p class="note">{p.multiple ? 'Choix multiple' : 'Choix unique'}</p>
              <div class="row">
                <button type="button" class="btn" onclick={() => (editingPoll = structuredClone($state.snapshot(p)))}>Modifier</button>
                <button type="button" class="btn ghost" onclick={() => exportOne('poll', p.id, p.question.slice(0, 40))}>Exporter…</button>
                {#if confirmDelete === p.id}
                  <button type="button" class="btn danger" onclick={() => remove('poll', p.id)}>Confirmer la suppression</button>
                {:else}
                  <button type="button" class="btn ghost" onclick={() => (confirmDelete = p.id)}>Supprimer…</button>
                {/if}
              </div>
            </div>
          {/each}
        </div>
      {/if}
    {/if}
  </div>
</div>

<style>
  .space { flex: 1; min-height: 0; display: grid; grid-template-columns: 230px minmax(0, 1fr); gap: 10px; }
  nav, .content { background: var(--surface); border: 1px solid var(--line); border-radius: 12px; box-shadow: var(--shadow); }
  nav { padding: 12px; display: flex; flex-direction: column; gap: 4px; align-self: start; }
  nav h2 { margin: 0 0 6px; font-size: 15px; }
  nav button {
    display: flex; align-items: center; gap: 8px; border: 0; background: transparent; border-radius: 8px; padding: 8px 10px;
    text-align: left; cursor: pointer; font-size: 14.5px;
  }
  nav button span { margin-left: auto; color: var(--muted); font-family: var(--mono); font-size: 13px; }
  nav button[aria-current='true'] { background: var(--accent-soft); color: var(--accent); font-weight: 700; }
  .content { min-height: 0; overflow: auto; padding: 16px 18px; display: flex; flex-direction: column; gap: 14px; }
  .t { margin: 0; font-size: 19px; }
  .toolbar { display: flex; gap: 8px; flex-wrap: wrap; }
  .cards, .polls { display: grid; grid-template-columns: repeat(auto-fill, minmax(240px, 1fr)); gap: 10px; }
  .card {
    text-align: left; border: 1px solid var(--line); border-radius: 10px; padding: 10px 12px; background: var(--surface); cursor: pointer;
    display: flex; flex-direction: column; gap: 4px;
  }
  .card.static { cursor: default; }
  .card.cur { border-color: var(--accent); background: var(--accent-soft); }
  .card h3 { margin: 0; font-size: 15px; }
  .card p { margin: 0; font-size: 13px; color: var(--muted); }
  .card .row { display: flex; gap: 6px; flex-wrap: wrap; margin-top: 6px; }
  .detail { display: flex; flex-direction: column; gap: 8px; }
  .dhead { display: flex; align-items: center; gap: 6px; flex-wrap: wrap; }
  .dhead h3 { margin: 0; font-size: 16px; }
  .grow { flex: 1; }
  .confirm { display: inline-flex; align-items: center; gap: 6px; font-size: 14px; }
  .steps { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 6px; }
  .steps li {
    display: grid; grid-template-columns: 30px 1fr auto auto; gap: 10px; align-items: center; border: 1px solid var(--line);
    border-radius: 9px; padding: 8px 10px; font-size: 14px;
  }
  .no { width: 24px; height: 24px; border-radius: 50%; background: var(--surface-2); display: inline-flex; align-items: center; justify-content: center; font-size: 13px; }
  .du { color: var(--muted); font-size: 13px; }
  .ac { color: var(--muted); font-size: 13px; }
  @media (max-width: 1050px) {
    .space { grid-template-columns: 190px minmax(0, 1fr); }
    .steps li { grid-template-columns: 30px 1fr auto; }
    .ac { grid-column: 2 / 4; }
  }
</style>
