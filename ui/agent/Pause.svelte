<script lang="ts">
  // Écran en pause (SPEC.md, ECRAN-1, ECRAN-2, ECRAN-4) : il recouvre
  // l'écran avec le message de l'enseignant ; rien n'est fermé dessous.
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import Icon from '$shared/Icon.svelte';

  let message = $state('');
  invoke<string | null>('pause_message').then((m) => (message = m ?? ''));
  listen<string>('pause', (e) => (message = e.payload));
</script>

<main role="alert">
  <Icon name="pause" size={46} />
  <p class="big">{message}</p>
  <p>Votre travail est conservé tel quel.</p>
</main>

<style>
  main {
    height: 100vh; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 14px;
    background: #151a2b; color: #e8ecff; text-align: center; padding: 24px; user-select: none; cursor: default;
  }
  .big { font-size: 30px; font-weight: 700; max-width: 26em; text-wrap: balance; margin: 0; }
  p { margin: 0; font-size: 17px; }
</style>
