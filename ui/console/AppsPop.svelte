<script lang="ts">
  // Applications interdites des postes visés (SPEC.md, APP-2 à APP-5) :
  // nommées par l'enseignant, fermées dès qu'elles s'ouvrent, jusqu'à la
  // fin de la séance ; les listes le suivent (APP-4).
  import type { StationView } from '$shared/bindings/StationView';
  import { hhmm } from '$shared/time';
  import { ui, call } from './store.svelte';
  import Pop from './Pop.svelte';

  let { members, stations, scope, onclose }: { members: string[]; stations: StationView[]; scope: string; onclose: () => void } = $props();

  const view = ui.view!;
  const lists = $derived(ui.view!.lists.apps);
  const current = (() => {
    const all = stations.map((s) => s.banned_apps.join('\n'));
    return all.length && all.every((x) => x === all[0]) ? all[0] : '';
  })();
  let text = $state(current);
  let chosen = $state('');
  let name = $state('');
  let saving = $state(false);
  const apps = $derived(text.split(/[\n,;]+/).map((s) => s.trim()).filter(Boolean));

  function pick(e: Event) {
    chosen = (e.currentTarget as HTMLSelectElement).value;
    const l = lists.find((x) => x.name === chosen);
    if (l) {
      text = l.apps.join('\n');
      name = l.name;
    }
  }

  async function save() {
    if (await call('save_app_list', { name, apps })) {
      chosen = name.trim();
      saving = false;
    }
  }

  async function apply(list: string[]) {
    if (await call('set_banned_apps', { members, apps: list })) onclose();
  }
</script>

<Pop title={`Applications interdites pour ${scope}`} sub={`Une application nommée ici est fermée dès qu'elle s'ouvre, et l'élève en est prévenu, jusqu'à ${hhmm(view.session!.end_ms)}. Les programmes du système ne sont jamais fermés.`} {onclose}>
  {#if lists.length}
    <label class="field">
      Liste enregistrée
      <select value={chosen} onchange={pick}>
        <option value="">— Applications saisies ci-dessous —</option>
        {#each lists as l (l.name)}<option value={l.name}>{l.name} ({l.apps.join(', ')})</option>{/each}
      </select>
    </label>
  {/if}
  <label class="field">
    Applications, une par ligne
    <textarea bind:value={text} placeholder={'Minecraft\nSteam\nRoblox'}></textarea>
    <small>Le nom de l'application (« Minecraft ») ou de son programme (« steam.exe »).</small>
  </label>
  {#if saving}
    <div class="save">
      <input bind:value={name} aria-label="Nom de la liste" placeholder="Nom de la liste, par exemple « Jeux »" />
      <button type="button" class="btn" onclick={save} disabled={!name.trim() || !apps.length}>Enregistrer</button>
    </div>
  {:else}
    <div class="links">
      <button type="button" class="link" onclick={() => (saving = true)} disabled={!apps.length}>Enregistrer cette liste…</button>
      {#if chosen}<button type="button" class="link" onclick={() => { call('delete_app_list', { name: chosen }); chosen = ''; }}>Supprimer « {chosen} »</button>{/if}
    </div>
  {/if}
  <div class="row">
    {#if current}<button type="button" class="btn" onclick={() => apply([])}>Lever l'interdiction</button>{/if}
    <span class="grow"></span>
    <button type="button" class="btn ghost" onclick={onclose}>Annuler</button>
    <button type="button" class="btn primary" onclick={() => apply(apps)} disabled={!apps.length}>Interdire</button>
  </div>
</Pop>

<style>
  .save { display: flex; gap: 6px; }
  .save input { flex: 1; border: 1px solid var(--line); border-radius: 8px; padding: 6px 9px; background: var(--surface); }
  .links { display: flex; gap: 14px; flex-wrap: wrap; }
  .link { background: none; border: 0; padding: 0; color: var(--accent); text-decoration: underline; cursor: pointer; font-size: 13.5px; }
  .link:disabled { color: var(--muted); cursor: default; text-decoration: none; }
  .grow { flex: 1; }
</style>
