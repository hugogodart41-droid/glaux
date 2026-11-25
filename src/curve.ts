// Courbe des taux : données prêtes à dessiner pour la ligne du panneau, la vue détail
// et l'image exportée. Les taux viennent de Rust (store.rs : `CURVE`, commande get_curve).

import { invoke } from "@tauri-apps/api/core";
import { COLORS, type AxisLabel, type ChartSeries } from "./chart";
import { CURVE_TENORS, tenorCode } from "./indicators";
import type { IndicatorView, Observation } from "./types";

/** Courbe à une date : un taux par échéance de `CURVE_TENORS` (null si absent). */
export type CurvePoint = { date: string; values: (number | null)[] };

/** Courbe du dernier jour publié et courbe de comparaison (1 ou 5 ans plus tôt). */
export type CurveView = { latest: CurvePoint | null; past: CurvePoint | null };

/** Courbe du jour et celle de `months` mois plus tôt. */
export function loadCurve(months: number): Promise<CurveView> {
  return invoke<CurveView>("get_curve", { months });
}

/** Taux du jour par échéance, depuis la vue du panneau (undefined : échéance absente). */
export function todayRates(indicators: IndicatorView[]): (number | undefined)[] {
  return CURVE_TENORS.map((tenor) => indicators.find((i) => i.id === tenor.id)?.latest?.value);
}

/** Pente 2-10 ans de la courbe AAA (indicateur calculé « slope », envoyé par Rust). */
export function curveSlope(indicators: IndicatorView[]): IndicatorView | null {
  return indicators.find((i) => i.id === "slope") ?? null;
}

/** Rang d'une échéance, écrit comme une « date » triable (00 … 10) pour les fonctions de dessin. */
function tenorKey(index: number): string {
  return String(index).padStart(2, "0");
}

/** Rang de l'échéance d'une « date » renvoyée au survol de la courbe. */
export function tenorAt(key: string): number {
  return Number(key);
}

function points(curve: CurvePoint | null): Observation[] {
  if (!curve) return [];
  return curve.values.flatMap((value, k) => (value === null ? [] : [{ date: tenorKey(k), value }]));
}

/**
 * Courbes à dessiner : la comparaison en pointillés crème, puis celle du jour en or (au-dessus),
 * et les échéances comme étiquettes de l'axe horizontal.
 */
export function curveChart(curve: CurveView): { series: ChartSeries[]; labels: AxisLabel[] } {
  const series: ChartSeries[] = [];
  if (curve.past) series.push({ points: points(curve.past), color: COLORS.creme, dashed: true });
  series.push({ points: points(curve.latest), color: COLORS.or });
  // Rangs dans la liste réunie des échéances présentes (celle des fonctions de dessin).
  const keys = [...new Set(series.flatMap((s) => s.points.map((p) => p.date)))].sort();
  const labels = CURVE_TENORS.flatMap((tenor, k) => {
    const index = keys.indexOf(tenorKey(k));
    return index < 0 ? [] : [{ index, text: tenorCode(tenor) }];
  });
  return { series, labels };
}
