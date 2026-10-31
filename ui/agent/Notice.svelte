<script lang="ts">
  // Notification discrète à l'élève (SPEC.md, AGENT-4) : site bloqué
  // (WEB-2), application fermée (APP-1). Elle disparaît d'elle-même.
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import Icon from '$shared/Icon.svelte';

  let text = $state('');
  invoke<string | null>('notice_text').then((t) => (text = t ?? ''));
  listen<string>('notice', (e) => (text = e.payload));
</script>

<div class="notice" role="status">
  <Icon name="lock" size={18} />
  <span>{text}</span>
</div>

<style>
  :global(body) { background: transparent; overflow: hidden; }
  .notice {
    height: 100vh; display: flex; align-items: center; gap: 10px; padding: 0 14px; background: var(--bar); color: var(--bar-ink);
    border-radius: 10px; font-size: 14px; line-height: 1.3; user-select: none;
  }
  .notice :global(.ic) { color: #d29922; }
</style>
