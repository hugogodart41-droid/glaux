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
  eurusd: { kind: "fx", decimals: 4, monthly: false },
  unemployment: { kind: "rate", decimals: 1, monthly: true },
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
      hint: "Courbe des taux de la BCE, emprunts d'État de la zone euro, toutes notations.",
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
      label: "Taux réel",
      hint: "Taux de dépôt de la BCE moins l'inflation sur 1 an : le coût de l'argent une fois l'inflation déduite.",
      explanation:
        "Taux de dépôt moins inflation. Négatif, la politique monétaire reste accommodante même quand les taux montent ; positif, elle freine vraiment l'économie.",
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
      label: "Pente 10 ans − €STR",
      hint: "Écart entre le taux souverain 10 ans et le taux au jour le jour, en points de base.",
      explanation:
        "Écart entre taux longs et courts. Une pente négative (courbe inversée) a souvent précédé les ralentissements ; en hausse, elle reflète des attentes de croissance.",
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
      hint: "ECB yield curve, euro area government bonds, all ratings.",
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
      label: "Real rate",
      hint: "ECB deposit rate minus annual inflation: the cost of money after inflation.",
      explanation:
        "Deposit rate minus inflation. When negative, policy stays accommodative even as rates rise; when positive, it genuinely holds the economy back.",
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
      label: "Slope 10Y − €STR",
      hint: "Spread between the 10-year sovereign yield and the overnight rate, in basis points.",
      explanation:
        "Gap between long and short rates. A negative slope (inverted curve) has often come before slowdowns; a rising slope reflects growth expectations.",
    },
  },
};

/** Présentation d'un indicateur dans la langue actuelle (undefined : indicateur inconnu). */
export function displayFor(id: string): Display | undefined {
  const format = FORMATS[id];
  const texts = TEXTS[lang()][id];
  return format && texts ? { ...format, ...texts } : undefined;
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
const selection: Record<string, string[]> = { spreads: ["spread_fr", "spread_it", "spread_es"] };

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
    spreads: {
      label: "Écarts de taux à 10 ans",
      short: "Écarts vs Allemagne",
      hint: "Taux d'emprunt à 10 ans de chaque État moins celui de l'Allemagne (Bund), en points de base.",
      explanation:
        "La prime que paie chaque État par rapport à l'Allemagne, la référence. Elle grimpe quand les marchés doutent de ses finances publiques ou de sa stabilité politique.",
    },
  },
  en: {
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
    .slice(0, MEMBER_COLORS.length)
    .map((candidate, k) => ({ ...candidate, color: MEMBER_COLORS[k] }));
  return { ...texts, members };
}
