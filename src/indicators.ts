// Présentation de chaque indicateur : unité, précision, et textes (français / anglais).
// Clés = identifiants définis dans src-tauri/src/ecb.rs (l'ordre d'affichage vient de Rust).

import { lang, type Lang } from "./i18n";

export type Display = {
  label: string;
  /**
   * "rate" : en %, variation en points ; "fx" : cours, variation en % ;
   * "bp" : écart de taux en points de base (1 pb = 0,01 point).
   */
  kind: "rate" | "fx" | "bp";
  /** Décimales affichées (pour "bp" : décimales des valeurs reçues, en points). */
  decimals: number;
  monthly: boolean;
  /** Ce que mesure l'indicateur (infobulle du nom, sous-titre de l'image exportée). */
  hint: string;
  /** Ce que ça veut dire, en 2 lignes (vue détail et image exportée). */
  explanation: string;
  /** Taux qui ne bouge qu'aux décisions de la BCE : courbe en marches d'escalier. */
  step?: boolean;
};

type Format = Pick<Display, "kind" | "decimals" | "monthly" | "step">;
type Texts = Pick<Display, "label" | "hint" | "explanation">;

const FORMATS: Record<string, Format> = {
  deposit_rate: { kind: "rate", decimals: 2, monthly: false, step: true },
  hicp: { kind: "rate", decimals: 1, monthly: true },
  estr: { kind: "rate", decimals: 3, monthly: false },
  yield_10y: { kind: "rate", decimals: 2, monthly: false },
  // Taux à 10 ans par pays (groupe « yields »), moyennes mensuelles en %.
  yield_de: { kind: "rate", decimals: 2, monthly: true },
  yield_fr: { kind: "rate", decimals: 2, monthly: true },
  yield_it: { kind: "rate", decimals: 2, monthly: true },
  yield_es: { kind: "rate", decimals: 2, monthly: true },
  yield_gr: { kind: "rate", decimals: 2, monthly: true },
  yield_pt: { kind: "rate", decimals: 2, monthly: true },
  yield_be: { kind: "rate", decimals: 2, monthly: true },
  yield_nl: { kind: "rate", decimals: 2, monthly: true },
  eurusd: { kind: "fx", decimals: 4, monthly: false },
  unemployment: { kind: "rate", decimals: 1, monthly: true },
  // Trimestriel : dates « 2026-Q3 », affichées « T3 2026 » (format.ts).
  gdp_qoq: { kind: "rate", decimals: 1, monthly: true },
  gdp_yoy: { kind: "rate", decimals: 1, monthly: true },
  // V3 : masqués par défaut (à afficher dans « Indicateurs affichés »).
  core_inflation: { kind: "rate", decimals: 1, monthly: true },
  services_inflation: { kind: "rate", decimals: 1, monthly: true },
  m3: { kind: "rate", decimals: 1, monthly: true },
  loans_nfc: { kind: "rate", decimals: 1, monthly: true },
  loans_hh: { kind: "rate", decimals: 1, monthly: true },
  // Calculés par Glaux (src-tauri/src/data.rs)
  real_rate: { kind: "rate", decimals: 2, monthly: true },
  slope: { kind: "bp", decimals: 2, monthly: false },
  // Écarts de taux à 10 ans avec l'Allemagne (groupe « spreads »), en points de base.
  spread_fr: { kind: "bp", decimals: 2, monthly: true },
  spread_it: { kind: "bp", decimals: 2, monthly: true },
  spread_es: { kind: "bp", decimals: 2, monthly: true },
  spread_gr: { kind: "bp", decimals: 2, monthly: true },
  spread_pt: { kind: "bp", decimals: 2, monthly: true },
  spread_be: { kind: "bp", decimals: 2, monthly: true },
  spread_nl: { kind: "bp", decimals: 2, monthly: true },
};

