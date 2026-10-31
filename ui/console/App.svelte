<script lang="ts">
  import { ui, start } from './store.svelte';
  import Setup from './Setup.svelte';
  import Shell from './Shell.svelte';
  import Toasts from './Toasts.svelte';

  start();
</script>

{#if ui.view}
  {#if ui.view.setup}
    <Shell />
  {:else}
    <Setup />
  {/if}
{/if}
<Toasts />
