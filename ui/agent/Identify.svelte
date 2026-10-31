<script lang="ts">
  // Numéro du poste, le même que sur sa tuile dans la console (SALLE-4).
  let { number }: { number: string } = $props();
</script>

<main>
  <div class="num mono">{number}</div>
  <p>Numéro de ce poste dans la console de l'enseignant</p>
</main>

<style>
  main {
    height: 100vh; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 6px;
    background: var(--accent); color: var(--accent-ink); text-align: center; padding: 12px;
  }
  .num { font-size: 150px; font-weight: 600; line-height: 1; }
  p { margin: 0; font-size: 15px; }
</style>
