// Mise en forme des nombres et des dates, en français ou en anglais (voir i18n.ts).

import { locale, t } from "./i18n";
import type { Display } from "./indicators";
import type { IndicatorView } from "./types";

export const IS_MAC = navigator.userAgent.includes("Mac");

export function formatNumber(value: number, decimals: number, signed = false): string {
  return (
    new Intl.NumberFormat(locale(), {
      minimumFractionDigits: decimals,
      maximumFractionDigits: decimals,
      signDisplay: signed ? "exceptZero" : "auto",
    })
      .format(value)
      // Vrai signe moins typographique (−) plutôt que le tiret (-).
      .replace(/-/g, "\u2212")
  );
}

/** Trimestre d'une période « 2026-Q3 » (→ [2026, 3]) ; undefined pour un jour ou un mois. */
function quarterOf(period: string): [number, number] | undefined {
  const match = /^(\d{4})-Q([1-4])$/.exec(period.trim());
  return match ? [Number(match[1]), Number(match[2])] : undefined;
}

/** Vrai pour une période trimestrielle (« 2026-Q3 »). */
export function isQuarter(period: string | undefined): boolean {
  return period !== undefined && quarterOf(period) !== undefined;
}

/**
 * « 2026-11-21 », « 2026-10 » ou « 2026-Q3 » (→ 1er jour du trimestre) → Date locale
 * (sans décalage de fuseau).
 */
export function parsePeriod(period: string): Date | null {
  const quarter = quarterOf(period);
  if (quarter) return new Date(quarter[0], quarter[1] * 3 - 3, 1);
  const [y, m, d] = period.split("-").map(Number);
  if (!y || !m) return null;
  return new Date(y, m - 1, d || 1);
}

/** Date courte : « 23 nov. » / « 23 Nov » (année ajoutée si ce n'est pas l'année en cours), ou « oct. 2026 ». */
export function formatPeriod(period: string, monthly: boolean): string {
  const quarter = quarterOf(period);
  if (quarter) return t().quarter(quarter[1], quarter[0]);
  const date = parsePeriod(period);
  if (!date) return period;
  if (monthly) {
    return new Intl.DateTimeFormat(locale(), { month: "short", year: "numeric" }).format(date);
  }
  const sameYear = date.getFullYear() === new Date().getFullYear();
  return new Intl.DateTimeFormat(locale(), {
    day: "numeric",
    month: "short",
    year: sameYear ? undefined : "numeric",
  }).format(date);
}

/** Date complète : « 23 nov. 2026 » (ou « oct. 2026 » pour une série mensuelle). */
export function formatFullPeriod(period: string, monthly: boolean): string {
  const quarter = quarterOf(period);
  if (quarter) return t().quarter(quarter[1], quarter[0]);
  const date = parsePeriod(period);
  if (!date) return period;
  return new Intl.DateTimeFormat(locale(), {
    day: monthly ? undefined : "numeric",
    month: "short",
    year: "numeric",
  }).format(date);
}

export function formatValue(value: number, display: Display, decimals = display.decimals): string {
  switch (display.kind) {
    case "rate":
      return t().percent(formatNumber(value, decimals));
    case "bp":
      // Valeur reçue en points de pourcentage : 1,49 point = 149 pb.
      return t().basisPoints(formatNumber(value * 100, Math.max(0, decimals - 2)));
    default:
      return formatNumber(value, decimals);
  }
}

/**
 * Variation sur 1 mois (1 trimestre pour le PIB) : en points pour un taux, en % pour un cours
 * de change.
 */