const TEXTS: Record<Lang, Record<string, Texts>> = {
  fr: {
    deposit_rate: {
      label: "Taux de dépôt BCE",
      hint: "Taux de la facilité de dépôt : le taux directeur de référence de la BCE.",
      explanation:
        "Ce que la BCE verse aux banques sur leurs dépôts. Quand il monte, le crédit renchérit et l'activité ralentit, ce qui freine l'inflation.",
    },
    hicp: {
      label: "Inflation (sur 1 an)",
      hint: "Indice des prix harmonisé (IPCH), zone euro, glissement annuel.",
      explanation:
        "Hausse des prix sur douze mois. La BCE vise 2 % à moyen terme : nettement au-dessus, elle a tendance à relever ses taux.",
    },
    estr: {
      label: "€STR",
      hint: "Taux auquel les banques se prêtent au jour le jour, sans garantie.",
      explanation:
        "Coût de l'argent au jour le jour entre banques. Il suit de près le taux de dépôt : c'est par lui que les décisions de la BCE atteignent les marchés.",
    },
    yield_10y: {
      label: "Taux souverain 10 ans",
      hint: "Taux à 10 ans de l'ensemble des États de la zone euro, toutes notations (Allemagne, France, Italie…), estimé chaque jour par la BCE.",
      explanation:
        "Coût d'emprunt à 10 ans des États de la zone euro. Il reflète les taux et l'inflation attendus, et sert de repère aux crédits immobiliers.",
    },
    eurusd: {
      label: "EUR/USD",
      hint: "Cours de référence de la BCE : dollars pour 1 euro.",
      explanation:
        "Nombre de dollars pour 1 euro. Un euro fort rend les importations moins chères (moins d'inflation importée) mais pèse sur les exportateurs.",
    },
    unemployment: {
      label: "Chômage",
      hint: "Taux de chômage, zone euro, 15-74 ans, corrigé des variations saisonnières.",
      explanation:
        "Part des actifs sans emploi. Un chômage bas signale un marché du travail tendu, qui peut pousser les salaires, puis les prix, à la hausse.",
    },
    gdp_qoq: {
      label: "PIB (sur 1 trimestre)",
      hint: "PIB en volume, zone euro, croissance par rapport au trimestre précédent (Eurostat, CVS).",
      explanation:
        "La croissance de l'économie d'un trimestre à l'autre : le chiffre phare d'Eurostat. Deux trimestres de recul d'affilée signalent souvent une récession.",
    },
    gdp_yoy: {
      label: "PIB (sur 1 an)",
      hint: "PIB en volume, zone euro, croissance par rapport au même trimestre un an plus tôt (CVS).",
      explanation:
        "La croissance sur douze mois, moins heurtée que d'un trimestre à l'autre. Elle se compare directement à l'inflation et aux salaires.",
    },
    core_inflation: {
      label: "Inflation sous-jacente",
      hint: "IPCH hors énergie, alimentation, alcool et tabac, zone euro, glissement annuel.",
      explanation:
        "La tendance de fond des prix, sans les éléments les plus volatils. La BCE la suit de près pour juger si l'inflation revient durablement vers 2 %.",
    },
    services_inflation: {
      label: "Inflation des services",
      hint: "IPCH des services (loyers, transports, restauration…), zone euro, glissement annuel.",
      explanation:
        "Très liée aux salaires, elle ne baisse que lentement. Tant qu'elle reste élevée, la BCE hésite à baisser ses taux.",
    },
    m3: {
      label: "Masse monétaire M3",
      hint: "Croissance sur 1 an de M3 : billets, dépôts et placements à court terme, zone euro.",
      explanation:
        "La quantité de monnaie au sens large. Une croissance rapide peut annoncer de l'inflation ; une croissance faible, un crédit qui ralentit.",
    },
    loans_nfc: {
      label: "Crédit aux entreprises",
      hint: "Prêts des banques aux entreprises de la zone euro, croissance sur 1 an (corrigée des cessions et titrisations).",
      explanation:
        "Montre si les décisions de la BCE se transmettent à l'économie : quand les taux montent, le crédit ralentit, puis l'investissement.",
    },
    loans_hh: {
      label: "Crédit aux ménages",
      hint: "Prêts des banques aux ménages de la zone euro, surtout immobiliers, croissance sur 1 an (corrigée des cessions et titrisations).",
      explanation:
        "Reflète d'abord le marché immobilier, qui réagit vite aux taux des crédits : un bon thermomètre de la politique monétaire.",
    },
    real_rate: {
      label: "Taux directeur réel",
      hint: "Taux de dépôt de la BCE moins l'inflation sur 1 an (taux réel ex post) : le coût de l'argent une fois l'inflation déduite.",
      explanation:
        "Taux de dépôt moins inflation. Négatif, la politique monétaire reste accommodante même quand les taux montent ; positif, elle freine vraiment l'économie.",
    },
    // Taux à 10 ans par pays (groupe « yields ») : l'explication est celle du groupe.
    yield_de: {
      label: "Allemagne",
      hint: "Bund : taux d'emprunt à 10 ans de l'Allemagne, moyenne du mois.",
      explanation: "",
    },
    yield_fr: {
      label: "France",
      hint: "OAT : taux d'emprunt à 10 ans de la France, moyenne du mois.",
      explanation: "",
    },
    yield_it: {
      label: "Italie",
      hint: "BTP : taux d'emprunt à 10 ans de l'Italie, moyenne du mois.",
      explanation: "",
    },
    yield_es: {
      label: "Espagne",
      hint: "Bonos : taux d'emprunt à 10 ans de l'Espagne, moyenne du mois.",
      explanation: "",
    },
    yield_gr: {
      label: "Grèce",
      hint: "GGB : taux d'emprunt à 10 ans de la Grèce, moyenne du mois.",
      explanation: "",
    },
    yield_pt: {
      label: "Portugal",
      hint: "OT : taux d'emprunt à 10 ans du Portugal, moyenne du mois.",
      explanation: "",
    },
    yield_be: {
      label: "Belgique",
      hint: "OLO : taux d'emprunt à 10 ans de la Belgique, moyenne du mois.",
      explanation: "",
    },
    yield_nl: {
      label: "Pays-Bas",
      hint: "DSL : taux d'emprunt à 10 ans des Pays-Bas, moyenne du mois.",
      explanation: "",
    },
    spread_fr: {
      label: "France",
      hint: "OAT − Bund : taux d'emprunt à 10 ans de la France moins celui de l'Allemagne.",
      explanation: "",
    },
    spread_it: {
      label: "Italie",
      hint: "BTP − Bund : taux d'emprunt à 10 ans de l'Italie moins celui de l'Allemagne.",
      explanation: "",
    },
    spread_es: {
      label: "Espagne",
      hint: "Bonos − Bund : taux d'emprunt à 10 ans de l'Espagne moins celui de l'Allemagne.",
      explanation: "",
    },
    spread_gr: {
      label: "Grèce",
      hint: "GGB − Bund : taux d'emprunt à 10 ans de la Grèce moins celui de l'Allemagne.",
      explanation: "",
    },
    spread_pt: {
      label: "Portugal",
      hint: "OT − Bund : taux d'emprunt à 10 ans du Portugal moins celui de l'Allemagne.",
      explanation: "",
    },
    spread_be: {
      label: "Belgique",
      hint: "OLO − Bund : taux d'emprunt à 10 ans de la Belgique moins celui de l'Allemagne.",
      explanation: "",
    },
    spread_nl: {
      label: "Pays-Bas",
      hint: "DSL − Bund : taux d'emprunt à 10 ans des Pays-Bas moins celui de l'Allemagne.",
      explanation: "",
    },
    slope: {
      label: "Pente 2-10 ans",
      hint: "Taux à 10 ans moins taux à 2 ans des États notés AAA (courbe de la BCE), en points de base.",
      explanation:
        "La pente de la courbe des taux. Positive, c'est la norme ; négative (courbe inversée), les marchés attendent des baisses de taux, souvent avant un ralentissement.",
    },
  },
  en: {
    deposit_rate: {
      label: "ECB deposit rate",
      hint: "Deposit facility rate: the ECB's main policy rate.",
      explanation:
        "What the ECB pays banks on their deposits. When it rises, credit gets more expensive and activity slows, which cools inflation.",
    },
    hicp: {
      label: "Inflation (y/y)",
      hint: "Harmonised Index of Consumer Prices (HICP), euro area, annual rate.",
      explanation:
        "Price growth over twelve months. The ECB targets 2% over the medium term: well above that, it tends to raise rates.",
    },
    estr: {
      label: "€STR",
      hint: "Rate at which banks lend to each other overnight, unsecured.",
      explanation:
        "The cost of overnight money between banks. It closely tracks the deposit rate: it is how ECB decisions reach the markets.",
    },
    yield_10y: {
      label: "10-year sovereign yield",
      hint: "10-year rate across all euro area governments, all ratings (Germany, France, Italy…), estimated daily by the ECB.",
      explanation:
        "What euro area governments pay to borrow for 10 years. It reflects expected rates and inflation, and is a benchmark for mortgages.",
    },
    eurusd: {
      label: "EUR/USD",
      hint: "ECB reference rate: US dollars per euro.",
      explanation:
        "Dollars per euro. A strong euro makes imports cheaper (less imported inflation) but weighs on exporters.",
    },
    unemployment: {
      label: "Unemployment",
      hint: "Unemployment rate, euro area, ages 15–74, seasonally adjusted.",
      explanation:
        "Share of the labour force without a job. Low unemployment signals a tight labour market, which can push wages, then prices, higher.",
    },
    gdp_qoq: {
      label: "GDP (q/q)",
      hint: "Real GDP, euro area, growth on the previous quarter (Eurostat, seasonally adjusted).",
      explanation:
        "How fast the economy grew from one quarter to the next: Eurostat's headline figure. Two quarters of decline in a row often signal a recession.",
    },
    gdp_yoy: {
      label: "GDP (y/y)",
      hint: "Real GDP, euro area, growth on the same quarter a year earlier (seasonally adjusted).",
      explanation:
        "Growth over twelve months, smoother than quarter on quarter. It compares directly with inflation and wages.",
    },
    core_inflation: {
      label: "Core inflation",
      hint: "HICP excluding energy, food, alcohol and tobacco, euro area, annual rate.",
      explanation:
        "The underlying price trend, without the most volatile items. The ECB watches it closely to judge whether inflation is durably back near 2%.",
    },
    services_inflation: {
      label: "Services inflation",
      hint: "HICP services (rents, transport, restaurants…), euro area, annual rate.",
      explanation:
        "Closely tied to wages, it comes down only slowly. While it stays high, the ECB is reluctant to cut rates.",
    },
    m3: {
      label: "M3 money supply",
      hint: "Annual growth of M3: currency, deposits and short-term instruments, euro area.",
      explanation:
        "The broad amount of money in the economy. Fast growth can signal inflation ahead; weak growth, slowing credit.",
    },
    loans_nfc: {
      label: "Loans to firms",
      hint: "Bank loans to euro area non-financial companies, annual growth (adjusted for sales and securitisation).",
      explanation:
        "Shows whether ECB decisions reach the economy: when rates rise, lending slows, then investment follows.",
    },
    loans_hh: {
      label: "Loans to households",
      hint: "Bank loans to euro area households, mostly mortgages, annual growth (adjusted for sales and securitisation).",
      explanation:
        "Mainly reflects the housing market, which reacts quickly to mortgage rates: a good gauge of monetary policy.",
    },
    real_rate: {
      label: "Real policy rate",
      hint: "ECB deposit rate minus annual inflation (ex post real rate): the cost of money after inflation.",
      explanation:
        "Deposit rate minus inflation. When negative, policy stays accommodative even as rates rise; when positive, it genuinely holds the economy back.",
    },
    yield_de: {
      label: "Germany",
      hint: "Bund: Germany's 10-year borrowing rate, monthly average.",
      explanation: "",
    },
    yield_fr: {
      label: "France",
      hint: "OAT: France's 10-year borrowing rate, monthly average.",
      explanation: "",
    },
    yield_it: {
      label: "Italy",
      hint: "BTP: Italy's 10-year borrowing rate, monthly average.",
      explanation: "",
    },
    yield_es: {
      label: "Spain",
      hint: "Bonos: Spain's 10-year borrowing rate, monthly average.",
      explanation: "",
    },
    yield_gr: {
      label: "Greece",
      hint: "GGB: Greece's 10-year borrowing rate, monthly average.",
      explanation: "",
    },
    yield_pt: {
      label: "Portugal",
      hint: "OT: Portugal's 10-year borrowing rate, monthly average.",
      explanation: "",
    },
    yield_be: {
      label: "Belgium",
      hint: "OLO: Belgium's 10-year borrowing rate, monthly average.",
      explanation: "",
    },
    yield_nl: {
      label: "Netherlands",
      hint: "DSL: the Netherlands' 10-year borrowing rate, monthly average.",
      explanation: "",
    },
    spread_fr: {
      label: "France",
      hint: "OAT − Bund: France's 10-year borrowing rate minus Germany's.",
      explanation: "",
    },
    spread_it: {
      label: "Italy",
      hint: "BTP − Bund: Italy's 10-year borrowing rate minus Germany's.",
      explanation: "",
    },
    spread_es: {
      label: "Spain",
      hint: "Bonos − Bund: Spain's 10-year borrowing rate minus Germany's.",
      explanation: "",
    },
    spread_gr: {
      label: "Greece",
      hint: "GGB − Bund: Greece's 10-year borrowing rate minus Germany's.",
      explanation: "",
    },
    spread_pt: {
      label: "Portugal",
      hint: "OT − Bund: Portugal's 10-year borrowing rate minus Germany's.",
      explanation: "",
    },
    spread_be: {
      label: "Belgium",
      hint: "OLO − Bund: Belgium's 10-year borrowing rate minus Germany's.",
      explanation: "",
    },
    spread_nl: {
      label: "Netherlands",
      hint: "DSL − Bund: the Netherlands' 10-year borrowing rate minus Germany's.",
      explanation: "",
    },
    slope: {
      label: "2-10Y slope",
      hint: "AAA-rated governments' 10-year rate minus their 2-year rate (ECB yield curve), in basis points.",
      explanation:
        "The slope of the yield curve. Positive is the norm; negative (inverted curve) means markets expect rate cuts, often ahead of a slowdown.",
    },
  },
};

