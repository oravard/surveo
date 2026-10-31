<script lang="ts">
  // Plan de la salle (SPEC.md, AFF-1, SALLE-2, SALLE-3) : une tuile par
  // poste, placée comme dans la salle réelle ; les nouveaux postes
  // attendent dans « postes à placer ». On place un poste en le glissant,
  // ou au clavier : « Placer » puis la place choisie.
  import Icon from '$shared/Icon.svelte';
  import type { StationView } from '$shared/bindings/StationView';
  import { ui, call, type DialogKind } from './store.svelte';
  import Tile from './Tile.svelte';

  let { show }: { show: (d: DialogKind) => void } = $props();

  const view = $derived(ui.view!);
  const room = $derived(view.room!);
  const session = $derived(view.session && view.session.room_id === room.id ? view.session : null);
  const placed = $derived(new Map(room.stations.filter((s) => s.pos).map((s) => [`${s.pos!.col},${s.pos!.row}`, s])));
  const unplaced = $derived(room.stations.filter((s) => !s.pos));
  const cells = $derived(
    Array.from({ length: room.rows }, (_, row) => Array.from({ length: room.cols }, (_, col) => ({ col, row }))).flat(),
  );
  const placingStation = $derived(room.stations.find((s) => s.member === ui.placing));

  let dragOver: string | null = $state(null);

  function placeAt(member: string, col: number, row: number) {
    ui.placing = null;
    call('place', { member, col, row });
  }

  function drop(e: DragEvent, col: number, row: number) {
    e.preventDefault();
    dragOver = null;
    const member = e.dataTransfer?.getData('text/x-surveo-poste');
    if (member) placeAt(member, col, row);
  }

  function dropUnplaced(e: DragEvent) {
    e.preventDefault();
    dragOver = null;
    const member = e.dataTransfer?.getData('text/x-surveo-poste');
    const s = room.stations.find((x) => x.member === member);
    if (member && s?.pos) call('unplace', { member });
  }

  function allowDrop(e: DragEvent, key: string) {
    if (e.dataTransfer?.types.includes('text/x-surveo-poste')) {
      e.preventDefault();
      dragOver = key;
    }
  }

  function startDrag(e: DragEvent, s: StationView) {
    e.dataTransfer?.setData('text/x-surveo-poste', s.member);
    if (e.dataTransfer) e.dataTransfer.effectAllowed = 'move';
  }
</script>

