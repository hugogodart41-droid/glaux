// Textes de l'interface, en français et en anglais.
// La langue vient de Rust (langue du système, ou choix enregistré dans prefs.json) ;
// le bouton « English / Français » du panneau la change.
//
// Le dictionnaire anglais a le même type que le français : un texte oublié
// d'un côté empêche la compilation (tsc).

export type Lang = "fr" | "en";

let current: Lang = "fr";

export function lang(): Lang {
  return current;
}

/** Change la langue. Renvoie vrai si elle a changé. */
export function setLang(next: Lang): boolean {
  if (next === current) return false;
  current = next;
  document.documentElement.lang = next;
  return true;
}

/** Locale pour Intl (dates, nombres) : formats européens dans les deux langues. */
export function locale(): string {
  return current === "fr" ? "fr-FR" : "en-GB";
}

/** Espace fine insécable (avant « % » et les unités, en français). */
const NNBSP = " ";

/** « de novembre », mais « d'avril », « d'août », « d'octobre ». */
function deMois(month: string): string {
  return /^[aeiouyàâéèêîôû]/i.test(month) ? `d'${month}` : `de ${month}`;
}

const fr = {
  // En-tête et pied
  tagline: "Zone euro",
  refresh: "Actualiser",
  refreshHint: (keys: string) => `Actualiser (${keys})`,
  excelAria: "Exporter en Excel",
  excelHint: "Exporter toutes les données dans un fichier Excel (Documents › Glaux)",
  excelDone: "Fichier Excel enregistré",
  excelFailed: "Échec de l'export Excel",
  quit: "Quitter",
  quitHint: (keys: string) => `Quitter Glaux (${keys})`,
  loading: "Chargement…",
  brandHint: (version: string) => `Glaux ${version} · données de la BCE`,
  agendaAria: "Agenda",
  indicatorsAria: "Indicateurs macroéconomiques",
  periodAria: "Période",
  // Langue : le bouton propose l'autre langue, écrite dans cette langue.
  switchLanguage: "English",
  switchLanguageHint: "Switch Glaux to English",

  // Statut (pied du panneau)
  refreshing: "Actualisation…",
  offlineSince: (time: string, today: boolean) =>
    today ? `Hors ligne · données de ${time}` : `Hors ligne · données du ${time}`,
  partialData: "Données partielles · réessai automatique",
  unreachable: "BCE injoignable · réessai dans 15 min",
  updated: (time: string, today: boolean) => `BCE · mis à jour ${today ? "à" : "le"} ${time}`,
  loadingData: "Chargement des données…",

  // Mises à jour (remplacent le statut quand une nouvelle version existe)
  installUpdate: (version: string) => `Installer la v${version}`,
  installUpdateHint: (version: string) =>
    `Nouvelle version de Glaux (${version}) : cliquer pour l'installer et relancer l'app.`,
  updating: "Mise à jour…",
  updateFailed: (error: string) => `La mise à jour a échoué (${error}). Cliquer pour réessayer.`,

  // Liste
  derivedSection: "Calculés par Glaux",
  derivedHint: "Indicateurs calculés à partir des séries de la BCE.",
  sparkHint: (low: string, high: string) => `Sur 1 an : plus bas ${low}, plus haut ${high}`,
  /** Sous-titre d'un groupe en points de base (écarts de taux). */
  groupSub: (date: string) => `en pb · ${date}`,
  memberHint: (label: string, value: string, change: string) => `${label} : ${value} (${change})`,

  // Unités et variations
  percent: (n: string) => `${n}${NNBSP}%`,
  basisPoints: (n: string) => `${n}${NNBSP}pb`,
  bpUnit: "pb",
  /** Variation d'un taux, en points. */
  points: (n: string) => `${n}${NNBSP}pt`,
  change: (text: string) => `${text} sur 1 mois`,
  stable: "stable sur 1 mois",
  since: (date: string) => `depuis le ${date}`,

  // Vue détail
  back: "‹ Indicateurs",
  backHint: "Retour à la liste (Échap)",
  oneYear: "1 an",
  fiveYears: "5 ans",
  range: (low: string, high: string) => `Plus bas ${low} · plus haut ${high}`,
  noHistory: "Pas encore d'historique",
  readout: (date: string, value: string) => `${date} : ${value}`,
  hoverHint: "Survoler la courbe pour lire chaque mois",
  monthlyAverages: "Moyennes mensuelles",
  exportImage: "Enregistrer",
  exportHint: (keys: string) => `Enregistrer l'image en PNG dans Images › Glaux (${keys})`,
  exporting: "Enregistrement…",
  exported: "Enregistrée",
  exportFailed: "Échec",
  copyImage: "Copier",
  copyHint: (keys: string) => `Copier l'image, à coller directement dans un post (${keys})`,
  copying: "Copie…",
  copied: "Copiée",
  copyFailed: "Échec",
  formatAria: "Format de l'image",
  formatNames: { portrait: "Portrait 4:5", landscape: "Paysage 16:9", story: "Story 9:16" },
  formatHint: (size: string, platforms: string) => `Format de l'image : ${size} px, pour ${platforms}`,
  formatPlatforms: { portrait: "LinkedIn et Instagram", landscape: "LinkedIn et X", story: "TikTok, Reels et stories" },

  // Alertes
  alertsOn: "Alertes : oui",
  alertsOff: "Alertes : non",
  alertsBlocked: "Alertes : bloquées",
  alertsOnHint:
    "Notifications activées : changement de taux BCE, nouvelle inflation, rappel le jour J. Cliquer pour les couper.",
  alertsOffHint: "Notifications coupées. Cliquer pour les activer.",

  // Agenda
  ecbLabel: "Décision BCE",
  ecbHint: "Réunion de politique monétaire de la BCE : décision publiée à 14 h 15, conférence de presse à 14 h 45.",
  hicpLabel: "Inflation (flash)",
  hicpHint: (month: string) =>
    `Estimation rapide de l'inflation ${deMois(month)} dans la zone euro, publiée par Eurostat.`,
  today: "aujourd'hui",
  tomorrow: "demain",
  inDays: (n: number) => `dans ${n} jours`,
  nextEcb: (date: string) => `Prochaine décision BCE : ${date}`,
  nextHicp: (date: string) => `Prochaine inflation : ${date}`,

  // Image exportée
  effectiveSince: (date: string) => `en vigueur depuis le ${date}`,
  dataOf: (date: string, monthly: boolean) => (monthly ? `donnée ${deMois(date)}` : `donnée du ${date}`),
  overYears: (years: number) => (years > 1 ? `Sur ${years} ans` : "Sur 1 an"),
  sinceMonth: (date: string) => `depuis ${date}`,
  source: "Source : Banque centrale européenne (BCE)",
};

