// Ferme un menu ou une fenêtre d'action quand on clique ailleurs ; un
// clic sur le bouton qui l'ouvre ([data-pop-toggle]) est laissé à ce
// bouton.
export function outside(node: HTMLElement, close: () => void) {
  let live = true;
  const away = (e: PointerEvent) => {
    const t = e.target as Element;
    if (!node.contains(t) && !t.closest?.('[data-pop-toggle]')) close();
  };
  setTimeout(() => live && document.addEventListener('pointerdown', away));
  return {
    destroy() {
      live = false;
      document.removeEventListener('pointerdown', away);
    },
  };
}
