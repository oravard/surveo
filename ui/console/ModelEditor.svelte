<script lang="ts">
  // Modifier un modèle de TP (SPEC.md, MODELE-1, MODELE-2) : titre, rendus
  // attendus, fichiers joints, et des étapes ordonnées avec objectif,
  // consignes en Markdown, durée indicative et action en cas de
  // dépassement. L'aide de l'IA viendra avec le chapitre 5.
  import { invoke } from '@tauri-apps/api/core';
  import Icon from '$shared/Icon.svelte';
  import { markdown } from '$shared/markdown';
  import type { Model } from '$shared/bindings/Model';
  import { pickFiles, toast } from './store.svelte';
  import { sizeText } from './peda';

  let { model, onclose }: { model: Model; onclose: (saved: string | null) => void } = $props();

  // Brouillon : rien n'est enregistré avant « Enregistrer ».
  // svelte-ignore state_referenced_locally
  let draft: Model = $state(structuredClone($state.snapshot(model)));
  let preview: number | null = $state(null);
  let saving = $state(false);

  function addStep() {
    draft.steps.push({ objective: '', instructions: '', minutes: 15, overrun: 'nothing' });
  }

  function move(i: number, d: -1 | 1) {
    const [s] = draft.steps.splice(i, 1);
    draft.steps.splice(i + d, 0, s);
  }

  async function attach() {
    const files = await pickFiles('Fichiers joints au TP');
    if (files) draft.files.push(...files.filter((f) => !draft.files.some((x) => x.sha256 === f.sha256 && x.name === f.name)));
  }

  async function submit(e: SubmitEvent) {
    e.preventDefault();
    saving = true;
    try {
      const id = await invoke<string>('save_model', { model: $state.snapshot(draft) });
      toast({ text: `Modèle « ${draft.title.trim()} » enregistré`, detail: null, warning: false });
      onclose(id);
    } catch (err) {
      toast({ text: String(err), detail: null, warning: true }, 8000);
    } finally {
      saving = false;
    }
  }
</script>

<form class="editor" onsubmit={submit}>
  <h2 class="t">{model.id ? 'Modifier le modèle' : 'Nouveau modèle de TP'}</h2>
  <div class="grid">
    <label class="field">Titre
      <input bind:value={draft.title} placeholder="Par exemple « Adressage IPv4 »" required />
    </label>
    <label class="field small">Rendus attendus
      <input type="number" min="0" max="20" bind:value={draft.expected} />
      <small>Un repère : l'élève peut en rendre davantage.</small>
    </label>
  </div>

  <section>
    <h3>Fichiers joints</h3>
    <p class="note">Distribués aux postes au lancement du TP, et retirés des postes de salle à la fin de la séance.</p>
    {#each draft.files as f, i (f.sha256 + f.name)}
      <div class="file"><Icon name="file" size={16} /><span class="grow">{f.name}</span><span class="note">{sizeText(f.size)}</span>
        <button type="button" class="btn ghost" aria-label={`Retirer ${f.name}`} onclick={() => draft.files.splice(i, 1)}><Icon name="x" size={14} /></button>
      </div>
    {/each}
    <div><button type="button" class="btn" onclick={attach}><Icon name="plus" size={15} />Joindre des fichiers…</button></div>
  </section>

  <section>
    <h3>Étapes</h3>
    {#each draft.steps as s, i}
      <fieldset class="step">
        <legend class="mono">Étape {i + 1}</legend>
        <div class="srow">
          <label class="field grow">Objectif
            <input bind:value={s.objective} placeholder="Ce que l'élève doit atteindre, en quelques mots" required />
          </label>
          <label class="field small">Durée (min)
            <input type="number" min="0" max="240" bind:value={s.minutes} title="0 : sans durée" />
          </label>
          <label class="field wide">Si dépassement
            <select bind:value={s.overrun}>
              <option value="nothing">Rien</option>
              <option value="show_instructions">Afficher les consignes</option>
            </select>
          </label>
        </div>
        <div class="field">
          <span class="lbl">Consignes
            <button type="button" class="link" onclick={() => (preview = preview === i ? null : i)}>{preview === i ? 'Modifier' : 'Aperçu'}</button>
          </span>
          {#if preview === i}
            <div class="md preview">{@html markdown(s.instructions) || '<p class="note">Aucune consigne.</p>'}</div>
          {:else}
            <textarea bind:value={s.instructions} rows="5" placeholder={'## Titre\n**gras**, *italique*, `code`\n- une liste'}></textarea>
          {/if}
        </div>
        <div class="row">
          <button type="button" class="btn ghost" onclick={() => move(i, -1)} disabled={i === 0} aria-label="Monter l'étape"><Icon name="up" size={15} /></button>
          <button type="button" class="btn ghost" onclick={() => move(i, 1)} disabled={i === draft.steps.length - 1} aria-label="Descendre l'étape"><Icon name="chev" size={15} /></button>
          <button type="button" class="btn ghost" onclick={() => draft.steps.splice(i, 1)} disabled={draft.steps.length === 1}>Retirer l'étape</button>
        </div>
      </fieldset>
    {/each}
    <div><button type="button" class="btn" onclick={addStep}><Icon name="plus" size={15} />Ajouter une étape</button></div>
  </section>

  <div class="foot">
    <button type="button" class="btn ghost" onclick={() => onclose(null)}>Annuler</button>
    <button class="btn primary" disabled={saving}>Enregistrer le modèle</button>
  </div>
</form>

<style>
  .editor { display: flex; flex-direction: column; gap: 16px; max-width: 860px; }
  .t { margin: 0; font-size: 19px; }
  h3 { margin: 0 0 4px; font-size: 15.5px; }
  section { display: flex; flex-direction: column; gap: 8px; }
  section p { margin: 0; }
  .grid { display: grid; grid-template-columns: 1fr 170px; gap: 12px; }
  .grow { flex: 1; }
  .small { width: 150px; }
  .wide { width: 220px; }
  .file { display: flex; align-items: center; gap: 8px; border: 1px solid var(--line); border-radius: 8px; padding: 4px 4px 4px 10px; font-size: 14px; }
  .step { border: 1px solid var(--line); border-radius: 10px; padding: 10px 12px 12px; display: flex; flex-direction: column; gap: 10px; margin: 0; }
  legend { font-size: 13px; color: var(--muted); padding: 0 4px; }
  .srow { display: flex; gap: 10px; flex-wrap: wrap; align-items: flex-start; }
  .lbl { display: flex; align-items: baseline; gap: 10px; }
  textarea {
    border: 1px solid var(--line); border-radius: 8px; padding: 7px 10px; background: var(--surface); font: inherit; color: inherit;
    font-family: var(--mono); font-size: 13.5px; resize: vertical;
  }
  .preview { border: 1px dashed var(--line); border-radius: 8px; padding: 8px 12px; min-height: 5em; }
  .link { background: none; border: 0; padding: 0; color: var(--accent); text-decoration: underline; cursor: pointer; font-size: 13px; }
  .row { display: flex; gap: 4px; justify-content: flex-end; }
  .foot { display: flex; gap: 8px; justify-content: flex-end; position: sticky; bottom: -16px; background: var(--surface); padding: 10px 0; border-top: 1px solid var(--line); }
  @media (max-width: 1050px) { .grid { grid-template-columns: 1fr; } }
</style>