type Strings = typeof fr;

const en: Strings = {
  tagline: "Euro area",
  refresh: "Refresh",
  refreshHint: (keys) => `Refresh (${keys})`,
  excelAria: "Export to Excel",
  excelHint: "Export all data to an Excel file (Documents › Glaux)",
  excelDone: "Excel file saved",
  excelFailed: "Excel export failed",
  quit: "Quit",
  quitHint: (keys) => `Quit Glaux (${keys})`,
  loading: "Loading…",
  brandHint: (version) => `Glaux ${version} · ECB data`,
  agendaAria: "Calendar",
  indicatorsAria: "Macroeconomic indicators",
  periodAria: "Period",
  switchLanguage: "Français",
  switchLanguageHint: "Passer Glaux en français",

  refreshing: "Refreshing…",
  offlineSince: (time) => `Offline · data from ${time}`,
  partialData: "Partial data · retrying automatically",
  unreachable: "ECB unreachable · retry in 15 min",
  updated: (time, today) => `ECB · updated ${today ? "at" : "on"} ${time}`,
  loadingData: "Loading data…",

  installUpdate: (version) => `Install v${version}`,
  installUpdateHint: (version) => `New Glaux version (${version}): click to install it and restart the app.`,
  updating: "Updating…",
  updateFailed: (error) => `Update failed (${error}). Click to try again.`,

  derivedSection: "Computed by Glaux",
  derivedHint: "Indicators computed from ECB series.",
  sparkHint: (low, high) => `Past year: low ${low}, high ${high}`,
  groupSub: (date) => `in bp · ${date}`,
  memberHint: (label, value, change) => `${label}: ${value} (${change})`,

  percent: (n) => `${n}%`,
  basisPoints: (n) => `${n} bp`,
  bpUnit: "bp",
  points: (n) => `${n} pp`,
  change: (text) => `${text} over 1 month`,
  stable: "unchanged over 1 month",
  since: (date) => `since ${date}`,

  back: "‹ Indicators",
  backHint: "Back to the list (Esc)",
  oneYear: "1 year",
  fiveYears: "5 years",
  range: (low, high) => `Low ${low} · high ${high}`,
  noHistory: "No history yet",
  readout: (date, value) => `${date}: ${value}`,
  hoverHint: "Hover over the chart to read each month",
  monthlyAverages: "Monthly averages",
  exportImage: "Save",
  exportHint: (keys) => `Save the image as PNG in Pictures › Glaux (${keys})`,
  exporting: "Saving…",
  exported: "Saved",
  exportFailed: "Failed",
  copyImage: "Copy",
  copyHint: (keys) => `Copy the image to paste it straight into a post (${keys})`,
  copying: "Copying…",
  copied: "Copied",
  copyFailed: "Failed",
  formatAria: "Image format",
  formatNames: { portrait: "Portrait 4:5", landscape: "Landscape 16:9", story: "Story 9:16" },
  formatHint: (size, platforms) => `Image format: ${size} px, for ${platforms}`,
  formatPlatforms: { portrait: "LinkedIn and Instagram", landscape: "LinkedIn and X", story: "TikTok, Reels and stories" },

  alertsOn: "Alerts: on",
  alertsOff: "Alerts: off",
  alertsBlocked: "Alerts: blocked",
  alertsOnHint:
    "Notifications on: ECB rate changes, new inflation figures, same-day reminders. Click to turn them off.",
  alertsOffHint: "Notifications off. Click to turn them on.",

  ecbLabel: "ECB decision",
  ecbHint: "ECB monetary policy meeting: decision published at 14:15 CET, press conference at 14:45.",
  hicpLabel: "Flash inflation",
  hicpHint: (month) => `Eurostat's flash estimate of euro area inflation for ${month}.`,
  today: "today",
  tomorrow: "tomorrow",
  inDays: (n) => `in ${n} days`,
  nextEcb: (date) => `Next ECB decision: ${date}`,
  nextHicp: (date) => `Next inflation release: ${date}`,

  effectiveSince: (date) => `effective since ${date}`,
  dataOf: (date) => `data for ${date}`,
  overYears: (years) => (years > 1 ? `Over ${years} years` : "Over 1 year"),
  sinceMonth: (date) => `since ${date}`,
  source: "Source: European Central Bank (ECB)",
};

/** Textes dans la langue actuelle. */
export function t(): Strings {
  return current === "fr" ? fr : en;
}
