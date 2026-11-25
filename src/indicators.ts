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
  // Calculés par Glaux (src-tauri/src/data.rs)
  real_rate: { kind: "rate", decimals: 2, monthly: true },
  slope: { kind: "bp", decimals: 2, monthly: false },
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
    real_rate: {
      label: "Taux réel",
      hint: "Taux de dépôt de la BCE moins l'inflation sur 1 an : le coût de l'argent une fois l'inflation déduite.",
      explanation:
        "Taux de dépôt moins inflation. Négatif, la politique monétaire reste accommodante même quand les taux montent ; positif, elle freine vraiment l'économie.",
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
    real_rate: {
      label: "Real rate",
      hint: "ECB deposit rate minus annual inflation: the cost of money after inflation.",
      explanation:
        "Deposit rate minus inflation. When negative, policy stays accommodative even as rates rise; when positive, it genuinely holds the economy back.",
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
