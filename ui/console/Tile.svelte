<script lang="ts">
  // Tuile d'un poste (SPEC.md, AFF-2) : numéro, état toujours écrit en
  // toutes lettres (ACCES-4), libellé, restrictions en cours, avancement
  // dans le TP (TP-8), et l'image de l'écran quand l'enseignant affiche
  // les écrans (AFF-4). Un clic sélectionne (SEL-1) ; le menu court porte
  // les actions propres au poste (SEL-5). Un fichier glissé dessus lui est
  // distribué (DISTRIB-1).
  import Icon from '$shared/Icon.svelte';
  import type { StationView } from '$shared/bindings/StationView';
  import { ui, selection, thumbs, call, distributeDropped } from './store.svelte';
  import { outside } from './outside';

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
  let fileOver = $state(false);

  function dropFiles(e: DragEvent) {
    if (!inSession || !e.dataTransfer?.files.length) return;
    e.preventDefault();
    e.stopPropagation();
    fileOver = false;
    distributeDropped(e.dataTransfer.files, [station.member]);
  }

  function overFiles(e: DragEvent) {
    if (inSession && station.state === 'in_session' && e.dataTransfer?.types.includes('Files')) {
      e.preventDefault();
      e.stopPropagation();
      fileOver = true;
    }
  }

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

  /** Restrictions en cours, avec leur icône et leur nom (ACCES-4). */
  const flags = $derived.by(() => {
    const f: { icon: 'pause' | 'globe' | 'appx' | 'wifioff' | 'clock' | 'file' | 'poll'; text: string; warn?: boolean }[] = [];
    if (station.state === 'off') return f;
    if (station.tp?.late) f.push({ icon: 'clock', text: 'En retard sur l’étape', warn: true });
    if (station.pause !== null) f.push({ icon: 'pause', text: 'Écran en pause' });
    if (station.web_state.t === 'escaped') f.push({ icon: 'wifioff', text: 'Sorti du filtrage web', warn: true });
    else if (station.web_state.t === 'unavailable') f.push({ icon: 'wifioff', text: `Filtrage impossible : ${station.web_state.reason}`, warn: true });
    else if (station.web !== null) f.push({ icon: 'globe', text: 'Filtrage web' });
    if (station.banned_apps.length) f.push({ icon: 'appx', text: `Applications interdites : ${station.banned_apps.join(', ')}` });
    if (station.files_received < station.files_total) f.push({ icon: 'file', text: `Fichiers en cours de réception (${station.files_received}/${station.files_total})` });
    if (station.answered) f.push({ icon: 'poll', text: 'A répondu au sondage' });
    return f;
  });

  function identify() {
    menu = false;
    call('identify', { members: [station.member] });
  }

  function openApps() {
    menu = false;
    selection.clear();
    selection.add(station.member);
    ui.pop = 'open-apps';
  }

  function pause() {
    menu = false;
    call('set_pause', { members: [station.member], message: station.pause === null ? '' : null });
  }
</script>

<div class="wrap">
  <button
    type="button"
    class="tile st-{station.state}"
    class:sel={selected}
    class:paused={station.pause !== null}
    class:fileover={fileOver}
    aria-pressed={selected}
    ondragover={overFiles}
    ondragleave={() => (fileOver = false)}
    ondrop={dropFiles}
    draggable="true"
    {ondragstart}
    onclick={click}
    title={[station.label, station.os, station.mode === 'sans_admin' ? 'sans droits d’administration' : station.mode === 'etablissement' ? 'déployé par l’établissement' : null]
      .filter(Boolean)
      .join(' · ')}
  >
    <span class="t-head">
      <span class="t-num mono">{station.number}</span>
      {#if flags.length}
        <span class="t-flags">
          {#each flags as f}<span class="flag" class:warn={f.warn} title={f.text}><Icon name={f.icon} size={12} /><span class="sr">{f.text}</span></span>{/each}
        </span>
      {/if}
    </span>
    <span class="t-label">{station.label}</span>
    <span class="t-state">
      <Icon name={station.state === 'off' ? 'power' : station.state === 'in_session' ? 'check' : 'minus'} size={13} />
      {stateText}
    </span>
    {#if station.tp && station.state !== 'off'}
      <span class="t-foot mono" title={station.tp.objective ? `Étape en cours : ${station.tp.objective}` : 'Toutes les étapes sont terminées'}>
        <span>{station.tp.step === null ? 'Terminé' : `Étape ${station.tp.step + 1}/${station.tp.steps}`}</span>
        {#if station.tp.expected}<span>Rendus {station.tp.rendus}/{station.tp.expected}</span>{/if}
      </span>
    {/if}
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
  <button type="button" class="more" data-pop-toggle aria-label={`Actions du poste ${station.number}`} aria-expanded={menu} onclick={() => (menu = !menu)}>
    <Icon name="chev" size={14} />
  </button>
  {#if menu}
    <div class="menu" role="menu" use:outside={() => (menu = false)}>
      {#if inSession && station.state === 'in_session'}
        <button role="menuitem" onclick={pause}>
          <Icon name={station.pause === null ? 'pause' : 'play'} size={15} />{station.pause === null ? 'Mettre en pause' : 'Reprendre l’écran'}
        </button>
        <button role="menuitem" data-pop-toggle onclick={openApps}><Icon name="appx" size={15} />Applications ouvertes…</button>
      {/if}
      {#if station.tp}
        <button role="menuitem" onclick={() => { menu = false; call('open_rendus', { member: station.member }); }}><Icon name="folder" size={15} />Ouvrir ses rendus</button>
      {/if}
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
  .tile { position: relative; }
  .tile.st-off { background: transparent; border-style: dashed; color: var(--muted); }
  .tile.st-on { background: var(--surface-2); color: var(--muted); }
  .t-head { display: flex; align-items: center; gap: 6px; padding-right: 18px; }
  .t-num { font-weight: 600; font-size: 17px; color: var(--ink); line-height: 1.2; }
  .t-flags { display: flex; gap: 3px; flex-wrap: wrap; }
  .flag { display: inline-flex; align-items: center; padding: 2px 5px; border-radius: 99px; background: var(--restrict-soft); color: var(--restrict); }
  .flag.warn { background: var(--late-soft); color: var(--late); }
  .tile.paused { background: repeating-linear-gradient(135deg, var(--restrict-soft) 0 7px, var(--surface) 7px 14px); }
  .tile.sel.paused { background: repeating-linear-gradient(135deg, var(--restrict-soft) 0 7px, var(--accent-soft) 7px 14px); }
  .st-off .t-num { color: var(--muted); }
  .tile.fileover { border-color: var(--accent); background: var(--accent-soft); }
  .t-foot { display: flex; flex-wrap: wrap; justify-content: space-between; gap: 0 6px; font-size: 11px; color: var(--muted); }
  .t-foot span { white-space: nowrap; }
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
