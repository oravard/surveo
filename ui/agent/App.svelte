<script lang="ts">
  // Fenêtres de l'agent, choisies par l'ancre de l'adresse (main.rs) :
  // #indicator (REJ-4, MAIT-1), #identify/<numéro> (SALLE-4), #pause
  // (ECRAN-1), #notice (AGENT-4), #panel (AGENT-3), #poll (SONDAGE-2),
  // #error.
  import Indicator from './Indicator.svelte';
  import Identify from './Identify.svelte';
  import Pause from './Pause.svelte';
  import Notice from './Notice.svelte';
  import StartupError from './StartupError.svelte';
  import StudentPanel from './StudentPanel.svelte';
  import PollCard from './PollCard.svelte';

  const route = location.hash.slice(1);
</script>

{#if route === 'indicator'}
  <Indicator />
{:else if route.startsWith('identify/')}
  <Identify number={decodeURIComponent(route.slice('identify/'.length))} />
{:else if route === 'pause'}
  <Pause />
{:else if route === 'notice'}
  <Notice />
{:else if route === 'panel'}
  <StudentPanel />
{:else if route === 'poll'}
  <PollCard />
{:else}
  <StartupError />
{/if}