/** Présentation d'un indicateur dans la langue actuelle (undefined : indicateur inconnu). */
export function displayFor(id: string): Display | undefined {
  const format = FORMATS[id];
  const texts = TEXTS[lang()][id];
  if (format && texts) return { ...format, ...texts };
  return countryDisplay(id);
}

// ---------- Zone euro ou par pays ----------
// Inflation et chômage existent pour chaque pays (BY_COUNTRY dans src-tauri/src/ecb.rs) :
// la ligne du panneau affiche la zone euro ou le pays choisi dans la vue détail.

/** Pays proposés, dans l'ordre de COUNTRIES (ecb.rs). */
export const COUNTRY_CODES = ["de", "fr", "it", "es", "gr", "pt", "be", "nl"];

/** Lignes du panneau disponibles par pays (même liste que BY_COUNTRY dans ecb.rs). */
const COUNTRY_ROWS = ["hicp", "core_inflation", "services_inflation", "unemployment", "gdp_qoq", "gdp_yoy"];

const COUNTRY_NAMES: Record<Lang, Record<string, string>> = {
  fr: {
    de: "Allemagne",
    fr: "France",
    it: "Italie",
    es: "Espagne",
    gr: "Grèce",
    pt: "Portugal",
    be: "Belgique",
    nl: "Pays-Bas",
  },
  en: {
    de: "Germany",
    fr: "France",
    it: "Italy",
    es: "Spain",
    gr: "Greece",
    pt: "Portugal",
    be: "Belgium",
    nl: "Netherlands",
  },
};

