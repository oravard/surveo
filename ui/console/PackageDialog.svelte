<script lang="ts">
  // Paquet d'installation d'une salle (SPEC.md, INST-3) : un fichier de
  // rattachement à copier, avec l'agent, sur chaque poste de la salle.
  import { invoke } from '@tauri-apps/api/core';
  import { save } from '@tauri-apps/plugin-dialog';
  import { ui, call, toast } from './store.svelte';
  import Dialog from './Dialog.svelte';

  let { onclose }: { onclose: () => void } = $props();
  const room = ui.view!.room!;

  async function exportPackage() {
    const name = await invoke<string>('package_file_name');
    const path = await save({ defaultPath: name, filters: [{ name: 'Rattachement Surveo', extensions: ['json'] }] });
    if (!path) return;
    if (await call('export_package', { roomId: room.id, path })) {
      toast({ text: `Paquet de ${room.name} enregistré`, detail: path, warning: false }, 9000);
      onclose();
    }
  }
</script>

<Dialog title={`Paquet d'installation de ${room.name}`} {onclose}>
  <p>
    Ce fichier rattache les postes à votre établissement et à la salle {room.name}, sans aucune saisie. Copiez-le, sous le nom
    <b class="mono">surveo-rattachement.json</b>, à côté de l'agent (<span class="mono">surveo-agent</span>) sur chaque poste,
    puis lancez l'agent : il s'installe dans le compte de l'élève, sans droits d'administration, et démarre à chaque ouverture de session.
  </p>
  <p>Gardez ce fichier pour vous : il permet d'ajouter des postes à la salle.</p>
  <div class="row">
    <button type="button" class="btn ghost" onclick={onclose}>Annuler</button>
    <button type="button" class="btn primary" onclick={exportPackage}>Enregistrer le paquet…</button>
  </div>
</Dialog>
