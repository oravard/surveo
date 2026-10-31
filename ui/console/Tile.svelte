<script lang="ts">
  // Tuile d'un poste (SPEC.md, AFF-2) : numéro, état toujours écrit en
  // toutes lettres (ACCES-4), libellé, et l'image de l'écran quand
  // l'enseignant affiche les écrans (AFF-4). Un clic sélectionne (SEL-1) ;
  // le menu court porte les actions propres au poste (SEL-5).
  import Icon from '$shared/Icon.svelte';
  import type { StationView } from '$shared/bindings/StationView';
  import { ui, selection, thumbs, call } from './store.svelte';

  let {
    station,
    inSession,
    screensOn,
    ondragstart,
    onplacehere,
  }: {
    station: StationView;
    inSession: boolean;
    screensOn: boolean;
    ondragstart: (e: DragEvent) => void;
    onplacehere?: () => void;
  } = $props();

  let menu = $state(false);

  const selected = $derived(selection.has(station.member));
  const stateText = $derived(
    station.state === 'off' ? 'Éteint' : station.state === 'in_session' ? 'Dans la séance' : inSession ? 'Hors séance' : 'Allumé',
  );
  const thumb = $derived(thumbs.get(station.member));
  const showScreen = $derived(screensOn && station.state === 'in_session');

  function click() {
    if (onplacehere) return onplacehere();
    if (selected) selection.delete(station.member);
    else selection.add(station.member);
  }

  function identify() {
    menu = false;
    call('identify', { members: [station.member] });
  }
</script>

<div class="wrap">
  <button
    type="button"
    class="tile st-{station.state}"
    class:sel={selected}
    aria-pressed={selected}
    draggable="true"
    {ondragstart}
    onclick={click}
    title={station.os ? `${station.label} · ${station.os}` : station.label}
  >
    <span class="t-num mono">{station.number}</span>
    <span class="t-label">{station.label}</span>
    <span class="t-state">
      <Icon name={station.state === 'off' ? 'power' : station.state === 'in_session' ? 'check' : 'minus'} size={13} />
      {stateText}
    </span>
    {#if showScreen}
      {#if station.screen.t === 'unavailable'}
        <span class="masked"><Icon name="eyeoff" size={14} />Écran non visible : {station.screen.reason}</span>
      {:else if thumb}
        <img src={`data:image/jpeg;base64,${thumb}`} alt={`Écran du poste ${station.number}`} />
      {:else}
        <span class="masked">Image de l'écran en attente…</span>
      {/if}
    {/if}
    {#if onplacehere}<span class="swap">Échanger les places</span>{/if}
  </button>
  <button type="button" class="more" aria-label={`Actions du poste ${station.number}`} aria-expanded={menu} onclick={() => (menu = !menu)}>
    <Icon name="chev" size={14} />
  </button>
  {#if menu}
    <div class="menu" role="menu">
      <button role="menuitem" onclick={identify} disabled={station.state === 'off'}><Icon name="target" size={15} />Identifier</button>
      <button role="menuitem" onclick={() => { menu = false; ui.placing = station.member; }}><Icon name="room" size={15} />Déplacer</button>
      <button role="menuitem" onclick={() => { menu = false; call('unplace', { member: station.member }); }}><Icon name="x" size={15} />Retirer du plan</button>
    </div>
  {/if}
</div>

<style>
  .wrap { position: relative; display: flex; }
  .tile {
    flex: 1; min-height: 104px; display: flex; flex-direction: column; gap: 5px; padding: 8px 9px; border-radius: 9px;
    background: var(--surface); border: 1px solid var(--line); text-align: left; cursor: pointer; transition: border-color 0.12s;
    font-size: 13px; min-width: 0;
  }
  .tile:hover { border-color: var(--muted); }
  .tile.sel { outline: 2.5px solid var(--accent); outline-offset: 1px; background: var(--accent-soft); }
  .tile.st-off { background: transparent; border-style: dashed; color: var(--muted); }
  .tile.st-on { background: var(--surface-2); color: var(--muted); }
  .t-num { font-weight: 600; font-size: 17px; color: var(--ink); line-height: 1.2; }
  .st-off .t-num { color: var(--muted); }
  .t-state { display: inline-flex; align-items: center; gap: 4px; font-size: 12px; font-weight: 700; margin-top: auto; }
  .st-in_session .t-state { color: var(--ok); }
  .t-label { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; padding-right: 14px; }
  img { width: 100%; aspect-ratio: 16 / 9; object-fit: cover; border-radius: 5px; display: block; background: var(--surface-2); }
  .masked {
    aspect-ratio: 16 / 9; border-radius: 5px; border: 1px dashed var(--line); display: flex; align-items: center; justify-content: center;
    gap: 4px; color: var(--muted); font-size: 11.5px; text-align: center; padding: 4px; background: var(--surface-2);
  }
  .swap { font-size: 12px; color: var(--accent); font-weight: 700; }
  .more {
    position: absolute; top: 5px; right: 5px; width: 22px; height: 22px; border-radius: 6px; border: 0; background: transparent;
    color: var(--muted); cursor: pointer; display: inline-flex; align-items: center; justify-content: center;
  }
  .more:hover { background: var(--surface-2); color: var(--ink); }
  .menu {
    position: absolute; top: 30px; right: 4px; z-index: 20; background: var(--surface); border: 1px solid var(--line);
    border-radius: 10px; box-shadow: 0 10px 30px rgba(0, 0, 0, 0.2); padding: 4px; display: flex; flex-direction: column; min-width: 170px;
  }
  .menu button {
    display: flex; align-items: center; gap: 8px; border: 0; background: transparent; padding: 7px 9px; border-radius: 7px;
    cursor: pointer; text-align: left; font-size: 14px;
  }
  .menu button:hover:not(:disabled) { background: var(--surface-2); }
  .menu button:disabled { opacity: 0.5; cursor: default; }
</style>
