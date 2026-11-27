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
  customizeAria: "Choisir les indicateurs",
  enlargePanel: "Agrandir le panneau",
  shrinkPanel: "Réduire le panneau",
  customizeHint: (keys: string) => `Choisir et ordonner les indicateurs (${keys})`,
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
  euroSection: "Zone euro",
  euroHint: "Indicateurs de la zone euro dans son ensemble (taux de la BCE, marchés, monnaie et crédit).",
  countrySection: "Zone euro ou par pays",
  countryHint: "Ouvrez un indicateur pour choisir la zone euro ou un pays.",
  countriesSwitch: "Zone euro ou pays affiché sur cette ligne",
  /** Survol d'un pays : « 3,4 % · zone euro 2,9 % ». */
  countryReadout: (value: string, euroName: string, euroValue: string) => `${value} · ${euroName} ${euroValue}`,
  euroDashed: "pointillés : zone euro",
  derivedHint: "Indicateurs calculés à partir des séries de la BCE.",
  sparkHint: (low: string, high: string) => `Sur 1 an : plus bas ${low}, plus haut ${high}`,
  /** Sous-titre d'un groupe : unité de ses valeurs (« pb », « % ») et date. */
  groupSub: (unit: string, date: string) => `en ${unit} · ${date}`,
  memberHint: (label: string, value: string, change: string) => `${label} : ${value} (${change})`,

  // Écran « Indicateurs affichés »
  customizeTitle: "Indicateurs affichés",
  customizeHelp: "Cliquer sur une ligne pour l'afficher ou la masquer, la faire glisser pour changer l'ordre.",
  resetLayout: "Par défaut",
  resetLayoutHint: "Revenir à la liste d'origine : tous les indicateurs, dans l'ordre initial",
  keepOne: "Au moins un indicateur reste affiché",
  moveHint: "Glisser pour déplacer (ou ↑ ↓ au clavier)",
  countriesAria: "Pays affichés sur cette ligne",
  countriesFull: "3 pays au maximum : retirez-en un d'abord",
  countriesOne: "Au moins un pays reste affiché",

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

  // Courbe des taux
  curveSlope: "Pente 2-10 ans",
  curveSub: (date: string) => `États notés AAA · ${date}`,
  curveEnds: "3 mois → 30 ans",
  curveEndsHint: "Taux à 3 mois et à 30 ans de la courbe AAA du jour",
  curveAgo: (months: number) => (months >= 24 ? `il y a ${Math.round(months / 12)} ans` : "il y a 1 an"),
  curveHoverHint: "Survoler la courbe pour lire chaque échéance",
  curveReadout: (tenor: string, value: string, ago: string, past: string) => `${tenor} : ${value} (${ago} : ${past})`,
  curveSparkHint: (short: string, long: string) => `Courbe du jour : ${short} à 3 mois, ${long} à 30 ans`,
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
  formatNames: { portrait: "Portrait 4:5", landscape: "Paysage 16:9", story: "Story 9:16", carousel: "Carrousel PDF" },
  formatHint: (size: string, platforms: string) => `Format de l'image : ${size} px, pour ${platforms}`,
  formatPlatforms: { portrait: "LinkedIn et Instagram", landscape: "LinkedIn et X", story: "TikTok, Reels et stories" },
  // Carrousel LinkedIn : un PDF (couverture, indicateurs affichés, sources).
  carouselHint: (pages: number) =>
    `Carrousel LinkedIn : un PDF de ${pages} pages 4:5 (couverture, indicateurs affichés dans l'ordre du panneau, sources), dans Images › Glaux`,
  carouselProgress: (page: number, total: number) => `Page ${page}/${total}…`,
  carouselDone: "PDF enregistré",
  coverKicker: (month: string) => `Point macro · ${month}`,
  coverTitle: ["La zone euro", "en chiffres"],
  coverSubtitle: "Les derniers chiffres de la Banque centrale européenne.",
  swipe: "Faites défiler →",
  endTitle: "Sources et méthode",
  endSources: (downloaded: string) =>
    `Données : Banque centrale européenne, API publique (data.ecb.europa.eu), téléchargées le ${downloaded}.`,
  endComputed:
    "Calculés par Glaux : taux directeur réel = taux de dépôt − inflation ; pente 2-10 ans = taux AAA à 10 ans − taux AAA à 2 ans ; écarts = taux à 10 ans du pays − taux allemand.",
  endMonthly: "Par pays : chiffres mensuels ; taux à 10 ans en moyenne du mois (statistiques de convergence).",
  endDisclaimer: "Ces chiffres informent ; ils ne constituent pas un conseil en investissement.",
  endTagline: "Tableau de bord macro de la zone euro",
  endLink: "Mac et Windows · github.com/hugogodart41-droid/glaux",

  // Note macro mensuelle
  noteAria: "Note macro du mois",
  noteHint: (keys: string) => `Note macro du mois : faits chiffrés, ton analyse, post LinkedIn et PDF (${keys})`,
  noteTitle: "Note macro",
  noteHelp: "Phrases tirées des données de la BCE et des calculs de Glaux. Ton analyse ouvre le post et le PDF.",
  noteAnalysisLabel: "Mon analyse",
  notePlaceholder: "Ce que je retiens du mois, en quelques phrases…",
  noteCount: (n: number, limit: number) => `Post : ${n} / ${limit} caractères`,
  noteTooLong: (n: number, limit: number) => `Post : ${n} / ${limit} caractères, trop long pour LinkedIn`,
  noteEmpty: "Pas encore de données : actualiser (↻), puis rouvrir la note.",
  noteCopy: "Copier le texte",
  noteCopyHint: "Copier le texte du post (titre, analyse, chiffres, sources), à coller dans LinkedIn",
  noteCopied: "Texte copié",
  noteFailed: "Échec",
  noteSave: "Enregistrer le PDF",
  noteSaveHint: (keys: string) =>
    `Note en PDF 4:5 (couverture, analyse, 4 rubriques, sources), dans Images › Glaux (${keys})`,
  noteSaved: "PDF enregistré",
  noteCoverTitle: ["Zone euro :", "le point du mois"],
  noteCoverSubtitle: (withAnalysis: boolean): string =>
    withAnalysis
      ? "Les chiffres officiels de la BCE, les faits du mois et mon analyse."
      : "Les chiffres officiels de la BCE et les faits du mois.",
  noteContinued: (title: string) => `${title} (suite)`,

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
  customizeAria: "Choose indicators",
  enlargePanel: "Enlarge the panel",
  shrinkPanel: "Shrink the panel",
  customizeHint: (keys) => `Choose and reorder indicators (${keys})`,
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
  euroSection: "Euro area",
  euroHint: "Indicators for the euro area as a whole (ECB rates, markets, money and credit).",
  countrySection: "Euro area or by country",
  countryHint: "Open an indicator to choose the euro area or a country.",
  countriesSwitch: "Euro area or country shown in this row",
  countryReadout: (value, euroName, euroValue) => `${value} · ${euroName} ${euroValue}`,
  euroDashed: "dashed: euro area",
  derivedHint: "Indicators computed from ECB series.",
  sparkHint: (low, high) => `Past year: low ${low}, high ${high}`,
  groupSub: (unit, date) => `in ${unit} · ${date}`,
  memberHint: (label, value, change) => `${label}: ${value} (${change})`,

  customizeTitle: "Displayed indicators",
  customizeHelp: "Click a row to show or hide it, drag it to change the order.",
  resetLayout: "Reset",
  resetLayoutHint: "Back to the original list: every indicator, in the original order",
  keepOne: "At least one indicator stays visible",
  moveHint: "Drag to move (or ↑ ↓ on the keyboard)",
  countriesAria: "Countries shown in this row",
  countriesFull: "3 countries at most: remove one first",
  countriesOne: "At least one country stays visible",

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

  curveSlope: "2-10Y slope",
  curveSub: (date) => `AAA-rated governments · ${date}`,
  curveEnds: "3M → 30Y",
  curveEndsHint: "3-month and 30-year rates of today's AAA curve",
  curveAgo: (months) => (months >= 24 ? `${Math.round(months / 12)} years ago` : "1 year ago"),
  curveHoverHint: "Hover over the chart to read each maturity",
  curveReadout: (tenor, value, ago, past) => `${tenor}: ${value} (${ago}: ${past})`,
  curveSparkHint: (short, long) => `Today's curve: ${short} at 3 months, ${long} at 30 years`,
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
  formatNames: { portrait: "Portrait 4:5", landscape: "Landscape 16:9", story: "Story 9:16", carousel: "PDF carousel" },
  formatHint: (size, platforms) => `Image format: ${size} px, for ${platforms}`,
  formatPlatforms: { portrait: "LinkedIn and Instagram", landscape: "LinkedIn and X", story: "TikTok, Reels and stories" },
  carouselHint: (pages) =>
    `LinkedIn carousel: a ${pages}-page 4:5 PDF (cover, indicators shown in panel order, sources), in Pictures › Glaux`,
  carouselProgress: (page, total) => `Page ${page}/${total}…`,
  carouselDone: "PDF saved",
  coverKicker: (month) => `Macro update · ${month}`,
  coverTitle: ["The euro area", "in numbers"],
  coverSubtitle: "The latest figures from the European Central Bank.",
  swipe: "Swipe →",
  endTitle: "Sources and method",
  endSources: (downloaded) =>
    `Data: European Central Bank, public API (data.ecb.europa.eu), downloaded on ${downloaded}.`,
  endComputed:
    "Computed by Glaux: real policy rate = deposit rate − inflation; 2-10Y slope = AAA 10-year yield − AAA 2-year yield; spreads = country 10-year yield − German yield.",
  endMonthly: "By country: monthly figures; 10-year yields as monthly averages (convergence statistics).",
  endDisclaimer: "These figures are for information only; they are not investment advice.",
  endTagline: "Euro area macro dashboard",
  endLink: "Mac and Windows · github.com/hugogodart41-droid/glaux",

  noteAria: "Monthly macro note",
  noteHint: (keys) => `Monthly macro note: key facts, your analysis, LinkedIn post and PDF (${keys})`,
  noteTitle: "Macro note",
  noteHelp: "Sentences built from ECB data and Glaux calculations. Your analysis opens the post and the PDF.",
  noteAnalysisLabel: "My analysis",
  notePlaceholder: "What I take away from this month, in a few sentences…",
  noteCount: (n, limit) => `Post: ${n} / ${limit} characters`,
  noteTooLong: (n, limit) => `Post: ${n} / ${limit} characters, too long for LinkedIn`,
  noteEmpty: "No data yet: refresh (↻), then reopen the note.",
  noteCopy: "Copy text",
  noteCopyHint: "Copy the post text (title, analysis, figures, sources), ready to paste into LinkedIn",
  noteCopied: "Text copied",
  noteFailed: "Failed",
  noteSave: "Save PDF",
  noteSaveHint: (keys) => `4:5 PDF note (cover, analysis, 4 sections, sources), in Pictures › Glaux (${keys})`,
  noteSaved: "PDF saved",
  noteCoverTitle: ["Euro area:", "the month in review"],
  noteCoverSubtitle: (withAnalysis) =>
    withAnalysis
      ? "Official ECB figures, the month's key facts and my analysis."
      : "Official ECB figures and the month's key facts.",
  noteContinued: (title) => `${title} (continued)`,

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
