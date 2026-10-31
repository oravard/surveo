<script lang="ts">
  // L'agent n'a pas pu démarrer (rattachement manquant) : seul cas où il
  // ouvre une fenêtre hors séance.
  import { invoke } from '@tauri-apps/api/core';

  let text = $state('');
  invoke<string | null>('startup_error').then((t) => (text = t ?? ''));
</script>

<main>
  <h1>Surveo ne peut pas démarrer</h1>
  <p>{text}</p>
</main>

<style>
  main { padding: 18px 20px; display: flex; flex-direction: column; gap: 8px; }
  h1 { margin: 0; font-size: 18px; }
  p { margin: 0; color: var(--muted); }
</style>
