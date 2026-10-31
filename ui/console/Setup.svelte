<script lang="ts">
  // Premier lancement : créer un établissement, localement et sans compte
  // (SPEC.md, CONF-1, AUTO-1). Rejoindre un établissement existant par un
  // code d'appairage viendra avec le chapitre 6.
  import { call } from './store.svelte';

  let etabName = $state('');
  let teacher = $state('');
  let busy = $state(false);

  async function submit(e: SubmitEvent) {
    e.preventDefault();
    busy = true;
    await call('create_establishment', { etabName, teacher });
    busy = false;
  }
</script>

<main>
  <form class="card" onsubmit={submit}>
    <h1><span class="mark" aria-hidden="true"></span>Bienvenue dans Surveo</h1>
    <p class="note">
      Cette console va créer votre établissement : une clé propre, créée sur ce poste, sans compte ni connexion à Internet.
      Les postes élèves que vous installerez ensuite n'obéiront qu'à elle.
    </p>
    <label class="field">
      Établissement
      <input bind:value={etabName} required placeholder="Lycée Jean Moulin, ou simplement votre nom" autocomplete="off" />
    </label>
    <label class="field">
      Votre nom
      <input bind:value={teacher} required placeholder="M. Durand" autocomplete="off" />
      <small>Les élèves le verront sur leur poste pendant la séance : « Ce poste est relié à la séance de … ».</small>
    </label>
    <div class="row"><button class="btn primary" disabled={busy}>Créer l'établissement</button></div>
  </form>
</main>

<style>
  main { min-height: 100vh; display: flex; align-items: center; justify-content: center; padding: 16px; }
  .card {
    width: min(520px, 100%); background: var(--surface); border: 1px solid var(--line); border-radius: 14px;
    box-shadow: var(--shadow); padding: 22px; display: flex; flex-direction: column; gap: 14px;
  }
  h1 { margin: 0; font-size: 21px; display: flex; align-items: center; gap: 10px; }
  .mark { width: 22px; height: 22px; border-radius: 6px; background: var(--accent); }
  p { margin: 0; }
  .row { display: flex; justify-content: flex-end; }
</style>
