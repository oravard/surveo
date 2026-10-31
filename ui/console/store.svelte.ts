// État de l'interface : l'état complet publié par le cœur Rust
// (événement `state`), les dernières miniatures (`thumb`), les messages
// passagers (`notice`, plus ceux de l'interface), la sélection et
// l'horloge.
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import { listen } from '@tauri-apps/api/event';
import { SvelteMap, SvelteSet } from 'svelte/reactivity';
import type { ConsoleView } from '$shared/bindings/ConsoleView';
import type { Notice } from '$shared/bindings/Notice';
import type { Thumb } from '$shared/bindings/Thumb';
import type { StationView } from '$shared/bindings/StationView';
import type { Attachment } from '$shared/bindings/Attachment';

export type Toast = Omit<Notice, 'undo'> & { id: number; undo?: () => void };

/** Dialogue ouvert dans la console. */
export type DialogKind = 'open' | 'close' | 'room-new' | 'room-edit' | 'package' | null;

/** Fenêtre d'action ouverte au-dessus de la barre d'actions. */
export type PopKind = 'pause' | 'more' | 'web' | 'apps' | 'open-apps' | 'exam' | 'tp' | 'file' | 'poll' | null;

/** Espace de la console (SPEC.md, ESPACE-1) : Administrer viendra avec le
 * chapitre 6. */
export type Space = 'seance' | 'preparer';

export const ui = $state({
  view: null as ConsoleView | null,
  now: Date.now(),
  toasts: [] as Toast[],
  /** Poste en cours de placement sur le plan (SALLE-3). */
  placing: null as string | null,
  pop: null as PopKind,
  /** Séance à l'ouverture (ESPACE-2). */
  space: 'seance' as Space,
});

/** Postes sélectionnés (SEL-1). */
export const selection = new SvelteSet<string>();

/** Postes visés par une action : la sélection, ou toute la salle (SEL-2),
 * auquel cas le cœur reçoit une liste vide. */
export function targets(): { members: string[]; stations: StationView[]; all: boolean } {
  const stations = ui.view?.room?.stations ?? [];
  if (selection.size === 0) return { members: [], stations: stations.filter((s) => s.state === 'in_session'), all: true };
  return { members: [...selection], stations: stations.filter((s) => selection.has(s.member)), all: false };
}
/** Fait choisir des fichiers à l'enseignant, puis les range dans le
 * magasin de la console ; rend leur description, ou rien. */
export async function pickFiles(title: string): Promise<Attachment[] | null> {
  const paths = await open({ multiple: true, directory: false, title });
  if (!paths || paths.length === 0) return null;
  try {
    return await invoke<Attachment[]>('add_files', { paths });
  } catch (e) {
    toast({ text: String(e), detail: null, warning: true }, 8000);
    return null;
  }
}

/** Fichier glissé depuis le bureau, distribué aux postes visés
 * (DISTRIB-1). */
export async function distributeDropped(files: FileList, members: string[]) {
  for (const f of Array.from(files)) {
    try {
      const data = new Uint8Array(await f.arrayBuffer());
      await invoke('distribute_dropped', data, { headers: { 'x-name': encodeURIComponent(f.name), 'x-members': JSON.stringify(members) } });
    } catch (e) {
      toast({ text: String(e), detail: null, warning: true }, 8000);
    }
  }
}

/** Dernière miniature de chaque poste, en base 64 (AFF-4). */
export const thumbs = new SvelteMap<string, string>();

let nextToast = 1;

export function toast(t: Omit<Toast, 'id'>, ms = 6000) {
  const id = nextToast++;
  ui.toasts.push({ ...t, id });
  setTimeout(() => dismiss(id), ms);
}

export function dismiss(id: number) {
  ui.toasts = ui.toasts.filter((t) => t.id !== id);
}

/** Geste de l'enseignant transmis au cœur ; rend `false` en cas d'échec,
 * après en avoir montré la raison, en français (RETOUR-5). */
export async function call(cmd: string, args?: Record<string, unknown>): Promise<boolean> {
  try {
    await invoke(cmd, args);
    return true;
  } catch (e) {
    toast({ text: String(e), detail: null, warning: true }, 8000);
    return false;
  }
}

export async function start() {
  await listen<ConsoleView>('state', (e) => {
    ui.view = e.payload;
    const known = new Set(e.payload.room?.stations.map((s) => s.member) ?? []);
    for (const m of selection) if (!known.has(m)) selection.delete(m);
  });
  await listen<Thumb>('thumb', (e) => thumbs.set(e.payload.member, e.payload.data));
  await listen<Notice>('notice', (e) => {
    const { undo, ...n } = e.payload;
    // « Annuler » pendant quelques secondes (RETOUR-3).
    toast({ ...n, undo: undo === null ? undefined : () => call('undo', { id: undo }) }, n.warning ? 9000 : 8000);
  });
  ui.view = await invoke<ConsoleView>('state');
  setInterval(() => (ui.now = Date.now()), 15000);
}
