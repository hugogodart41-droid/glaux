// Accès à l'API publique de la BCE (ECB Data Portal) : gratuite, sans clé ni compte.
// Doc : https://data.ecb.europa.eu/help/api/data

use std::time::Duration;

use chrono::NaiveDate;

use crate::data::{parse_period, Frequency, Observation};

const API_BASE: &str = "https://data-api.ecb.europa.eu/service/data";

/// Une série BCE suivie par Glaux.
pub struct SeriesDef {
    /// Identifiant interne, partagé avec l'interface (src/main.ts).
    pub id: &'static str,
    /// Jeu de données BCE (« dataflow »).
    pub flow: &'static str,
    /// Clé de la série dans ce jeu de données.
    pub key: &'static str,
    pub frequency: Frequency,
    /// Taux qui ne bouge qu'aux décisions de la BCE (courbe en escalier).
    pub step: bool,
    /// Affichée comme une ligne du panneau ; sinon, sert seulement aux indicateurs calculés.
    pub shown: bool,
}

impl SeriesDef {
    /// Clé complète, telle qu'affichée sur le portail de la BCE.
    pub fn full_key(&self) -> String {
        format!("{}.{}", self.flow, self.key)
    }
}

/// Séries téléchargées : les 6 indicateurs affichés (dans l'ordre du panneau), puis les taux
/// à 10 ans par pays, qui servent aux écarts de taux (calculés dans store.rs).
pub const SERIES: [SeriesDef; 10] = [
    // Taux de la facilité de dépôt : le taux directeur de référence de la BCE.
    SeriesDef {
        id: "deposit_rate",
        flow: "FM",
        key: "D.U2.EUR.4F.KR.DFR.LEV",
        frequency: Frequency::Daily,
        step: true,
        shown: true,
    },
    // Inflation IPCH, zone euro, glissement annuel (jeu « HICP » qui remplace « ICP » depuis 2026).
    SeriesDef {
        id: "hicp",
        flow: "HICP",
        key: "M.U2.N.000000.4D0.ANR",
        frequency: Frequency::Monthly,
        step: false,
        shown: true,
    },
    // €STR : taux interbancaire au jour le jour, moyenne tronquée pondérée par les volumes.
    SeriesDef {
        id: "estr",
        flow: "EST",
        key: "B.EU000A2X2A25.WT",
        frequency: Frequency::Daily,
        step: false,
        shown: true,
    },
    // Taux souverain 10 ans : courbe des taux BCE, toutes notations (taux au comptant).
    SeriesDef {
        id: "yield_10y",
        flow: "YC",
        key: "B.U2.EUR.4F.G_N_C.SV_C_YM.SR_10Y",
        frequency: Frequency::Daily,
        step: false,
        shown: true,
    },
    // Cours de référence EUR/USD (publié vers 16 h, heure de Francfort).
    SeriesDef {
        id: "eurusd",
        flow: "EXR",
        key: "D.USD.EUR.SP00.A",
        frequency: Frequency::Daily,
        step: false,
        shown: true,
    },
    // Taux de chômage, zone euro, 15-74 ans, corrigé des variations saisonnières.
    SeriesDef {
        id: "unemployment",
        flow: "LFSI",
        key: "M.U2.S.UNEHRT.TOTAL0.15_74.T",
        frequency: Frequency::Monthly,
        step: false,
        shown: true,
    },
    // Taux d'emprunt à 10 ans par pays : statistiques de convergence (critères de Maastricht),
    // moyennes mensuelles publiées par la BCE, environ 3 semaines après la fin du mois.
    SeriesDef {
        id: "yield_de",
        flow: "IRS",
        key: "M.DE.L.L40.CI.0000.EUR.N.Z",
        frequency: Frequency::Monthly,
        step: false,
        shown: false,
    },
    SeriesDef {
        id: "yield_fr",
        flow: "IRS",
        key: "M.FR.L.L40.CI.0000.EUR.N.Z",
        frequency: Frequency::Monthly,
        step: false,
        shown: false,
    },
    SeriesDef {
        id: "yield_it",
        flow: "IRS",
        key: "M.IT.L.L40.CI.0000.EUR.N.Z",
        frequency: Frequency::Monthly,
        step: false,
        shown: false,
    },
    SeriesDef {
        id: "yield_es",
        flow: "IRS",
        key: "M.ES.L.L40.CI.0000.EUR.N.Z",
        frequency: Frequency::Monthly,
        step: false,
        shown: false,
    },
];

/// Client HTTP réutilisé pour toutes les requêtes.
pub fn new_agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        // Au-delà, on considère la BCE injoignable (le cache reste affiché).
        .timeout_global(Some(Duration::from_secs(20)))
        // On lit nous-mêmes le code HTTP pour donner un message clair.
        .http_status_as_error(false)
        .user_agent(concat!("Glaux/", env!("CARGO_PKG_VERSION")))
        .build()
        .into()
}