/** Vrai pour une ligne qu'on peut afficher par pays (inflation, chômage, PIB). */
export function hasCountries(row: string): boolean {
  return COUNTRY_ROWS.includes(row);
}

/** Nom d'un pays (« France ») ; sans pays : « Zone euro ». */
export function countryName(code: string | undefined): string {
  return (code && COUNTRY_NAMES[lang()][code]) || (lang() === "fr" ? "Zone euro" : "Euro area");
}

/** Code court d'une pastille : « FR », ou « ZE » / « EA » pour la zone euro. */
export function countryCode(code: string | undefined): string {
  return code ? code.toUpperCase() : lang() === "fr" ? "ZE" : "EA";
}

/** Série affichée sur une ligne : « hicp » (zone euro) ou « hicp_fr » (pays choisi). */
export function countrySeries(row: string, country: string | undefined): string {
  return country && hasCountries(row) && COUNTRY_CODES.includes(country) ? `${row}_${country}` : row;
}

/** Ligne et pays d'une série par pays (« hicp_fr » → ["hicp", "fr"]) ; sinon undefined. */
export function splitCountrySeries(id: string): [string, string] | undefined {
  const cut = id.lastIndexOf("_");
  const [row, country] = [id.slice(0, cut), id.slice(cut + 1)];
  return cut > 0 && hasCountries(row) && COUNTRY_CODES.includes(country) ? [row, country] : undefined;
}