export function formatChange(indicator: IndicatorView, display: Display): string {
  const { latest, reference } = indicator;
  if (!latest || !reference) return "";

  // Taux : écart en points ; écart de taux : en points de base ; cours : en %.
  const text = t();
  const [change, decimals, unit] =
    display.kind === "rate"
      ? [latest.value - reference.value, Math.min(display.decimals, 2), text.points]
      : display.kind === "bp"
        ? [(latest.value - reference.value) * 100, 0, text.basisPoints]
        : [(latest.value / reference.value - 1) * 100, 1, text.percent];

  // Variation trop petite pour être visible à l'arrondi : « stable ».
  const quarterly = isQuarter(latest.date);
  if (Math.abs(change) < 0.5 * 10 ** -decimals) return quarterly ? text.stableQuarter : text.stable;

  const shown = unit(formatNumber(change, decimals, true));
  return quarterly ? text.changeQuarter(shown) : text.change(shown);
}

/**
 * Valeur courte d'un membre de groupe, sans unité (déjà dans le sous-titre) :
 * écart en points de base « 147 », taux en % « 3,42 ».
 */
export function shortValue(value: number | undefined, display: Display): string {
  if (value === undefined) return "—";
  return display.kind === "bp" ? formatNumber(value * 100, 0) : formatNumber(value, Math.min(display.decimals, 2));
}

/** Unité des valeurs courtes d'un groupe : « pb » ou « % ». */
export function shortUnit(display: Display): string {
  return display.kind === "bp" ? t().bpUnit : "%";
}

/**
 * Variation courte sur 1 mois d'un membre de groupe : écart en pb « +45 », taux en points
 * « +0,12 » ; « = » si invisible à l'arrondi. `withUnit` : « +45 pb », « +0,12 pt ».
 */
export function shortChange(indicator: IndicatorView, display: Display, withUnit = false): string {
  const { latest, reference } = indicator;
  if (!latest || !reference) return "";
  const bp = display.kind === "bp";
  const decimals = bp ? 0 : 2;
  const change = (latest.value - reference.value) * (bp ? 100 : 1);
  if (Math.abs(change) < 0.5 * 10 ** -decimals) return "=";
  const text = formatNumber(change, decimals, true);
  if (!withUnit) return text;
  return bp ? t().basisPoints(text) : t().points(text);
}

/** Ligne de date sous le nom : « depuis le 16 sept. 2026 » / « since 16 Sept 2026 », « 23 nov. », « oct. 2026 ». */
export function formatDateLine(indicator: IndicatorView, display: Display): string {
  if (indicator.since) {
    const date = parsePeriod(indicator.since);
    if (date) {
      const text = new Intl.DateTimeFormat(locale(), {
        day: "numeric",
        month: "short",
        year: "numeric",
      }).format(date);
      return t().since(text);
    }
  }
  return indicator.latest ? formatPeriod(indicator.latest.date, display.monthly) : "";
}

/** Heure d'une actualisation : « 09:12 » aujourd'hui, sinon « 23 nov., 09:12 ». */
export function formatUpdate(ms: number): { today: boolean; text: string } {
  const date = new Date(ms);
  const time = new Intl.DateTimeFormat(locale(), { hour: "2-digit", minute: "2-digit" }).format(date);
  if (date.toDateString() === new Date().toDateString()) return { today: true, text: time };
  const day = new Intl.DateTimeFormat(locale(), { day: "numeric", month: "short" }).format(date);
  return { today: false, text: `${day}, ${time}` };
}

/**
 * Date de la donnée, pour l'image exportée :
 * « en vigueur depuis le 16 sept. 2026 », « donnée du 24 nov. 2026 », « donnée d'oct. 2026 »
 * (« effective since 16 Sept 2026 », « data for 24 Nov 2026 »…).
 */
export function formatDataDate(indicator: IndicatorView, display: Display): string {
  if (indicator.since) return t().effectiveSince(formatFullPeriod(indicator.since, false));
  if (!indicator.latest) return "";
  if (isQuarter(indicator.latest.date)) return t().dataOfQuarter(formatFullPeriod(indicator.latest.date, true));
  return t().dataOf(formatFullPeriod(indicator.latest.date, display.monthly), display.monthly);
}