/// Lit le CSV de la BCE (format=csvdata) et renvoie les observations triées par date.
/// Seules les colonnes TIME_PERIOD et OBS_VALUE sont utilisées ; les valeurs
/// manquantes (vides, « NaN ») sont ignorées.
pub fn parse_csv(body: &str) -> Result<Vec<Observation>, String> {
    let mut reader = csv::ReaderBuilder::new()
        .flexible(true)
        .from_reader(body.as_bytes());

    let headers = reader
        .headers()
        .map_err(|e| format!("CSV illisible : {e}"))?
        .clone();
    let column = |name: &str| {
        headers
            .iter()
            .position(|h| h.trim().trim_start_matches('\u{feff}') == name)
            .ok_or_else(|| format!("colonne {name} absente"))
    };
    let date_col = column("TIME_PERIOD")?;
    let value_col = column("OBS_VALUE")?;

    let mut observations = Vec::new();
    for record in reader.records() {
        let record = record.map_err(|e| format!("CSV illisible : {e}"))?;
        let (Some(date), Some(raw)) = (record.get(date_col), record.get(value_col)) else {
            continue;
        };
        let Ok(value) = raw.trim().parse::<f64>() else {
            continue;
        };
        if value.is_finite() && parse_period(date).is_some() {
            observations.push(Observation {
                date: date.trim().to_string(),
                value,
            });
        }
    }

    observations.sort_by_key(|o| parse_period(&o.date));
    observations.dedup_by(|a, b| a.date == b.date);
    Ok(observations)
}

/// Télécharge une URL de l'API et renvoie les observations.
/// `Ok(None)` : la BCE répond qu'il n'y a aucune donnée (HTTP 404).
fn get(agent: &ureq::Agent, url: &str) -> Result<Option<Vec<Observation>>, String> {
    let mut response = agent
        .get(url)
        .call()
        .map_err(|e| format!("BCE injoignable ({e})"))?;

    match response.status().as_u16() {
        200 => {
            let body = response
                .body_mut()
                .read_to_string()
                .map_err(|e| format!("réponse interrompue ({e})"))?;
            parse_csv(&body).map(Some)
        }
        404 => Ok(None),
        code => Err(format!("la BCE a répondu avec le code HTTP {code}")),
    }
}

/// Télécharge une série depuis `start` (inclus).
///
/// Pour un taux en escalier, on ajoute la dernière valeur connue avant `start`
/// (sinon la courbe commencerait au premier changement de taux de la période).
pub fn fetch_series(
    agent: &ureq::Agent,
    def: &SeriesDef,
    start: NaiveDate,
) -> Result<Vec<Observation>, String> {
    let base = format!(
        "{API_BASE}/{}/{}?format=csvdata&detail=dataonly",
        def.flow, def.key
    );

    let mut observations = get(agent, &format!("{base}&startPeriod={start}"))?
        .ok_or_else(|| format!("série {} introuvable", def.full_key()))?;

    if def.step {
        // Requête facultative : en cas d'échec, on garde simplement la période demandée.
        let anchor_url = format!("{base}&endPeriod={start}&lastNObservations=1");
        if let Ok(Some(before)) = get(agent, &anchor_url) {
            let anchor = before
                .into_iter()
                .rev()
                .find(|o| parse_period(&o.date).is_some_and(|d| d < start));
            if let Some(anchor) = anchor {
                observations.insert(0, anchor);
            }
        }
    }

    if observations.is_empty() {
        return Err(format!("aucune donnée pour {}", def.full_key()));
    }
    Ok(observations)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lit_un_csv_bce() {
        let csv =
            "\u{feff}KEY,FREQ,CURRENCY,CURRENCY_DENOM,EXR_TYPE,EXR_SUFFIX,TIME_PERIOD,OBS_VALUE\n\
                   EXR.D.USD.EUR.SP00.A,D,USD,EUR,SP00,A,2026-11-20,1.0532\n\
                   EXR.D.USD.EUR.SP00.A,D,USD,EUR,SP00,A,2026-11-19,1.0498\n\
                   EXR.D.USD.EUR.SP00.A,D,USD,EUR,SP00,A,2026-11-18,NaN\n\
                   EXR.D.USD.EUR.SP00.A,D,USD,EUR,SP00,A,2026-11-17,\n";
        let obs = parse_csv(csv).unwrap();
        assert_eq!(obs.len(), 2);
        // Triées par date croissante, valeurs manquantes ignorées.
        assert_eq!(obs[0].date, "2026-11-19");
        assert_eq!(obs[1].value, 1.0532);
    }

    #[test]
    fn lit_les_champs_entre_guillemets() {
        let csv = "KEY,TITLE,TIME_PERIOD,OBS_VALUE\n\
                   X,\"Taux, zone euro\",2026-10,2.1\n";
        let obs = parse_csv(csv).unwrap();
        assert_eq!(obs.len(), 1);
        assert_eq!(obs[0].date, "2026-10");
    }

    #[test]
    fn refuse_un_csv_sans_colonnes_attendues() {
        assert!(parse_csv("A,B\n1,2\n").is_err());
    }

    /// Test réel contre l'API de la BCE (nécessite Internet), ignoré par défaut.
    /// Lancer depuis src-tauri :  cargo test -- --ignored --nocapture
    #[test]
    #[ignore]
    fn telecharge_les_6_series_depuis_la_bce() {
        let agent = new_agent();
        let start = chrono::Local::now().date_naive() - chrono::Months::new(3);
        let mut failures = Vec::new();
        for def in &SERIES {
            match fetch_series(&agent, def, start) {
                Ok(obs) => {
                    let last = obs.last().unwrap();
                    println!(
                        "OK   {:<13} {:>4} valeurs, dernière : {} = {}",
                        def.id,
                        obs.len(),
                        last.date,
                        last.value
                    );
                }
                Err(e) => {
                    println!("ÉCHEC {:<12} {e}", def.id);
                    failures.push(def.id);
                }
            }
        }
        assert!(failures.is_empty(), "séries en échec : {failures:?}");
    }

    #[test]
    fn series_avec_identifiants_uniques() {
        let mut ids: Vec<_> = SERIES.iter().map(|s| s.id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), SERIES.len());
    }
}
