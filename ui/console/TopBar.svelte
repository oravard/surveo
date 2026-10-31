<script lang="ts">
  // Barre du haut : espaces Préparer et Séance (ESPACE-1), salle choisie
  // (SALLE-1) et conduite de la séance : ouvrir, prolonger, clôturer
  // (SEANCE-1, SEANCE-3, SEANCE-5).
  import Icon from '$shared/Icon.svelte';
  import { hhmm, duration } from '$shared/time';
  import { ui, call, toast, type DialogKind } from './store.svelte';

  let { show }: { show: (d: DialogKind) => void } = $props();

  const view = $derived(ui.view!);
  const session = $derived(view.session);
  const overdue = $derived(session !== null && ui.now >= session.end_ms);

  function chooseRoom(e: Event) {
    const v = (e.currentTarget as HTMLSelectElement).value;
    if (v === '__new') {
      (e.currentTarget as HTMLSelectElement).value = view.room?.id ?? '';
      show('room-new');
    } else {
      call('select_room', { id: v });
    }
  }

  /** Prolonger de 15 min, avec « Annuler » (SEANCE-3, RETOUR-3). */
  async function extend() {
    if (!session) return;
    const before = session.end_ms;
    const end = Math.min(Math.max(before, Date.now()) + view.limits.extend_step_ms, session.max_end_ms);
    if (end <= before) {
      toast({ text: `La séance ne peut pas aller au-delà de ${hhmm(session.max_end_ms)}`, detail: 'Durée maximale atteinte, prolongations comprises.', warning: true });
      return;
    }
    if (!(await call('set_end', { endMs: end }))) return;
    toast({ text: `Séance prolongée jusqu'à ${hhmm(end)}`, detail: null, warning: false, undo: () => call('set_end', { endMs: before }) });
  }
</script>

<header class="topbar">
  <span class="brand"><span class="mark" aria-hidden="true"></span>Surveo</span>
  <nav class="seg" aria-label="Espaces">
    <button type="button" aria-pressed={ui.space === 'preparer'} onclick={() => (ui.space = 'preparer')}><Icon name="book" size={16} />Préparer</button>
    <button type="button" aria-pressed={ui.space === 'seance'} onclick={() => (ui.space = 'seance')}><Icon name="room" size={16} />Séance</button>
  </nav>
  <label class="room">
    <Icon name="room" />
    <span class="sr">Salle</span>
    <select value={view.room?.id ?? ''} onchange={chooseRoom}>
      {#if !view.room}<option value="">Choisir une salle</option>{/if}
      {#each view.rooms as r (r.id)}
        <option value={r.id}>{r.name}</option>
      {/each}
      <option value="__new">Nouvelle salle…</option>
    </select>
  </label>

  <div class="seance">
    {#if session}
      <span class="live" aria-hidden="true"></span>
      <span><b>{session.room_name}</b> · <span class="mono">{hhmm(session.opened_at_ms)} – {hhmm(session.end_ms)}</span></span>
      {#if overdue}
        <span class="late">Heure de fin passée · fin automatique à <span class="mono">{hhmm(session.end_ms + session.margin_ms)}</span></span>
      {:else}
        <span class="note">reste {duration(session.end_ms - ui.now)}</span>
      {/if}
      <button class="btn" onclick={extend} title="Repousser l'heure de fin de 15 minutes"><Icon name="clock" />Prolonger</button>
      <button class="btn danger" onclick={() => show('close')}><Icon name="x" />Clôturer</button>
    {:else}
      <span class="note">Aucune séance ouverte</span>
      <button class="btn primary" onclick={() => show('open')} disabled={!view.room}><Icon name="clock" />Ouvrir une séance</button>
    {/if}
  </div>

  <span class="spacer"></span>
  <span class="who"><b>{view.setup?.teacher}</b> · {view.setup?.etab_name}</span>
</header>

<style>
  .topbar {
    display: flex; flex-wrap: wrap; align-items: center; gap: 8px 14px; background: var(--surface); border: 1px solid var(--line);
    border-radius: 12px; padding: 8px 12px; box-shadow: var(--shadow);
  }
  .brand { display: flex; align-items: center; gap: 8px; font-weight: 700; font-size: 18px; }
  .mark { width: 22px; height: 22px; border-radius: 6px; background: var(--accent); position: relative; }
  .mark::after { content: ''; position: absolute; inset: 6px 6px 9px 6px; border: 2px solid var(--accent-ink); border-radius: 2px; }
  .seg { display: inline-flex; background: var(--surface-2); border: 1px solid var(--line); border-radius: 9px; padding: 2px; gap: 2px; }
  .seg button {
    border: 0; background: transparent; border-radius: 7px; padding: 5px 10px; display: inline-flex; align-items: center; gap: 6px;
    cursor: pointer; font-weight: 700; color: var(--muted); font-size: 14px;
  }
  .seg button[aria-pressed='true'] { background: var(--surface); color: var(--ink); box-shadow: 0 1px 2px rgba(0, 0, 0, 0.12); }
  .room { display: inline-flex; align-items: center; gap: 6px; color: var(--muted); }
  .room select {
    border: 1px solid var(--line); border-radius: 8px; padding: 5px 8px; background: var(--surface-2); color: var(--ink);
    font-weight: 700;
  }
  .seance { display: flex; flex-wrap: wrap; align-items: center; gap: 8px; font-size: 14px; padding-left: 12px; border-left: 1px solid var(--line); }
  .live { width: 9px; height: 9px; border-radius: 50%; background: var(--ok); }
  .late { color: var(--late); font-weight: 700; }
  .spacer { flex: 1; }
  .who { font-size: 13px; color: var(--muted); }
  .who b { color: var(--ink); }
</style>
