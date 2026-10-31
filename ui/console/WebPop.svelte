<script lang="ts">
  // Filtrage web des postes visés (SPEC.md, WEB-1, WEB-3) : liste
  // d'autorisation, liste de blocage ou tout bloquer, à partir d'une liste
  // enregistrée ou de sites saisis ; les listes suivent l'enseignant
  // (WEB-4).
  import type { StationView } from '$shared/bindings/StationView';
  import type { WebPolicy } from '$shared/bindings/WebPolicy';
  import { hhmm } from '$shared/time';
  import { ui, call } from './store.svelte';
  import Pop from './Pop.svelte';

  let { members, stations, scope, onclose }: { members: string[]; stations: StationView[]; scope: string; onclose: () => void } = $props();

  type Kind = 'none' | 'allow' | 'block' | 'all';
  const view = ui.view!;
  const lists = $derived(ui.view!.lists.sites);

  // Le réglage commun des postes visés, s'il y en a un.
  const current: WebPolicy | null = (() => {
    const all = stations.map((s) => JSON.stringify(s.web));
    return all.length && all.every((x) => x === all[0]) ? stations[0].web : null;
  })();
  let kind: Kind = $state(current ? current.t : 'none');
  let text = $state(current && current.t !== 'all' ? current.sites.join('\n') : '');
  let chosen = $state('');
  let name = $state('');
  let saving = $state(false);

  const sites = $derived(text.split(/[\s,;]+/).map((s) => s.trim()).filter(Boolean));

  function pick(e: Event) {
    chosen = (e.currentTarget as HTMLSelectElement).value;
    const l = lists.find((x) => x.name === chosen);
    if (l) {
      text = l.sites.join('\n');
      name = l.name;
    }
  }

  async function save() {
    if (await call('save_site_list', { name, sites })) {
      chosen = name.trim();
      saving = false;
    }
  }

  async function apply() {
    const policy = kind === 'none' ? null : kind === 'all' ? { t: 'all' } : { t: kind, sites };
    if (await call('set_web', { members, policy })) onclose();
  }
</script>

<Pop title={`Filtrage web pour ${scope}`} sub={`Il cesse au plus tard à ${hhmm(view.session!.end_ms)}, fin de la séance. Sans droits d'administration, il ne porte que sur les navigateurs.`} {onclose}>
  <div class="choices" role="radiogroup" aria-label="Réglage">
    <label class="radio"><input type="radio" bind:group={kind} value="none" /><span>Aucun filtrage<small>Accès web habituel du poste.</small></span></label>
    <label class="radio"><input type="radio" bind:group={kind} value="allow" /><span>Liste d'autorisation<small>Seuls les sites indiqués sont accessibles.</small></span></label>
    <label class="radio"><input type="radio" bind:group={kind} value="block" /><span>Liste de blocage<small>Les sites indiqués sont bloqués, le reste est accessible.</small></span></label>
    <label class="radio"><input type="radio" bind:group={kind} value="all" /><span>Tout bloquer<small>Aucun site accessible.</small></span></label>
  </div>

  {#if kind === 'allow' || kind === 'block'}
    {#if lists.length}
      <label class="field">
        Liste enregistrée
        <select value={chosen} onchange={pick}>
          <option value="">— Sites saisis ci-dessous —</option>
          {#each lists as l (l.name)}<option value={l.name}>{l.name} ({l.sites.length} site{l.sites.length > 1 ? 's' : ''})</option>{/each}
        </select>
      </label>
    {/if}
    <label class="field">
      Sites, un par ligne
      <textarea bind:value={text} placeholder={'wikipedia.org\ncisco.com'}></textarea>
      <small>Un site couvre aussi ses sous-domaines : wikipedia.org couvre fr.wikipedia.org.</small>
    </label>
    {#if saving}
      <div class="save">
        <input bind:value={name} aria-label="Nom de la liste" placeholder="Nom de la liste, par exemple « Sites du TP réseau »" />
        <button type="button" class="btn" onclick={save} disabled={!name.trim() || !sites.length}>Enregistrer</button>
      </div>
    {:else}
      <div class="links">
        <button type="button" class="link" onclick={() => (saving = true)} disabled={!sites.length}>Enregistrer cette liste…</button>
        {#if chosen}<button type="button" class="link" onclick={() => { call('delete_site_list', { name: chosen }); chosen = ''; }}>Supprimer « {chosen} »</button>{/if}
      </div>
    {/if}
  {/if}

  <div class="row">
    <button type="button" class="btn ghost" onclick={onclose}>Annuler</button>
    <button type="button" class="btn primary" onclick={apply} disabled={(kind === 'allow' || kind === 'block') && !sites.length}>Appliquer</button>
  </div>
</Pop>

<style>
  .choices { display: flex; flex-direction: column; gap: 6px; }
  .save { display: flex; gap: 6px; }
  .save input { flex: 1; border: 1px solid var(--line); border-radius: 8px; padding: 6px 9px; background: var(--surface); }
  .links { display: flex; gap: 14px; flex-wrap: wrap; }
  .link { background: none; border: 0; padding: 0; color: var(--accent); text-decoration: underline; cursor: pointer; font-size: 13.5px; }
  .link:disabled { color: var(--muted); cursor: default; text-decoration: none; }
</style>
