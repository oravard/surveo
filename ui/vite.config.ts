// Une même configuration pour les deux interfaces : `--mode console` ou
// `--mode agent` choisit le dossier source et le dossier de sortie
// (ui/dist/<mode>), que lit tauri.conf.json de chaque application.
import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';
import { fileURLToPath } from 'node:url';

export default defineConfig(({ mode }) => {
  const app = mode === 'agent' ? 'agent' : 'console';
  return {
    root: fileURLToPath(new URL(`./${app}`, import.meta.url)),
    base: './',
    plugins: [svelte()],
    resolve: { alias: { $shared: fileURLToPath(new URL('./shared', import.meta.url)) } },
    build: {
      outDir: fileURLToPath(new URL(`./dist/${app}`, import.meta.url)),
      emptyOutDir: true,
      target: 'es2021',
    },
    clearScreen: false,
  };
});