<section class="stage" aria-label={`Salle ${room.name}`}>
  <div class="stage-head">
    <b>{room.name}</b>
    <span class="hint">
      {room.stations.filter((s) => s.state !== 'off').length} allumé(s) sur {room.stations.length} poste(s) connu(s)
    </span>
    <span class="spacer"></span>
    {#if session}
      <label class="switch" title="Les écrans ne sont pas affichés par défaut (AFF-4)">
        <input type="checkbox" checked={session.screens_on} onchange={(e) => call('set_screens', { on: e.currentTarget.checked })} />
        Afficher les écrans
      </label>
    {/if}
    <button class="btn ghost" onclick={() => show('room-edit')}><Icon name="gear" />Plan</button>
    <button class="btn ghost" onclick={() => show('package')}><Icon name="file" />Paquet d'installation</button>
  </div>

  <div class="stage-body">
    {#if ui.placing && placingStation}
      <p class="placing-hint" role="status">
        Choisissez la place du poste <b class="mono">{placingStation.number}</b> ({placingStation.label}) · Échap pour annuler
      </p>
    {/if}
    <div class="front"><div class="board">Tableau</div></div>
    <div class="plan" class:placing={ui.placing} style:grid-template-columns={`repeat(${room.cols}, minmax(108px, var(--tw)))`}>
      {#each cells as c (`${c.col},${c.row}`)}
        {@const key = `${c.col},${c.row}`}
        {@const s = placed.get(key)}
        <div
          class="cell"
          class:over={dragOver === key}
          role="gridcell"
          tabindex="-1"
          ondragover={(e) => allowDrop(e, key)}
          ondragleave={() => (dragOver = null)}
          ondrop={(e) => drop(e, c.col, c.row)}
        >
          {#if s}
            <Tile station={s} inSession={session !== null} screensOn={session?.screens_on ?? false} ondragstart={(e) => startDrag(e, s)}
              onplacehere={ui.placing && ui.placing !== s.member ? () => placeAt(ui.placing!, c.col, c.row) : undefined} />
          {:else if ui.placing}
            <button class="slot" onclick={() => placeAt(ui.placing!, c.col, c.row)}>Placer ici</button>
          {:else}
            <div class="slot" aria-hidden="true"></div>
          {/if}
        </div>
      {/each}
    </div>

    <div
      class="unplaced"
      class:over={dragOver === 'unplaced'}
      role="region"
      aria-label="Postes à placer"
      ondragover={(e) => allowDrop(e, 'unplaced')}
      ondragleave={() => (dragOver = null)}
      ondrop={dropUnplaced}
    >
      <span class="u-title"><Icon name="plus" size={14} /><b>Postes à placer</b> ({unplaced.length})</span>
      {#if unplaced.length === 0}
        <span class="note">
          {room.stations.length === 0
            ? "Aucun poste encore : installez l'agent sur les postes avec le paquet d'installation de la salle."
            : 'Tous les postes connus sont placés. Glissez une tuile ici pour la retirer du plan.'}
        </span>
      {/if}
      {#each unplaced as s (s.member)}
        <button
          class="chip"
          class:active={ui.placing === s.member}
          draggable="true"
          ondragstart={(e) => startDrag(e, s)}
          onclick={() => (ui.placing = ui.placing === s.member ? null : s.member)}
          title="Glisser sur le plan, ou cliquer puis choisir la place"
        >
          <span class="mono">{s.number}</span> {s.label}
          <span class="st st-{s.state}">{s.state === 'off' ? 'éteint' : s.state === 'on' ? 'allumé' : 'dans la séance'}</span>
        </button>
      {/each}
    </div>
  </div>
</section>

<style>
  .stage {
    min-height: 0; background: var(--surface); border: 1px solid var(--line); border-radius: 12px; display: flex;
    flex-direction: column; box-shadow: var(--shadow); --tw: 150px;
  }
  .stage-head { display: flex; flex-wrap: wrap; align-items: center; gap: 8px 12px; padding: 10px 12px; border-bottom: 1px solid var(--line); }
  .hint { font-size: 13px; color: var(--muted); }
  .spacer { flex: 1; }
  .switch { display: inline-flex; align-items: center; gap: 7px; font-size: 14px; cursor: pointer; color: var(--muted); }
  .switch input { appearance: none; width: 32px; height: 18px; border-radius: 99px; background: var(--line); position: relative; cursor: pointer; margin: 0; }
  .switch input::after { content: ''; position: absolute; top: 2px; left: 2px; width: 14px; height: 14px; border-radius: 50%; background: var(--surface); transition: left 0.15s; }
  .switch input:checked { background: var(--accent); }
  .switch input:checked::after { left: 16px; }
  .stage-body { flex: 1; min-height: 0; overflow: auto; padding: 14px; }
  .placing-hint { margin: 0 auto 10px; width: max-content; max-width: 100%; background: var(--accent-soft); color: var(--accent); padding: 6px 12px; border-radius: 8px; font-size: 14px; }
  .front { display: flex; justify-content: center; margin-bottom: 14px; }
  .board {
    width: min(520px, 100%); text-align: center; font-size: 11.5px; letter-spacing: 0.14em; text-transform: uppercase;
    color: var(--muted); border-bottom: 3px solid var(--line); padding-bottom: 4px;
  }
  .plan { display: grid; gap: 12px; width: fit-content; max-width: 100%; margin-inline: auto; }
  .cell { min-height: 104px; display: flex; }
  .cell > :global(*) { flex: 1; }
  .slot {
    min-height: 104px; border: 1.5px dashed var(--line); border-radius: 9px; display: flex; align-items: center;
    justify-content: center; font-size: 12px; color: var(--muted); background: transparent;
  }
  .placing .slot { border-color: var(--accent); color: var(--accent); cursor: pointer; background: var(--accent-soft); font-weight: 700; }
  .cell.over .slot, .cell.over :global(.tile) { border-color: var(--accent); background: var(--accent-soft); }
  .unplaced {
    margin: 18px auto 0; display: flex; flex-wrap: wrap; align-items: center; gap: 8px 10px; padding: 10px 12px;
    border: 1.5px dashed var(--line); border-radius: 10px; font-size: 14px; width: max-content; max-width: 100%;
  }
  .unplaced.over { border-color: var(--accent); background: var(--accent-soft); }
  .u-title { display: inline-flex; align-items: center; gap: 6px; }
  .chip {
    border: 1px solid var(--line); background: var(--surface); border-radius: 99px; padding: 4px 11px; font-size: 13.5px;
    cursor: grab; display: inline-flex; align-items: center; gap: 6px;
  }
  .chip.active { border-color: var(--accent); background: var(--accent-soft); color: var(--accent); }
  .st { font-size: 12px; color: var(--muted); }
  .st-in_session { color: var(--ok); font-weight: 700; }
</style>