/**
 * Présentation d'une série par pays : celle de l'indicateur zone euro, avec le pays dans
 * l'infobulle (« Indice des prix harmonisé (IPCH), France, glissement annuel. »).
 */
function countryDisplay(id: string): Display | undefined {
  const split = splitCountrySeries(id);
  if (!split) return undefined;
  const [row, country] = split;
  const format = FORMATS[row];
  const texts = TEXTS[lang()][row];
  if (!format || !texts) return undefined;
  const name = countryName(country);
  const hint = texts.hint.replace(lang() === "fr" ? "zone euro" : "euro area", name);
  return { ...format, ...texts, hint };
}

// ---------- Groupes : plusieurs indicateurs sur une même ligne ----------

/** Un membre d'un groupe : indicateur, code court et couleur de sa courbe. */
export type GroupMember = { id: string; code: string; color: string };
/** Un membre possible d'un groupe, avant sélection (la couleur dépend de sa place). */
type GroupCandidate = Omit<GroupMember, "color">;

export type GroupDisplay = {
  /** Titre de la vue détail et de l'image exportée. */
  label: string;
  /** Nom court, sur la ligne du panneau. */
  short: string;
  hint: string;
  explanation: string;
  members: GroupMember[];
};

/** Membres possibles de chaque groupe, dans l'ordre d'affichage (le même que dans Rust). */
const GROUP_CANDIDATES: Record<string, GroupCandidate[]> = {
  yields: [
    { id: "yield_de", code: "DE" },
    { id: "yield_fr", code: "FR" },
    { id: "yield_it", code: "IT" },
    { id: "yield_es", code: "ES" },
    { id: "yield_gr", code: "GR" },
    { id: "yield_pt", code: "PT" },
    { id: "yield_be", code: "BE" },
    { id: "yield_nl", code: "NL" },
  ],
  spreads: [
    { id: "spread_fr", code: "FR" },
    { id: "spread_it", code: "IT" },
    { id: "spread_es", code: "ES" },
    { id: "spread_gr", code: "GR" },
    { id: "spread_pt", code: "PT" },
    { id: "spread_be", code: "BE" },
    { id: "spread_nl", code: "NL" },
  ],
};

