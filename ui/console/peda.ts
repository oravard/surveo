// Petits textes communs aux fenêtres de la pédagogie (SPEC.md, chap. 3).
import type { Model } from '$shared/bindings/Model';

const plural = (n: number, w: string) => `${n} ${w}${n > 1 ? 's' : ''}`;

/** « 3 étapes · 45 min · 2 rendus attendus ». */
export function modelSummary(m: Model): string {
  const min = m.steps.reduce((t, s) => t + s.minutes, 0);
  const rendus = m.expected === 0 ? 'aucun rendu attendu' : `${plural(m.expected, 'rendu')} attendu${m.expected > 1 ? 's' : ''}`;
  return [plural(m.steps.length, 'étape'), min ? `${min} min` : null, rendus].filter(Boolean).join(' · ');
}

/** « 1,4 Mo ». */
export function sizeText(bytes: number): string {
  if (bytes < 1000) return `${bytes} o`;
  const [v, u] = bytes >= 1e9 ? [bytes / 1e9, 'Go'] : bytes >= 1e6 ? [bytes / 1e6, 'Mo'] : [bytes / 1e3, 'Ko'];
  return `${(v < 10 ? v.toFixed(1) : v.toFixed(0)).replace('.', ',').replace(/,0$/, '')} ${u}`;
}
