// Agenda : prochaine décision de la BCE et prochaine estimation rapide de l'inflation.
// Les dates viennent du cœur Rust (src-tauri/src/agenda.rs), qui s'en sert aussi
// pour les rappels du jour J : un seul calendrier à mettre à jour chaque année.

import { locale, t } from "./i18n";
import type { AgendaData } from "./types";

export type AgendaKind = "ecb" | "hicp";

export type AgendaEvent = {
  kind: AgendaKind;
  date: Date;
  /** Jours restants : 0 aujourd'hui, 1 demain… */
  daysLeft: number;
  /** Indicateur ouvert au clic (vue détail). */
  indicatorId: string;
  label: string;
  /** Explication au survol. */
  hint: string;
};

/** « 2026-12-17 » → minuit, heure locale. */
function localDate(iso: string): Date {
  const [y, m, d] = iso.split("-").map(Number);
  return new Date(y, m - 1, d);
}

function startOfDay(date: Date): Date {
  return new Date(date.getFullYear(), date.getMonth(), date.getDate());
}

/** Première date du calendrier qui n'est pas encore passée (aujourd'hui compris). */
function nextDate(dates: string[], now: Date): { date: Date; daysLeft: number } | null {
  const today = startOfDay(now);
  for (const iso of dates) {
    const date = localDate(iso);
    if (date >= today) {
      // Arrondi : absorbe le décalage d'une heure des passages heure d'été / d'hiver.
      const daysLeft = Math.round((date.getTime() - today.getTime()) / 86_400_000);
      return { date, daysLeft };
    }
  }
  return null;
}

/** Prochains événements de l'agenda, dans l'ordre d'affichage. */
export function upcomingEvents(agenda: AgendaData | undefined, now = new Date()): AgendaEvent[] {
  const events: AgendaEvent[] = [];
  if (!agenda) return events;

  const ecb = nextDate(agenda.ecb, now);
  if (ecb) {
    events.push({
      kind: "ecb",
      ...ecb,
      indicatorId: "deposit_rate",
      label: t().ecbLabel,
      hint: t().ecbHint,
    });
  }

  const hicp = nextDate(agenda.hicp, now);
  if (hicp) {
    // L'estimation rapide porte sur le mois précédent la publication.
    const month = new Date(hicp.date.getFullYear(), hicp.date.getMonth() - 1, 1);
    const monthName = new Intl.DateTimeFormat(locale(), { month: "long" }).format(month);
    events.push({
      kind: "hicp",
      ...hicp,
      indicatorId: "hicp",
      label: t().hicpLabel,
      hint: t().hicpHint(monthName),
    });
  }

  return events;
}

/** « aujourd'hui », « demain », « dans 23 jours » (« today », « tomorrow », « in 23 days »). */
export function formatCountdown(daysLeft: number): string {
  if (daysLeft <= 0) return t().today;
  if (daysLeft === 1) return t().tomorrow;
  return t().inDays(daysLeft);
}

/** « jeu. 17 déc. » / « Thu 17 Dec » : sans l'année, l'événement est toujours dans les semaines qui viennent. */
export function formatEventDate(date: Date): string {
  return new Intl.DateTimeFormat(locale(), { weekday: "short", day: "numeric", month: "short" }).format(date);
}

/** Texte pour l'image exportée : « Prochaine décision BCE : 17 décembre 2026 » / « Next ECB decision: 17 December 2026 ». */
export function exportAgendaLine(
  indicatorId: string,
  agenda: AgendaData | undefined,
  now = new Date(),
): string | null {
  const event = upcomingEvents(agenda, now).find((e) => e.indicatorId === indicatorId);
  if (!event) return null;
  const date = new Intl.DateTimeFormat(locale(), { day: "numeric", month: "long", year: "numeric" }).format(
    event.date,
  );
  return event.kind === "ecb" ? t().nextEcb(date) : t().nextHicp(date);
}
