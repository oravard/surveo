<script lang="ts">
  // Panneau latéral (SPEC.md, PANNEAU-1) : état de la séance,
  // récapitulatif de la clôture (RESTR-5), relais intégré et connexions
  // refusées (CONF-3).
  import Icon from '$shared/Icon.svelte';
  import { hhmm } from '$shared/time';
  import { ui, type DialogKind } from './store.svelte';

  let { show }: { show: (d: DialogKind) => void } = $props();

  const view = $derived(ui.view!);
  const stations = $derived(view.room?.stations ?? []);
  const counts = $derived({
    in: stations.filter((s) => s.state === 'in_session').length,
    on: stations.filter((s) => s.state === 'on').length,
    off: stations.filter((s) => s.state === 'off').length,
  });
  const closure = $derived(view.closure);
</script>

<aside class="panel" aria-label="Activités de la séance">
  <section class="psec">
    <h3><Icon name="room" size={15} />Postes</h3>
    <ul class="counts">
      <li><span class="mono">{counts.in}</span> dans la séance</li>
      <li><span class="mono">{counts.on}</span> allumé{counts.on > 1 ? 's' : ''}, hors séance</li>
      <li><span class="mono">{counts.off}</span> éteint{counts.off > 1 ? 's' : ''}</li>
    </ul>
    {#if !view.session && counts.on > 0}
      <p class="note">Les postes allumés rejoindront la séance dès qu'elle sera ouverte.</p>
    {/if}
  </section>

  {#if closure}
    <section class="psec">
      <h3><Icon name="check" size={15} />Séance {closure.auto ? 'terminée' : 'clôturée'} à <span class="mono">{hhmm(closure.ended_at_ms)}</span></h3>
      <ul class="recap">
        <li>{closure.restored.length} poste{closure.restored.length > 1 ? 's' : ''} remis dans leur état d'origine.</li>
        {#each closure.pending as p}
          <li class="note">{p} : remise en état en cours…</li>
        {/each}
        {#each closure.unreachable as p}
          <li class="warn">{p} : injoignable, remise en état à son prochain démarrage ou à <span class="mono">{hhmm(closure.restore_by_ms)}</span>.</li>
        {/each}
      </ul>
    </section>
  {/if}

  <section class="psec">
    <h3><Icon name="gear" size={15} />Relais intégré</h3>
    {#if view.relay.error}
      <p class="warn">Le relais n'a pas pu démarrer : {view.relay.error}</p>
    {:else if view.relay.running}
      <p class="note">
        Les postes de la salle trouvent cette console sur le réseau local. Adresse{view.relay.addresses.length > 1 ? 's' : ''} :
        {#each view.relay.addresses as a, i}<span class="mono">{a}:{view.relay.port}</span>{i < view.relay.addresses.length - 1 ? ', ' : ''}{/each}
        {#if view.relay.addresses.length === 0}aucune (pas de réseau){/if}
      </p>
      {#if view.relay.discovery_error}
        <p class="warn">Les postes ne peuvent pas trouver ce relais par recherche : {view.relay.discovery_error}</p>
      {/if}
    {:else}
      <p class="note">Démarrage…</p>
    {/if}
    {#if view.room}
      <button class="btn" onclick={() => show('package')}><Icon name="file" />Paquet d'installation de {view.room.name}</button>
    {/if}
  </section>

  {#if view.refused.length}
    <section class="psec">
      <h3><Icon name="wifioff" size={15} />Connexions refusées</h3>
      <ul class="recap">
        {#each view.refused as r}<li class="note">{r}</li>{/each}
      </ul>
    </section>
  {/if}
</aside>

<style>
  .panel { min-height: 0; overflow: auto; display: flex; flex-direction: column; gap: 8px; }
  .psec {
    background: var(--surface); border: 1px solid var(--line); border-radius: 12px; box-shadow: var(--shadow); padding: 10px 12px;
    display: flex; flex-direction: column; gap: 8px; font-size: 14px;
  }
  h3 { margin: 0; font-size: 14.5px; display: flex; align-items: center; gap: 7px; }
  p { margin: 0; }
  ul { margin: 0; padding-left: 18px; display: flex; flex-direction: column; gap: 3px; }
  .counts { list-style: none; padding: 0; }
  .counts .mono { display: inline-block; min-width: 2.2em; font-weight: 600; }
  .warn { color: var(--late); }
  .psec .btn { align-self: flex-start; }
</style>
