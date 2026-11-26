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

/// Séries téléchargées, dans l'ordre par défaut du panneau (rangé ensuite par rubrique,
/// voir `section_of` dans store.rs) :
/// - les 11 indicateurs du panneau (5 masqués tant que l'utilisateur ne les affiche pas,
///   voir `DEFAULT_HIDDEN` dans store.rs) ; inflation et chômage existent aussi par pays
///   (`BY_COUNTRY`, plus bas) ;
/// - les taux à 10 ans par pays (ligne « Taux à 10 ans par pays », 3 pays au choix), qui
///   servent aussi aux écarts de taux (calculés dans store.rs) ;
/// - les taux de la courbe AAA (3 mois → 30 ans), affichés ensemble sur la ligne
///   « Courbe des taux ».
pub const SERIES: [SeriesDef; 30] = [
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
    // Inflation sous-jacente : IPCH hors énergie, alimentation, alcool et tabac, glissement annuel.
    SeriesDef {
        id: "core_inflation",
        flow: "HICP",
        key: "M.U2.N.XEF000.4D0.ANR",
        frequency: Frequency::Monthly,
        step: false,
        shown: true,
    },
    // Inflation des services, glissement annuel.
    SeriesDef {
        id: "services_inflation",
        flow: "HICP",
        key: "M.U2.N.SERV00.4D0.ANR",
        frequency: Frequency::Monthly,
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
    // Taux à 10 ans par pays (statistiques de convergence, moyennes mensuelles) : groupe
    // « yields » (ligne « Taux à 10 ans par pays ») et base des écarts de taux (store.rs).
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
    SeriesDef {
        id: "yield_gr",
        flow: "IRS",
        key: "M.GR.L.L40.CI.0000.EUR.N.Z",
        frequency: Frequency::Monthly,
        step: false,
        shown: false,
    },
    SeriesDef {
        id: "yield_pt",
        flow: "IRS",
        key: "M.PT.L.L40.CI.0000.EUR.N.Z",
        frequency: Frequency::Monthly,
        step: false,
        shown: false,
    },
    SeriesDef {
        id: "yield_be",
        flow: "IRS",
        key: "M.BE.L.L40.CI.0000.EUR.N.Z",
        frequency: Frequency::Monthly,
        step: false,
        shown: false,
    },
    SeriesDef {
        id: "yield_nl",
        flow: "IRS",
        key: "M.NL.L.L40.CI.0000.EUR.N.Z",
        frequency: Frequency::Monthly,
        step: false,
        shown: false,
    },
    // Courbe des taux AAA de la BCE (emprunts d'État de la zone euro les mieux notés, proches du
    // sans-risque), de 3 mois à 30 ans : taux zéro coupon quotidiens (modèle de Svensson).
    // Membres du groupe « curve » (store.rs), sans ligne propre. Le 10 ans ci-dessus, lui, porte
    // sur toutes les notations (taux moyen de la zone euro) : ce n'est pas le même que yc_10y.
    SeriesDef {
        id: "yc_3m",
        flow: "YC",
        key: "B.U2.EUR.4F.G_N_A.SV_C_YM.SR_3M",
        frequency: Frequency::Daily,
        step: false,
        shown: false,
    },
    SeriesDef {
        id: "yc_6m",
        flow: "YC",
        key: "B.U2.EUR.4F.G_N_A.SV_C_YM.SR_6M",
        frequency: Frequency::Daily,
        step: false,
        shown: false,
    },
    SeriesDef {
        id: "yc_1y",
        flow: "YC",
        key: "B.U2.EUR.4F.G_N_A.SV_C_YM.SR_1Y",
        frequency: Frequency::Daily,
        step: false,
        shown: false,
    },
    SeriesDef {
        id: "yc_2y",
        flow: "YC",
        key: "B.U2.EUR.4F.G_N_A.SV_C_YM.SR_2Y",
        frequency: Frequency::Daily,
        step: false,
        shown: false,
    },
    SeriesDef {
        id: "yc_3y",
        flow: "YC",
        key: "B.U2.EUR.4F.G_N_A.SV_C_YM.SR_3Y",
        frequency: Frequency::Daily,
        step: false,
        shown: false,
    },
    SeriesDef {
        id: "yc_5y",
        flow: "YC",
        key: "B.U2.EUR.4F.G_N_A.SV_C_YM.SR_5Y",
        frequency: Frequency::Daily,
        step: false,
        shown: false,
    },
    SeriesDef {
        id: "yc_7y",
        flow: "YC",
        key: "B.U2.EUR.4F.G_N_A.SV_C_YM.SR_7Y",
        frequency: Frequency::Daily,
        step: false,
        shown: false,
    },
    SeriesDef {
        id: "yc_10y",
        flow: "YC",
        key: "B.U2.EUR.4F.G_N_A.SV_C_YM.SR_10Y",
        frequency: Frequency::Daily,
        step: false,
        shown: false,
    },
    SeriesDef {
        id: "yc_15y",
        flow: "YC",
        key: "B.U2.EUR.4F.G_N_A.SV_C_YM.SR_15Y",
        frequency: Frequency::Daily,
        step: false,
        shown: false,
    },
    SeriesDef {
        id: "yc_20y",
        flow: "YC",
        key: "B.U2.EUR.4F.G_N_A.SV_C_YM.SR_20Y",
        frequency: Frequency::Daily,
        step: false,
        shown: false,
    },
    SeriesDef {
        id: "yc_30y",
        flow: "YC",
        key: "B.U2.EUR.4F.G_N_A.SV_C_YM.SR_30Y",
        frequency: Frequency::Daily,
        step: false,
        shown: false,
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
    // Masse monétaire M3 : croissance sur 1 an (bilans des banques, publiée vers le 28 du mois suivant).
    SeriesDef {
        id: "m3",
        flow: "BSI",
        key: "M.U2.Y.V.M30.X.I.U2.2300.Z01.A",
        frequency: Frequency::Monthly,
        step: false,
        shown: true,
    },
    // Prêts des banques aux entreprises non financières, croissance sur 1 an
    // (corrigée des cessions et titrisations de prêts).
    SeriesDef {
        id: "loans_nfc",
        flow: "BSI",
        key: "M.U2.Y.U.A20T.A.I.U2.2240.Z01.A",
        frequency: Frequency::Monthly,
        step: false,
        shown: true,
    },
    // Prêts des banques aux ménages, croissance sur 1 an (même correction).
    SeriesDef {
        id: "loans_hh",
        flow: "BSI",
        key: "M.U2.Y.U.A20T.A.I.U2.2250.Z01.A",
        frequency: Frequency::Monthly,
        step: false,
        shown: true,
    },
    // Taux d'emprunt à 10 ans par pays : statistiques de convergence (critères de Maastricht),
    // moyennes mensuelles publiées par la BCE, environ 3 semaines après la fin du mois.
];

/// Pays proposés pour les indicateurs « zone euro ou par pays » (codes ISO en minuscules,
/// ordre d'affichage) : les 4 grandes économies de la zone euro, puis 4 pays suivis de près
/// par les marchés. Même liste que les taux à 10 ans par pays.
pub const COUNTRIES: [&str; 8] = ["de", "fr", "it", "es", "gr", "pt", "be", "nl"];

/// Identifiants « {indicateur}_{pays} », dans l'ordre de `COUNTRIES` (ex. « hicp_fr »).
macro_rules! by_country {
    ($base:literal) => {
        [
            concat!($base, "_de"),
            concat!($base, "_fr"),
            concat!($base, "_it"),
            concat!($base, "_es"),
            concat!($base, "_gr"),
            concat!($base, "_pt"),
            concat!($base, "_be"),
            concat!($base, "_nl"),
        ]
    };
}

/// Un indicateur du panneau disponible aussi pour chaque pays de `COUNTRIES`.
///
/// La zone euro et les 8 pays arrivent en une seule requête (clé « M.U2+DE+FR+….N.000000.4D0.ANR »,
/// voir `downloads`).
pub struct CountryDef {
    /// Ligne du panneau (série zone euro de `SERIES`) : « hicp », « unemployment »…
    pub base: &'static str,
    /// Identifiant de chaque pays, dans l'ordre de `COUNTRIES` (ex. « hicp_fr »).
    pub ids: [&'static str; 8],
    pub flow: &'static str,
    /// Clé avant et après le code pays : « M. » + « FR » + « .N.000000.4D0.ANR ».
    pub prefix: &'static str,
    pub suffix: &'static str,
    pub frequency: Frequency,
}

impl CountryDef {
    /// Clé complète d'un pays (« de ») ou de plusieurs (« DE+FR+… »).
    pub fn full_key(&self, countries: &str) -> String {
        format!(
            "{}.{}{}{}",
            self.flow,
            self.prefix,
            countries.to_uppercase(),
            self.suffix
        )
    }
}

/// Indicateurs par pays : mêmes définitions que la série zone euro correspondante.
pub const BY_COUNTRY: [CountryDef; 4] = [
    // Inflation IPCH sur 1 an.
    CountryDef {
        base: "hicp",
        ids: by_country!("hicp"),
        flow: "HICP",
        prefix: "M.",
        suffix: ".N.000000.4D0.ANR",
        frequency: Frequency::Monthly,
    },
    // Inflation sous-jacente (hors énergie, alimentation, alcool et tabac).
    CountryDef {
        base: "core_inflation",
        ids: by_country!("core_inflation"),
        flow: "HICP",
        prefix: "M.",
        suffix: ".N.XEF000.4D0.ANR",
        frequency: Frequency::Monthly,
    },
    // Inflation des services.
    CountryDef {
        base: "services_inflation",
        ids: by_country!("services_inflation"),
        flow: "HICP",
        prefix: "M.",
        suffix: ".N.SERV00.4D0.ANR",
        frequency: Frequency::Monthly,
    },
    // Taux de chômage, 15-74 ans, CVS.
    CountryDef {
        base: "unemployment",
        ids: by_country!("unemployment"),
        flow: "LFSI",
        prefix: "M.",
        suffix: ".S.UNEHRT.TOTAL0.15_74.T",
        frequency: Frequency::Monthly,
    },
];

/// Ce qu'il faut savoir d'une série téléchargée (zone euro ou pays).
pub struct SeriesInfo {
    /// Clé BCE complète (pour vérifier que le cache correspond à la définition actuelle).
    pub key: String,
    pub frequency: Frequency,
    pub step: bool,
}

/// Clé, fréquence et forme d'une série de `SERIES` ou de `BY_COUNTRY` (None : inconnue).
pub fn series_info(id: &str) -> Option<SeriesInfo> {
    if let Some(def) = SERIES.iter().find(|d| d.id == id) {
        return Some(SeriesInfo {
            key: def.full_key(),
            frequency: def.frequency,
            step: def.step,
        });
    }
    BY_COUNTRY.iter().find_map(|def| {
        let k = def.ids.iter().position(|c| *c == id)?;
        Some(SeriesInfo {
            key: def.full_key(COUNTRIES[k]),
            frequency: def.frequency,
            step: false,
        })
    })
}

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
    let mut observations: Vec<Observation> =
        parse_rows(body)?.into_iter().map(|(_, o)| o).collect();
    sort_by_date(&mut observations);
    Ok(observations)
}

/// Lit un CSV de la BCE qui contient plusieurs séries : observations triées par date, pour
/// chaque clé complète de la colonne KEY (ex. « HICP.M.FR.N.000000.4D0.ANR »).
pub fn parse_csv_by_key(body: &str) -> Result<Vec<(String, Vec<Observation>)>, String> {
    let mut series: Vec<(String, Vec<Observation>)> = Vec::new();
    for (key, observation) in parse_rows(body)? {
        match series.iter_mut().find(|(k, _)| *k == key) {
            Some((_, list)) => list.push(observation),
            None => series.push((key, vec![observation])),
        }
    }
    for (_, list) in &mut series {
        sort_by_date(list);
    }
    Ok(series)
}

/// Tri par date, sans doublon.
fn sort_by_date(observations: &mut Vec<Observation>) {
    observations.sort_by_key(|o| parse_period(&o.date));
    observations.dedup_by(|a, b| a.date == b.date);
}

/// Lignes du CSV : (clé de la colonne KEY, vide si absente ; observation).
fn parse_rows(body: &str) -> Result<Vec<(String, Observation)>, String> {
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
    let key_col = column("KEY").ok();

    let mut rows = Vec::new();
    for record in reader.records() {
        let record = record.map_err(|e| format!("CSV illisible : {e}"))?;
        let (Some(date), Some(raw)) = (record.get(date_col), record.get(value_col)) else {
            continue;
        };
        let Ok(value) = raw.trim().parse::<f64>() else {
            continue;
        };
        if value.is_finite() && parse_period(date).is_some() {
            let key = key_col
                .and_then(|col| record.get(col))
                .unwrap_or_default()
                .trim()
                .to_string();
            rows.push((
                key,
                Observation {
                    date: date.trim().to_string(),
                    value,
                },
            ));
        }
    }
    Ok(rows)
}

/// Télécharge une URL de l'API et renvoie le CSV reçu.
/// `Ok(None)` : la BCE répond qu'il n'y a aucune donnée (HTTP 404).
fn get_csv(agent: &ureq::Agent, url: &str) -> Result<Option<String>, String> {
    let mut response = agent
        .get(url)
        .call()
        .map_err(|e| format!("BCE injoignable ({e})"))?;

    match response.status().as_u16() {
        200 => response
            .body_mut()
            .read_to_string()
            .map(Some)
            .map_err(|e| format!("réponse interrompue ({e})")),
        404 => Ok(None),
        code => Err(format!("la BCE a répondu avec le code HTTP {code}")),
    }
}

/// Télécharge une URL de l'API et renvoie les observations (`Ok(None)` : aucune donnée).
fn get(agent: &ureq::Agent, url: &str) -> Result<Option<Vec<Observation>>, String> {
    match get_csv(agent, url)? {
        Some(body) => parse_csv(&body).map(Some),
        None => Ok(None),
    }
}

// ---------- Téléchargements groupés ----------
// La BCE met environ 2 s à répondre à chaque requête, quelle que soit la quantité de
// données. Les séries dont les clés ne diffèrent que par une dimension (pays, échéance…)
// partent donc ensemble : « YC.B.U2.EUR.4F.G_N_A.SV_C_YM.SR_3M+SR_6M+… » ramène toute la
// courbe. 12 requêtes au lieu de 34.

/// Une requête BCE : une série seule, ou un groupe de séries (identifiant, clé complète).
pub struct Download {
    /// Nom pour les messages d'erreur : l'identifiant d'une série seule, sinon le groupe.
    pub name: &'static str,
    pub series: Vec<(&'static str, String)>,
}

/// Groupe de téléchargement d'une série (None : seule dans sa requête).
fn download_group(id: &str) -> Option<&'static str> {
    if id.starts_with("yc_") {
        return Some("curve"); // 11 échéances de la courbe AAA
    }
    if id.starts_with("yield_") && id != "yield_10y" {
        return Some("yields"); // taux à 10 ans des 8 pays
    }
    if id == "loans_nfc" || id == "loans_hh" {
        return Some("loans"); // crédit aux entreprises et aux ménages
    }
    // Inflation, sous-jacente, services, chômage : zone euro et 8 pays ensemble.
    BY_COUNTRY
        .iter()
        .find(|def| def.base == id || def.ids.contains(&id))
        .map(|def| def.base)
}

/// Toutes les requêtes d'une actualisation, dans l'ordre de `SERIES` puis de `BY_COUNTRY`.
pub fn downloads() -> Vec<Download> {
    let ids = SERIES
        .iter()
        .map(|def| def.id)
        .chain(BY_COUNTRY.iter().flat_map(|def| def.ids));
    let mut plan: Vec<Download> = Vec::new();
    for id in ids {
        let Some(info) = series_info(id) else {
            continue;
        };
        // Un taux en escalier (taux de dépôt) a sa propre requête complémentaire : seul.
        let group = download_group(id).filter(|_| !info.step);
        match group.and_then(|g| plan.iter_mut().find(|d| d.name == g)) {
            Some(download) => download.series.push((id, info.key)),
            None => plan.push(Download {
                name: group.unwrap_or(id),
                series: vec![(id, info.key)],
            }),
        }
    }
    plan
}

/// Clé d'une requête pour plusieurs séries : dimensions communes, et « A+B+… » pour la seule
/// qui varie. None si les clés diffèrent sur plus d'une dimension (la BCE renverrait aussi
/// des combinaisons non voulues) ou n'ont pas la même forme.
pub fn combined_key(keys: &[String]) -> Option<String> {
    let split: Vec<Vec<&str>> = keys.iter().map(|k| k.split('.').collect()).collect();
    let first = split.first()?;
    if split.iter().any(|parts| parts.len() != first.len()) {
        return None;
    }
    let mut varying = 0;
    let mut combined = Vec::with_capacity(first.len());
    for position in 0..first.len() {
        let mut values: Vec<&str> = Vec::new();
        for parts in &split {
            if !values.contains(&parts[position]) {
                values.push(parts[position]);
            }
        }
        if values.len() > 1 {
            varying += 1;
        }
        combined.push(values.join("+"));
    }
    (varying <= 1).then(|| combined.join("."))
}

/// Réponse d'une requête : séries reçues (identifiant, observations), et identifiants des
/// séries de `SERIES` absentes.
pub type Received = (Vec<(&'static str, Vec<Observation>)>, Vec<&'static str>);

/// Exécute une requête depuis `start` (inclus) : séries reçues (identifiant, observations),
/// et noms de celles de `SERIES` absentes de la réponse (une série par pays peut manquer
/// sans erreur : affichée « — »).
pub fn fetch_download(
    agent: &ureq::Agent,
    download: &Download,
    start: NaiveDate,
) -> Result<Received, String> {
    // Série seule : requête habituelle (avec la valeur d'avant la période pour un taux en escalier).
    if let [(id, _)] = download.series.as_slice() {
        if let Some(def) = SERIES.iter().find(|def| def.id == *id) {
            return fetch_series(agent, def, start).map(|o| (vec![(*id, o)], Vec::new()));
        }
    }
    let keys: Vec<String> = download.series.iter().map(|(_, key)| key.clone()).collect();
    let key =
        combined_key(&keys).ok_or_else(|| format!("clés incompatibles ({})", download.name))?;
    let (flow, rest) = key.split_once('.').ok_or("clé BCE invalide")?;
    let url =
        format!("{API_BASE}/{flow}/{rest}?format=csvdata&detail=dataonly&startPeriod={start}");
    let body =
        get_csv(agent, &url)?.ok_or_else(|| format!("séries {} introuvables", download.name))?;
    let mut received = parse_csv_by_key(&body)?;

    let mut found = Vec::new();
    let mut missing = Vec::new();
    for (id, key) in &download.series {
        let observations = received
            .iter_mut()
            .find(|(k, _)| k == key)
            .map(|(_, o)| std::mem::take(o))
            .unwrap_or_default();
        if !observations.is_empty() {
            found.push((*id, observations));
        } else if SERIES.iter().any(|def| def.id == *id) {
            missing.push(*id);
        }
    }
    if found.is_empty() {
        return Err(format!("aucune donnée pour {key}"));
    }
    Ok((found, missing))
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
    fn telecharge_toutes_les_series_depuis_la_bce() {
        let agent = new_agent();
        let start = chrono::Local::now().date_naive() - chrono::Months::new(3);
        let mut failures = Vec::new();
        for download in downloads() {
            match fetch_download(&agent, &download, start) {
                Ok((series, missing)) => {
                    for (id, obs) in &series {
                        let last = obs.last().unwrap();
                        println!(
                            "OK   {id:<22} {:>4} valeurs, dernière : {} = {}",
                            obs.len(),
                            last.date,
                            last.value
                        );
                    }
                    failures.extend(missing);
                }
                Err(e) => {
                    println!("ÉCHEC {:<21} {e}", download.name);
                    failures.push(download.name);
                }
            }
        }
        assert!(failures.is_empty(), "séries en échec : {failures:?}");
    }

    #[test]
    fn series_avec_identifiants_uniques() {
        let mut ids: Vec<_> = SERIES
            .iter()
            .map(|s| s.id)
            .chain(BY_COUNTRY.iter().flat_map(|d| d.ids))
            .collect();
        let total = ids.len();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), total);
        assert_eq!(total, 30 + 4 * 8);
        // Chaque indicateur par pays a sa série zone euro, de même fréquence.
        for def in &BY_COUNTRY {
            let euro = SERIES.iter().find(|s| s.id == def.base).unwrap();
            assert_eq!(euro.frequency, def.frequency);
        }
    }

    #[test]
    fn series_par_pays() {
        // Identifiants dans l'ordre de COUNTRIES : « hicp_fr » est le 2e.
        assert_eq!(BY_COUNTRY[0].ids[1], "hicp_fr");
        assert_eq!(BY_COUNTRY[3].ids[7], "unemployment_nl");
        let info = series_info("core_inflation_it").unwrap();
        assert_eq!(info.key, "HICP.M.IT.N.XEF000.4D0.ANR");
        assert_eq!(
            BY_COUNTRY[3].full_key("DE+FR"),
            "LFSI.M.DE+FR.S.UNEHRT.TOTAL0.15_74.T"
        );
        assert_eq!(
            series_info("hicp").unwrap().key,
            "HICP.M.U2.N.000000.4D0.ANR"
        );
        assert!(series_info("inconnu").is_none());
    }

    #[test]
    fn lit_un_csv_avec_plusieurs_series() {
        let csv = "KEY,FREQ,REF_AREA,ADJUSTMENT,TIME_PERIOD,OBS_VALUE\n\
            LFSI.M.FR.S,M,FR,S,2026-09,8.5\n\
            LFSI.M.DE.S,M,DE,S,2026-09,4.1\n\
            LFSI.M.FR.S,M,FR,S,2026-08,8.3\n\
            LFSI.M.DE.S,M,DE,S,2026-08,NaN\n";
        let series = parse_csv_by_key(csv).unwrap();
        assert_eq!(series.len(), 2);
        let (key, fr) = &series[0];
        assert_eq!(key, "LFSI.M.FR.S");
        assert_eq!(fr.len(), 2);
        assert_eq!(fr[0].date, "2026-08"); // trié par date
        assert_eq!(series[1].1.len(), 1); // valeur manquante ignorée
    }

    #[test]
    fn requetes_groupees() {
        let plan = downloads();
        // 5 séries seules + 7 groupes (inflation, sous-jacente, services, chômage — zone euro
        // et pays —, taux par pays, courbe, crédit) = 12 requêtes.
        let names: Vec<_> = plan.iter().map(|d| d.name).collect();
        assert_eq!(
            names,
            [
                "deposit_rate",
                "hicp",
                "core_inflation",
                "services_inflation",
                "unemployment",
                "estr",
                "yield_10y",
                "yields",
                "curve",
                "eurusd",
                "m3",
                "loans"
            ]
        );
        // Toutes les séries une seule fois, et chaque groupe forme une clé valide.
        let total: usize = plan.iter().map(|d| d.series.len()).sum();
        assert_eq!(total, SERIES.len() + 4 * 8);
        for download in &plan {
            let keys: Vec<String> = download.series.iter().map(|(_, k)| k.clone()).collect();
            assert!(
                combined_key(&keys).is_some(),
                "groupe {} invalide",
                download.name
            );
        }
        let curve = plan.iter().find(|d| d.name == "curve").unwrap();
        let keys: Vec<String> = curve.series.iter().map(|(_, k)| k.clone()).collect();
        assert_eq!(
            combined_key(&keys).unwrap(),
            "YC.B.U2.EUR.4F.G_N_A.SV_C_YM.SR_3M+SR_6M+SR_1Y+SR_2Y+SR_3Y+SR_5Y+SR_7Y+SR_10Y+SR_15Y+SR_20Y+SR_30Y"
        );
        let hicp = plan.iter().find(|d| d.name == "hicp").unwrap();
        let keys: Vec<String> = hicp.series.iter().map(|(_, k)| k.clone()).collect();
        assert_eq!(
            combined_key(&keys).unwrap(),
            "HICP.M.U2+DE+FR+IT+ES+GR+PT+BE+NL.N.000000.4D0.ANR"
        );
        // Taux de dépôt (en escalier) : seul.
        assert_eq!(plan[0].series.len(), 1);
    }

    #[test]
    fn cle_combinee_sur_une_seule_dimension() {
        let keys = |list: &[&str]| list.iter().map(|k| k.to_string()).collect::<Vec<_>>();
        assert_eq!(
            combined_key(&keys(&["A.M.FR.X", "A.M.DE.X"])).as_deref(),
            Some("A.M.FR+DE.X")
        );
        assert_eq!(
            combined_key(&keys(&["A.M.FR.X"])).as_deref(),
            Some("A.M.FR.X")
        );
        assert_eq!(combined_key(&keys(&["A.M.FR.X", "A.Q.DE.X"])), None); // deux dimensions
        assert_eq!(combined_key(&keys(&["A.M.FR.X", "A.M.FR"])), None); // formes différentes
        assert_eq!(combined_key(&[]), None);
    }
}