/** Couleur des courbes selon leur place (3 au plus), lisibles sur le fond nuit : or, crème, bleu gris. */
const MEMBER_COLORS = ["#c9a24a", "#f5f1e8", "#7fa2c9"];

/** Membres choisis par l'utilisateur, par groupe (envoyés par Rust ; voir `setGroupSelection`). */
const selection: Record<string, string[]> = {
  yields: ["yield_de", "yield_fr", "yield_it"],
  spreads: ["spread_fr", "spread_it", "spread_es"],
};

/** Nombre maximal de pays par ligne (place disponible ; même limite dans Rust). */
export const MAX_MEMBERS = 3;

/** Retient les membres affichés d'un groupe (ex. pays des écarts de taux). */
export function setGroupSelection(groupId: string, ids: string[]): void {
  if (ids.length > 0) selection[groupId] = ids;
}

/** Tous les membres possibles d'un groupe (écran « Indicateurs affichés »). */
export function groupCandidates(groupId: string): GroupCandidate[] {
  return GROUP_CANDIDATES[groupId] ?? [];
}

const GROUP_TEXTS: Record<Lang, Record<string, Pick<GroupDisplay, "label" | "short" | "hint" | "explanation">>> = {
  fr: {
    yields: {
      label: "Taux à 10 ans par pays",
      short: "Taux 10 ans par pays",
      hint: "Taux d'emprunt à 10 ans de chaque État, en %. Moyennes mensuelles (statistiques de convergence de la BCE).",
      explanation:
        "Ce que paie chaque État pour emprunter sur 10 ans. Plus il dépasse l'Allemagne, plus les marchés exigent une prime pour lui prêter.",
    },
    spreads: {
      label: "Écarts de taux à 10 ans",
      short: "Écarts vs Allemagne",
      hint: "Taux d'emprunt à 10 ans de chaque État moins celui de l'Allemagne (Bund), en points de base.",
      explanation:
        "La prime que paie chaque État par rapport à l'Allemagne, la référence. Elle grimpe quand les marchés doutent de ses finances publiques ou de sa stabilité politique.",
    },
  },
  en: {
    yields: {
      label: "10-year yields by country",
      short: "10Y yields by country",
      hint: "Each government's 10-year borrowing rate, in %. Monthly averages (ECB convergence statistics).",
      explanation:
        "What each government pays to borrow over 10 years. The further it sits above Germany, the bigger the premium markets demand to lend to it.",
    },
    spreads: {
      label: "10-year spreads",
      short: "Spreads vs Germany",
      hint: "Each government's 10-year borrowing rate minus Germany's (Bund), in basis points.",
      explanation:
        "The premium each government pays compared with Germany, the benchmark. It widens when markets doubt a country's public finances or political stability.",
    },
  },
};

