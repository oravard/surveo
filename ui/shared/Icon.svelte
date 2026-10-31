<script lang="ts" module>
  // Icônes de la maquette (maquette/console-surveo.html), tracées au trait.
  const PATHS = {
    clock: '<circle cx="12" cy="12" r="9"/><path d="M12 7v5l3 2"/>',
    power: '<path d="M12 3v8M6.4 6.4a8 8 0 1 0 11.2 0"/>',
    check: '<path d="M5 12.5l4.5 4.5L19 7.5"/>',
    file: '<path d="M14 3H7a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V8zM14 3v5h5"/>',
    x: '<path d="M6 6l12 12M18 6L6 18"/>',
    plus: '<path d="M12 5v14M5 12h14"/>',
    chev: '<path d="M6 9l6 6 6-6"/>',
    target: '<path d="M4 9V5h4M20 9V5h-4M4 15v4h4M20 15v4h-4"/><circle cx="12" cy="12" r="2.5"/>',
    eyeoff:
      '<path d="M3 3l18 18M10.6 6.1A9 9 0 0 1 21 12a14 14 0 0 1-2.3 3M6.6 6.6A14 14 0 0 0 3 12a9 9 0 0 0 12.4 5.4M9.9 9.9a3 3 0 0 0 4.2 4.2"/>',
    room: '<rect x="3" y="4" width="18" height="16" rx="2"/><path d="M7 9h3M14 9h3M7 14h3M14 14h3"/>',
    wifioff:
      '<path d="M3 3l18 18M8.5 16.5a5 5 0 0 1 7 0M5 12.9a10 10 0 0 1 5.2-2.8M19 12.9a10 10 0 0 0-2-1.5M2 8.8a15 15 0 0 1 4.2-2.6M22 8.8A15 15 0 0 0 10.7 5M12 20h.01"/>',
    undo: '<path d="M9 14L4 9l5-5M4 9h10a6 6 0 0 1 0 12h-3"/>',
    gear: '<circle cx="12" cy="12" r="3"/><path d="M12 2v3M12 19v3M4.2 4.2l2.1 2.1M17.7 17.7l2.1 2.1M2 12h3M19 12h3M4.2 19.8l2.1-2.1M17.7 6.3l2.1-2.1"/>',
    minus: '<path d="M5 12h14"/>',
  } as const;
  export type IconName = keyof typeof PATHS;
</script>

<script lang="ts">
  let { name, size = 18 }: { name: IconName; size?: number } = $props();
</script>

<svg class="ic" width={size} height={size} viewBox="0 0 24 24" aria-hidden="true">{@html PATHS[name]}</svg>

<style>
  .ic {
    stroke: currentColor;
    fill: none;
    stroke-width: 1.8;
    stroke-linecap: round;
    stroke-linejoin: round;
    flex: none;
  }
</style>
