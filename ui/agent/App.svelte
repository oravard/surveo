<script lang="ts">
  // Fenêtres de l'agent, choisies par l'ancre de l'adresse (main.rs) :
  // #indicator (REJ-4), #identify/<numéro> (SALLE-4), #error.
  import Indicator from './Indicator.svelte';
  import Identify from './Identify.svelte';
  import StartupError from './StartupError.svelte';

  const route = location.hash.slice(1);
</script>

{#if route === 'indicator'}
  <Indicator />
{:else if route.startsWith('identify/')}
  <Identify number={decodeURIComponent(route.slice('identify/'.length))} />
{:else}
  <StartupError />
{/if}
