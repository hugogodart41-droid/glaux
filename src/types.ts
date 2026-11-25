import type { Lang } from "./i18n";

// Données reçues du cœur Rust (voir src-tauri/src/store.rs et data.rs).

export type Observation = { date: string; value: number };

export type IndicatorView = {
  id: string;
  /** Vrai pour un indicateur calculé par Glaux (taux réel, pente). */
  derived: boolean;
  /** Groupe affiché sur une seule ligne (ex. « yields » : taux à 10 ans Allemagne, France, Italie). */
  group: string | null;
  latest: Observation | null;
  /** Valeur un mois plus tôt (base de la variation). */
  reference: Observation | null;
  /** Taux directeur : date d'entrée en vigueur de la valeur actuelle. */
  since: string | null;
  /** Mini-courbe : la dernière année, points équidistants. */
  spark: number[];
};

/** Une ligne du panneau : indicateur ou groupe (« yields », « spreads », « curve »), dans l'ordre choisi. */
export type RowView = {
  id: string;
  /** Ligne calculée par Glaux (section « Calculés par Glaux »). */
  derived: boolean;
  /** Masquée par l'utilisateur (écran « Indicateurs affichés »). */
  hidden: boolean;
};

/** Dates à venir (« 2026-12-17 »), aujourd'hui compris, définies dans src-tauri/src/agenda.rs. */
export type AgendaData = { ecb: string[]; hicp: string[] };

export type DashboardView = {
  indicators: IndicatorView[];
  /** Lignes du panneau dans l'ordre choisi, masquées comprises. */
  rows: RowView[];
  /** Pays affichés dans chaque ligne à choix (1 à 3) : { yields: ["yield_de", …], spreads: ["spread_fr", …] }. */
  selections: Record<string, string[]>;
  /** Dernière actualisation complète réussie (millisecondes). */
  updatedAt: number | null;
  refreshing: boolean;
  error: string | null;
  agenda: AgendaData;
  /** Notifications activées. */
  notifications: boolean;
  /** Refus ou erreur de macOS pour les notifications (null si tout va bien). */
  notificationError: string | null;
  /** Langue de Glaux : langue du système ou choix enregistré. */
  language: Lang;
  /** Version installée (ex. « 0.1.0 »). */
  version: string;
  /** Nouvelle version disponible (null : à jour). */
  update: string | null;
  /** Mise à jour en cours de téléchargement / d'installation. */
  updating: boolean;
  /** Échec de la dernière tentative de mise à jour. */
  updateError: string | null;
};