/** Groupe d'affichage avec ses membres choisis, colorés selon leur place (undefined : groupe inconnu). */
export function groupFor(id: string): GroupDisplay | undefined {
  const candidates = GROUP_CANDIDATES[id];
  const texts = GROUP_TEXTS[lang()][id];
  if (!candidates || !texts) return undefined;
  const chosen = selection[id] ?? [];
  const members = candidates
    .filter((candidate) => chosen.includes(candidate.id))
    .slice(0, MAX_MEMBERS)
    .map((candidate, k) => ({ ...candidate, color: MEMBER_COLORS[k] }));
  return { ...texts, members };
}

// ---------- Courbe des taux ----------
// Une ligne du panneau (groupe « curve » côté Rust) : la courbe du jour en mini-graphique et
// l'écart 10 ans − 2 ans ; en vue détail, la courbe du jour et celle d'il y a 1 ou 5 ans.

/** Identifiant de la ligne « Courbe des taux ». */
export const CURVE_ID = "curve";

/** Une échéance : série BCE, code court (axe) et nom (survol). */
export type Tenor = { id: string; code: Record<Lang, string>; name: Record<Lang, string> };

/** Échéances, de la plus courte à la plus longue (même ordre que `CURVE` dans store.rs). */
export const CURVE_TENORS: Tenor[] = [
  { id: "yc_3m", code: { fr: "3M", en: "3M" }, name: { fr: "3 mois", en: "3 months" } },
  { id: "yc_6m", code: { fr: "6M", en: "6M" }, name: { fr: "6 mois", en: "6 months" } },
  { id: "yc_1y", code: { fr: "1A", en: "1Y" }, name: { fr: "1 an", en: "1 year" } },
  { id: "yc_2y", code: { fr: "2A", en: "2Y" }, name: { fr: "2 ans", en: "2 years" } },
  { id: "yc_3y", code: { fr: "3A", en: "3Y" }, name: { fr: "3 ans", en: "3 years" } },
  { id: "yc_5y", code: { fr: "5A", en: "5Y" }, name: { fr: "5 ans", en: "5 years" } },
  { id: "yc_7y", code: { fr: "7A", en: "7Y" }, name: { fr: "7 ans", en: "7 years" } },
  { id: "yc_10y", code: { fr: "10A", en: "10Y" }, name: { fr: "10 ans", en: "10 years" } },
  { id: "yc_15y", code: { fr: "15A", en: "15Y" }, name: { fr: "15 ans", en: "15 years" } },
  { id: "yc_20y", code: { fr: "20A", en: "20Y" }, name: { fr: "20 ans", en: "20 years" } },
  { id: "yc_30y", code: { fr: "30A", en: "30Y" }, name: { fr: "30 ans", en: "30 years" } },
];

const CURVE_TEXTS: Record<Lang, { label: string; hint: string; explanation: string }> = {
  fr: {
    label: "Courbe des taux AAA",
    hint: "Taux d'emprunt des États de la zone euro notés AAA, de 3 mois à 30 ans (taux zéro coupon calculés par la BCE).",
    explanation:
      "Ce qu'il coûte d'emprunter selon la durée. Une courbe inversée (court plus cher que long) annonce souvent des baisses de taux et un ralentissement.",
  },
  en: {
    label: "AAA yield curve",
    hint: "Borrowing rates of AAA-rated euro area governments, 3 months to 30 years (zero-coupon rates estimated by the ECB).",
    explanation:
      "What it costs to borrow depending on the length. An inverted curve (short above long) often signals rate cuts and a slowdown ahead.",
  },
};

/** Présentation de la courbe : textes et format des taux (en %, 2 décimales). */
export function curveDisplay(): Display {
  return { ...CURVE_TEXTS[lang()], kind: "rate", decimals: 2, monthly: false };
}

/** Présentation de la pente 2-10 ans (indicateur calculé « slope », en points de base). */
export function curveSlopeDisplay(): Display | undefined {
  return displayFor("slope");
}

export function tenorCode(tenor: Tenor): string {
  return tenor.code[lang()];
}

export function tenorName(tenor: Tenor): string {
  return tenor.name[lang()];
}
