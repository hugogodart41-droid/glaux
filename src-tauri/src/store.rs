// Mémoire locale des données : chargée au démarrage, mise à jour depuis la BCE,
// enregistrée sur le disque pour un affichage immédiat et hors ligne.
// Emplacement : dossier de données de l'app
//   Mac     : ~/Library/Application Support/com.howzen.glaux/cache.json
//   Windows : %APPDATA%\com.howzen.glaux\cache.json
// Les préférences (alertes activées, rappels déjà envoyés, langue, indicateurs affichés)
// sont à côté, dans prefs.json.

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use chrono::{Local, Months, NaiveDate, NaiveTime, Utc};
use serde::{Deserialize, Serialize};

use crate::agenda::{self, agenda_view, due_reminders, AgendaView, Calendar};
use crate::alerts::{new_inflation_alert, rate_change_alert, Alert};
use crate::data::{
    history, real_rate, sparkline, spread, summarize, Frequency, Observation, Summary,
};
use crate::ecb::{self, BY_COUNTRY, COUNTRIES, SERIES};
use crate::i18n::Lang;

/// Au-delà de cet âge, les données sont retéléchargées.
pub const MAX_AGE: Duration = Duration::from_secs(6 * 60 * 60);

/// Historique conservé : 5 ans (vue détail) + 1 mois (variation).
const HISTORY_MONTHS: u32 = 5 * 12 + 1;

/// Période maximale affichable dans la vue détail.
pub const MAX_DETAIL_MONTHS: u32 = 5 * 12;

/// Téléchargements simultanés (29 séries : une à une, l'actualisation prendrait trop longtemps).
const PARALLEL_DOWNLOADS: usize = 4;

/// Version du format du fichier cache. À augmenter si sa structure change.
const CACHE_VERSION: u32 = 1;

/// Une série telle qu'enregistrée dans le cache.
#[derive(Clone, Serialize, Deserialize)]
struct CachedSeries {
    id: String,
    /// Clé BCE utilisée : si on change de série dans le code, l'ancien cache est ignoré.
    key: String,
    observations: Vec<Observation>,
}

/// Contenu du fichier cache.json.
#[derive(Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CacheFile {
    version: u32,
    /// Dernière actualisation où toutes les séries ont été téléchargées (secondes Unix).
    last_success: Option<i64>,
    series: Vec<CachedSeries>,
}

/// Préférences de l'utilisateur, enregistrées dans prefs.json.
#[derive(Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct Prefs {
    /// Notifications activées (oui par défaut).
    notifications: bool,
    /// Rappels du jour J déjà envoyés (ex. « ecb:2026-12-17 »), pour ne pas les répéter.
    reminded: Vec<String>,
    /// Langue choisie dans le panneau (absente : langue du système).
    language: Option<Lang>,
    /// Ordre des lignes du panneau choisi par l'utilisateur (vide : ordre par défaut).
    order: Vec<String>,
    /// Lignes masquées par l'utilisateur.
    hidden: Vec<String>,
    /// Lignes masquées par défaut (`DEFAULT_HIDDEN`) que l'utilisateur a affichées.
    shown: Vec<String>,
    /// Pays affichés dans la ligne des écarts de taux (ex. « spread_fr ») ; vide : choix par défaut.
    spreads: Vec<String>,
    /// Pays affichés dans la ligne des taux à 10 ans par pays (ex. « yield_fr ») ; vide : choix par défaut.
    yields: Vec<String>,
    /// Pays choisi pour chaque indicateur « zone euro ou par pays » (ex. « hicp » → « fr ») ;
    /// absent : zone euro.
    countries: BTreeMap<String, String>,
}

impl Default for Prefs {
    fn default() -> Self {
        Self {
            notifications: true,
            reminded: Vec::new(),
            language: None,
            order: Vec::new(),
            hidden: Vec::new(),
            shown: Vec::new(),
            spreads: Vec::new(),
            yields: Vec::new(),
            countries: BTreeMap::new(),
        }
    }
}

/// Nombre de rappels déjà envoyés gardés en mémoire (largement plus d'un an).
const MAX_REMINDED: usize = 60;

/// Indicateurs calculés par Glaux à partir des séries BCE (après les séries BCE, dans cet ordre).
/// - « real_rate » : taux réel = taux de dépôt − inflation (mensuel) ;
/// - « slope » : pente 2-10 ans = taux AAA à 10 ans − taux AAA à 2 ans (quotidien, en pb) ;
/// - « spread_xx » : écart de taux à 10 ans avec l'Allemagne (mensuel), affichés ensemble
///   dans le panneau (groupe « spreads », 3 pays au choix parmi 7).
const DERIVED: [(&str, Frequency); 9] = [
    ("real_rate", Frequency::Monthly),
    ("slope", Frequency::Daily),
    ("spread_fr", Frequency::Monthly),
    ("spread_it", Frequency::Monthly),
    ("spread_es", Frequency::Monthly),
    ("spread_gr", Frequency::Monthly),
    ("spread_pt", Frequency::Monthly),
    ("spread_be", Frequency::Monthly),
    ("spread_nl", Frequency::Monthly),
];

/// Lignes ajoutées en V3, masquées tant que l'utilisateur ne les affiche pas
/// (écran « Indicateurs affichés ») : le panneau garde sa taille habituelle.
const DEFAULT_HIDDEN: [&str; 5] = [
    "core_inflation",
    "services_inflation",
    "m3",
    "loans_nfc",
    "loans_hh",
];

/// Lignes dont l'utilisateur choisit les pays, avec le choix par défaut :
/// - « yields » : taux à 10 ans par pays (Allemagne, France, Italie) ;
/// - « spreads » : écarts de taux avec l'Allemagne (France, Italie, Espagne).
const CHOOSABLE: [(&str, [&str; 3]); 2] = [
    ("yields", ["yield_de", "yield_fr", "yield_it"]),
    ("spreads", ["spread_fr", "spread_it", "spread_es"]),
];

/// Nombre maximal de pays par ligne (place disponible, trois couleurs de courbe).
pub const MAX_MEMBERS: usize = 3;

/// Tous les membres possibles d'un groupe, dans l'ordre d'affichage (séries BCE, puis calculées).
fn group_members(group: &str) -> impl Iterator<Item = &'static str> + '_ {
    SERIES
        .iter()
        .map(|def| def.id)
        .chain(DERIVED.iter().map(|(id, _)| *id))
        .filter(move |id| group_of(id) == Some(group))
}

/// Pays par défaut d'une ligne à choix (vide pour un groupe inconnu).
fn default_members(group: &str) -> Vec<&'static str> {
    CHOOSABLE
        .iter()
        .find(|(id, _)| *id == group)
        .map(|(_, members)| members.to_vec())
        .unwrap_or_default()
}

/// Pays retenus pour une ligne à choix : identifiants connus, sans doublon, dans l'ordre
/// d'affichage, entre 1 et `MAX_MEMBERS` ; sinon, le choix par défaut.
fn arrange_members(group: &str, chosen: &[String]) -> Vec<&'static str> {
    let picked: Vec<&'static str> = group_members(group)
        .filter(|id| chosen.iter().any(|c| c == id))
        .take(MAX_MEMBERS)
        .collect();
    if picked.is_empty() {
        default_members(group)
    } else {
        picked
    }
}

/// Groupe d'affichage d'un indicateur (plusieurs indicateurs sur une même ligne du panneau) :
/// « yields » (taux à 10 ans par pays), « spreads » (écarts de taux) et « curve » (courbe AAA).
fn group_of(id: &str) -> Option<&'static str> {
    // Taux par pays : « yield_ » suivi d'un code pays (« yield_10y » est le taux zone euro).
    let country = |code: &str| code.len() == 2 && code.bytes().all(|b| b.is_ascii_lowercase());
    if id.starts_with("spread_") {
        Some("spreads")
    } else if id.starts_with("yc_") {
        Some("curve")
    } else if id.strip_prefix("yield_").is_some_and(country) {
        Some("yields")
    } else {
        None
    }
}

/// Échéances de la courbe des taux AAA, de la plus courte à la plus longue.
/// Même ordre que dans src/indicators.ts.
pub const CURVE: [&str; 11] = [
    "yc_3m", "yc_6m", "yc_1y", "yc_2y", "yc_3y", "yc_5y", "yc_7y", "yc_10y", "yc_15y", "yc_20y",
    "yc_30y",
];

/// Lignes du panneau dans l'ordre par défaut, rangées par rubrique (`Section`) : les séries
/// BCE affichées (un groupe, comme la courbe des taux, prend la place de son premier membre),
/// puis les indicateurs calculés par Glaux (les écarts de taux n'occupent qu'une ligne).
pub fn default_rows() -> Vec<&'static str> {
    let mut rows: Vec<&'static str> = Vec::new();
    let candidates = SERIES
        .iter()
        .filter_map(|def| {
            if def.shown {
                Some(def.id)
            } else {
                group_of(def.id)
            }
        })
        .chain(DERIVED.iter().map(|(id, _)| group_of(id).unwrap_or(id)));
    for row in candidates {
        if !rows.contains(&row) {
            rows.push(row);
        }
    }
    rows.sort_by_key(|row| section_of(row)); // tri stable : ordre de SERIES dans chaque rubrique
    rows
}

/// Rubrique d'une ligne du panneau, dans l'ordre d'affichage.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Section {
    /// « Zone euro » : taux de la BCE, marchés, monnaie et crédit (zone euro seulement).
    Euro,
    /// « Zone euro ou par pays » : inflation, chômage (pays au choix), taux 10 ans par pays.
    Country,
    /// « Calculés par Glaux ».
    Derived,
}

/// Rubrique d'une ligne : calculée par Glaux, disponible par pays, ou zone euro seulement.
fn section_of(row: &str) -> Section {
    let derived = DERIVED
        .iter()
        .any(|(id, _)| *id == row || group_of(id) == Some(row));
    if derived {
        Section::Derived
    } else if row == "yields" || BY_COUNTRY.iter().any(|def| def.base == row) {
        Section::Country
    } else {
        Section::Euro
    }
}

/// Pays enregistrés pour une ligne à choix (None : groupe sans choix de pays).
fn chosen_in<'a>(prefs: &'a Prefs, group: &str) -> Option<&'a Vec<String>> {
    match group {
        "spreads" => Some(&prefs.spreads),
        "yields" => Some(&prefs.yields),
        _ => None,
    }
}

/// Une ligne du panneau telle qu'envoyée à l'interface : ordre et visibilité choisis.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct RowView {
    pub id: &'static str,
    /// Rubrique de la ligne (« euro », « country », « derived »).
    pub section: Section,
    pub hidden: bool,
}

/// Range les lignes selon le choix de l'utilisateur :
/// - identifiants inconnus ignorés (fichier modifié à la main, ligne retirée de Glaux) ;
/// - lignes absentes du choix (nouvelles dans une version de Glaux) : à la fin de leur rubrique ;
/// - rubriques toujours dans le même ordre : zone euro, zone euro ou par pays, calculés ;
/// - masquée : choisie comme telle (`hidden`), ou masquée par défaut et pas affichée (`shown`) ;
/// - jamais toutes masquées (sinon le panneau serait vide) : retour à l'affichage par défaut.
fn arrange_rows(order: &[String], hidden: &[String], shown: &[String]) -> Vec<RowView> {
    let listed = |list: &[String], id: &str| list.iter().any(|item| item == id);
    let is_hidden =
        |id: &str| listed(hidden, id) || (DEFAULT_HIDDEN.contains(&id) && !listed(shown, id));
    let rank = |row: &str| order.iter().position(|id| id == row).unwrap_or(usize::MAX);
    let mut rows: Vec<RowView> = default_rows()
        .into_iter()
        .map(|id| RowView {
            id,
            section: section_of(id),
            hidden: is_hidden(id),
        })
        .collect();
    // Tri stable : à rang égal (lignes absentes du choix), l'ordre par défaut est conservé.
    rows.sort_by_key(|row| (row.section, rank(row.id)));
    if rows.iter().all(|row| row.hidden) {
        rows.iter_mut()
            .for_each(|row| row.hidden = DEFAULT_HIDDEN.contains(&row.id));
    }
    rows
}

/// Nom sûr pour un fichier exporté : l'identifiant connu correspondant (série affichée,
/// série d'un pays, indicateur calculé ou groupe), jamais le texte reçu de l'interface.
pub fn export_name(id: &str) -> Option<&'static str> {
    SERIES
        .iter()
        .filter(|def| def.shown)
        .map(|def| def.id)
        .chain(BY_COUNTRY.iter().flat_map(|def| def.ids))
        .chain(DERIVED.iter().map(|(derived, _)| *derived))
        .chain(["yields", "spreads", "curve"])
        .find(|known| *known == id)
}

/// Une courbe des taux à une date : un taux par échéance de `CURVE` (None si absent).
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CurvePoint {
    pub date: String,
    pub values: Vec<Option<f64>>,
}

/// Courbe du jour et courbe de comparaison (`months` plus tôt), pour la vue détail.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CurveView {
    pub latest: Option<CurvePoint>,
    pub past: Option<CurvePoint>,
}

/// Dernière observation au plus tard à `date` (séries triées par date, format « AAAA-MM-JJ »).
fn on_or_before<'a>(observations: &'a [Observation], date: &str) -> Option<&'a Observation> {
    let end = observations.partition_point(|o| o.date.as_str() <= date);
    end.checked_sub(1).map(|i| &observations[i])
}

/// Courbe à une date : pour chaque échéance, le dernier taux connu à cette date.
fn curve_at(cache: &CacheFile, date: &str) -> CurvePoint {
    CurvePoint {
        date: date.to_string(),
        values: CURVE
            .iter()
            .map(|id| on_or_before(cached(cache, id), date).map(|o| o.value))
            .collect(),
    }
}

/// Courbe du dernier jour publié et celle de `months` mois plus tôt (dernier jour ouvré
/// à cette date). La date de référence est celle du 10 ans AAA.
fn curve_view(cache: &CacheFile, months: u32) -> CurveView {
    let Some(last) = cached(cache, "yc_10y").last() else {
        return CurveView {
            latest: None,
            past: None,
        };
    };
    let past = NaiveDate::parse_from_str(&last.date, "%Y-%m-%d")
        .ok()
        .and_then(|d| d.checked_sub_months(Months::new(months)))
        .and_then(|target| on_or_before(cached(cache, "yc_10y"), &target.to_string()))
        .map(|o| curve_at(cache, &o.date));
    CurveView {
        latest: Some(curve_at(cache, &last.date)),
        past,
    }
}

/// Une série pour l'export Excel (src/excel.rs) : BCE (avec sa clé) ou calculée.
pub struct ExportSeries {
    pub id: &'static str,
    pub observations: Vec<Observation>,
    /// Taux en escalier : valeur en vigueur reportée sur chaque date.
    pub step: bool,
    /// Clé BCE complète (None pour un indicateur calculé par Glaux).
    pub key: Option<String>,
}

/// Un indicateur tel qu'envoyé à l'interface.
#[derive(Clone, Serialize)]
pub struct IndicatorView {
    pub id: &'static str,
    /// Vrai pour un indicateur calculé par Glaux (affiché dans une section à part).
    pub derived: bool,
    /// Groupe affiché sur une seule ligne (ex. « spreads » : écarts France, Italie, Espagne).
    pub group: Option<&'static str>,
    #[serde(flatten)]
    pub summary: Summary,
    /// Mini-courbe : valeurs de la dernière année, à intervalles réguliers.
    pub spark: Vec<f64>,
}

/// Tout ce dont l'interface a besoin pour s'afficher.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DashboardView {
    pub indicators: Vec<IndicatorView>,
    /// Dernière actualisation complète réussie (millisecondes, format JavaScript).
    pub updated_at: Option<i64>,
    pub refreshing: bool,
    /// Message de la dernière erreur, si la dernière actualisation a échoué.
    pub error: Option<String>,
    /// Lignes du panneau, dans l'ordre choisi, avec celles masquées (écran « Indicateurs affichés »).
    pub rows: Vec<RowView>,
    /// Pays affichés dans chaque ligne à choix (1 à 3) : « yields » → [« yield_de », …],
    /// « spreads » → [« spread_fr », …].
    pub selections: BTreeMap<&'static str, Vec<&'static str>>,
    /// Pays choisi pour les indicateurs « zone euro ou par pays » (ex. « hicp » → « fr ») ;
    /// absent : zone euro.
    pub countries: BTreeMap<&'static str, &'static str>,
    /// Prochaines décisions BCE et inflations flash.
    pub agenda: AgendaView,
    /// Notifications activées.
    pub notifications: bool,
    /// Refus ou erreur du système de notifications (Mac), rempli par main.rs.
    pub notification_error: Option<String>,
    /// Langue de l'interface (« fr » ou « en »).
    pub language: Lang,
    /// Version de Glaux installée (ex. « 0.1.0 »), remplie par main.rs.
    pub version: String,
    /// Nouvelle version disponible (ex. « 0.2.0 »), remplie par main.rs.
    pub update: Option<String>,
    /// Mise à jour en cours de téléchargement / d'installation.
    pub updating: bool,
    /// Échec de la dernière tentative de mise à jour.
    pub update_error: Option<String>,
}

pub struct Store {
    path: PathBuf,
    prefs_path: PathBuf,
    prefs: Mutex<Prefs>,
    agent: ureq::Agent,
    cache: Mutex<CacheFile>,
    /// Calendrier BCE / inflation en vigueur (dates intégrées ou agenda.json publié).
    calendar: Mutex<Calendar>,
    /// Copie locale du dernier agenda.json publié reçu (utilisée hors ligne).
    calendar_path: PathBuf,
    refreshing: AtomicBool,
    last_error: Mutex<Option<String>>,
    /// Langue du système, utilisée tant que l'utilisateur n'en a pas choisi une.
    system_lang: Lang,
}

/// Remet l'indicateur « actualisation en cours » à faux, même en cas d'erreur.
pub struct RefreshGuard<'a>(&'a AtomicBool);

impl Drop for RefreshGuard<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

impl Store {
    /// Charge le cache s'il existe et s'il est lisible ; sinon, part de zéro.
    /// `system_lang` : langue du système (voir `Lang::system`).
    pub fn load(dir: PathBuf, system_lang: Lang) -> Self {
        let path = dir.join("cache.json");
        let prefs_path = dir.join("prefs.json");
        let prefs = fs::read_to_string(&prefs_path)
            .ok()
            .and_then(|text| serde_json::from_str::<Prefs>(&text).ok())
            .unwrap_or_default();
        let cache = fs::read_to_string(&path)
            .ok()
            .and_then(|text| serde_json::from_str::<CacheFile>(&text).ok())
            .filter(|c| c.version == CACHE_VERSION)
            .unwrap_or_default();
        // Dernier calendrier publié reçu, s'il va plus loin que les dates intégrées.
        let calendar_path = dir.join("agenda.json");
        let calendar = fs::read_to_string(&calendar_path)
            .ok()
            .and_then(|text| Calendar::parse(&text).ok())
            .map(|saved| Calendar::builtin().merged_with(&saved))
            .unwrap_or_else(Calendar::builtin);

        Self {
            path,
            prefs_path,
            prefs: Mutex::new(prefs),
            agent: ecb::new_agent(),
            cache: Mutex::new(cache),
            calendar: Mutex::new(calendar),
            calendar_path,
            refreshing: AtomicBool::new(false),
            last_error: Mutex::new(None),
            system_lang,
        }
    }

    /// Vrai si les données ont plus de 6 h, s'il n'y en a jamais eu, ou s'il manque une série
    /// (nouvelle version de Glaux qui suit une série de plus : téléchargée sans attendre 6 h).
    pub fn needs_refresh(&self) -> bool {
        let Ok(cache) = self.cache.lock() else {
            return true;
        };
        let missing = SERIES.iter().any(|def| {
            !cache
                .series
                .iter()
                .any(|s| s.id == def.id && s.key == def.full_key())
        }) || BY_COUNTRY.iter().any(|def| {
            // Par pays : au moins un pays en cache (un pays sans donnée ne relance rien).
            !def.ids.iter().any(|id| !cached(&cache, id).is_empty())
        });
        match cache.last_success {
            None => true,
            Some(_) if missing => true,
            Some(t) => Utc::now().timestamp() - t > MAX_AGE.as_secs() as i64,
        }
    }

    /// Une requête BCE : séries reçues, prêtes pour le cache, et séries de `SERIES` absentes
    /// de la réponse (signalées comme erreur ; un pays manquant ne l'est pas).
    fn download_task(
        &self,
        download: &ecb::Download,
        start: NaiveDate,
    ) -> Result<DownloadResult, String> {
        let (received, missing) = ecb::fetch_download(&self.agent, download, start)?;
        let series = received
            .into_iter()
            .filter_map(|(id, observations)| {
                Some(CachedSeries {
                    id: id.to_string(),
                    key: ecb::series_info(id)?.key,
                    observations,
                })
            })
            .collect();
        Ok((series, missing))
    }

    /// Réserve l'actualisation. `None` si une actualisation est déjà en cours.
    pub fn try_begin_refresh(&self) -> Option<RefreshGuard<'_>> {
        self.refreshing
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .ok()
            .map(|_| RefreshGuard(&self.refreshing))
    }

    /// Télécharge toutes les séries (bloquant : à appeler hors du fil principal).
    /// Une série en échec garde ses anciennes données.
    /// Renvoie les alertes à afficher : changement du taux de dépôt, nouvelle inflation.
    pub fn download_all(&self) -> Vec<Alert> {
        let today = Local::now().date_naive();
        let start = today
            .checked_sub_months(Months::new(HISTORY_MONTHS))
            .unwrap_or(today);

        // Réseau d'abord, sans bloquer le cache (l'interface reste réactive) :
        // `PARALLEL_DOWNLOADS` fils se partagent les requêtes (séries seules ou groupées,
        // voir `ecb::downloads`), chacun prend la suivante libre.
        let plan = ecb::downloads();
        let tasks = plan.len();
        let next = AtomicUsize::new(0);
        let mut results: Vec<(usize, Result<DownloadResult, String>)> =
            std::thread::scope(|scope| {
                let workers: Vec<_> = (0..PARALLEL_DOWNLOADS)
                    .map(|_| {
                        scope.spawn(|| {
                            let mut done = Vec::new();
                            loop {
                                let i = next.fetch_add(1, Ordering::SeqCst);
                                if i >= tasks {
                                    break;
                                }
                                done.push((i, self.download_task(&plan[i], start)));
                            }
                            done
                        })
                    })
                    .collect();
                workers
                    .into_iter()
                    .flat_map(|worker| worker.join().unwrap_or_default())
                    .collect()
            });
        results.sort_by_key(|(i, _)| *i);

        let mut fresh = Vec::new();
        let mut errors = Vec::new();
        for (i, result) in results {
            match result {
                Ok((series, missing)) => {
                    fresh.extend(series);
                    errors.extend(
                        missing
                            .iter()
                            .map(|id| format!("{id} : absente de la réponse")),
                    );
                }
                Err(e) => errors.push(format!("{} : {e}", plan[i].name)),
            }
        }

        // Calendrier publié (agenda.json sur GitHub) : facultatif, une erreur n'est pas signalée.
        if let Err(e) = self.refresh_calendar() {
            eprintln!("Calendrier publié non mis à jour : {e}");
        }

        let lang = self.lang();
        let mut alerts = Vec::new();
        if let Ok(mut cache) = self.cache.lock() {
            // Dernières valeurs connues AVANT la mise à jour (aucune au tout premier lancement :
            // pas d'alerte pour des données que l'utilisateur n'avait jamais vues).
            let previous_rate = cached(&cache, "deposit_rate").last().cloned();
            let previous_inflation = cached(&cache, "hicp").last().cloned();

            cache.version = CACHE_VERSION;
            for series in fresh {
                cache.series.retain(|s| s.id != series.id);
                cache.series.push(series);
            }

            if let (Some(old), Some(new)) = (previous_rate, cached(&cache, "deposit_rate").last()) {
                alerts.extend(rate_change_alert(&old, new, lang));
            }
            if let Some(old) = previous_inflation {
                alerts.extend(new_inflation_alert(&old, cached(&cache, "hicp"), lang));
            }
            if errors.is_empty() {
                cache.last_success = Some(Utc::now().timestamp());
            }
            if let Err(e) = self.save(&cache) {
                errors.push(format!("enregistrement du cache impossible : {e}"));
            }
        }

        if let Ok(mut last_error) = self.last_error.lock() {
            *last_error = (!errors.is_empty()).then(|| errors.join(" ; "));
        }
        alerts
    }

    /// Notifications activées ?
    pub fn notifications_enabled(&self) -> bool {
        self.prefs.lock().map(|p| p.notifications).unwrap_or(true)
    }

    /// Active ou désactive les notifications (enregistré dans prefs.json).
    pub fn set_notifications(&self, enabled: bool) -> Result<(), String> {
        let mut prefs = self.prefs.lock().map_err(|_| "préférences indisponibles")?;
        prefs.notifications = enabled;
        self.save_prefs(&prefs)
    }

    /// Langue de Glaux : celle choisie dans le panneau, sinon celle du système.
    pub fn lang(&self) -> Lang {
        self.prefs
            .lock()
            .ok()
            .and_then(|p| p.language)
            .unwrap_or(self.system_lang)
    }

    /// Change la langue (enregistré dans prefs.json).
    pub fn set_language(&self, lang: Lang) -> Result<(), String> {
        let mut prefs = self.prefs.lock().map_err(|_| "préférences indisponibles")?;
        prefs.language = Some(lang);
        self.save_prefs(&prefs)
    }

    /// Lignes du panneau : ordre et visibilité choisis par l'utilisateur.
    pub fn rows(&self) -> Vec<RowView> {
        match self.prefs.lock() {
            Ok(prefs) => arrange_rows(&prefs.order, &prefs.hidden, &prefs.shown),
            Err(_) => arrange_rows(&[], &[], &[]),
        }
    }

    /// Enregistre l'ordre des lignes et celles masquées (prefs.json).
    /// `hidden` : toutes les lignes masquées à l'écran. Refuse de tout masquer.
    pub fn set_layout(&self, order: &[String], hidden: &[String]) -> Result<(), String> {
        let defaults = default_rows();
        if defaults.iter().all(|id| hidden.iter().any(|h| h == id)) {
            return Err("au moins un indicateur doit rester affiché".into());
        }
        // Tout ce qui n'est pas dans `hidden` est affiché, lignes masquées par défaut comprises.
        let shown: Vec<String> = DEFAULT_HIDDEN.iter().map(|id| id.to_string()).collect();
        let rows = arrange_rows(order, hidden, &shown);
        let mut prefs = self.prefs.lock().map_err(|_| "préférences indisponibles")?;
        // Ordre d'origine : rien d'enregistré, pour qu'une ligne ajoutée par une future
        // version de Glaux prenne sa place prévue.
        let default_order = rows.iter().map(|row| row.id).eq(defaults);
        prefs.order = if default_order {
            Vec::new()
        } else {
            rows.iter().map(|row| row.id.to_string()).collect()
        };
        // Écart avec l'affichage par défaut seulement : masquées d'habitude affichées (`shown`),
        // affichées d'habitude masquées (`hidden`).
        prefs.hidden = rows
            .iter()
            .filter(|row| row.hidden && !DEFAULT_HIDDEN.contains(&row.id))
            .map(|row| row.id.to_string())
            .collect();
        prefs.shown = rows
            .iter()
            .filter(|row| !row.hidden && DEFAULT_HIDDEN.contains(&row.id))
            .map(|row| row.id.to_string())
            .collect();
        self.save_prefs(&prefs)
    }

    /// « Par défaut » : ordre d'origine, lignes habituelles, pays par défaut.
    pub fn reset_layout(&self) -> Result<(), String> {
        let mut prefs = self.prefs.lock().map_err(|_| "préférences indisponibles")?;
        prefs.order.clear();
        prefs.hidden.clear();
        prefs.shown.clear();
        prefs.spreads.clear();
        prefs.yields.clear();
        prefs.countries.clear();
        self.save_prefs(&prefs)
    }

    /// Pays choisi pour chaque indicateur « zone euro ou par pays » (absent : zone euro).
    pub fn countries(&self) -> BTreeMap<&'static str, &'static str> {
        let Ok(prefs) = self.prefs.lock() else {
            return BTreeMap::new();
        };
        BY_COUNTRY
            .iter()
            .filter_map(|def| {
                let chosen = prefs.countries.get(def.base)?;
                let country = COUNTRIES.iter().find(|c| *c == chosen)?;
                Some((def.base, *country))
            })
            .collect()
    }

    /// Choisit le pays d'un indicateur (`None` : zone euro). Refuse un indicateur ou un
    /// pays inconnu.
    pub fn set_country(&self, row: &str, country: Option<&str>) -> Result<(), String> {
        if !BY_COUNTRY.iter().any(|def| def.base == row) {
            return Err(format!("indicateur sans choix de pays : {row}"));
        }
        if country.is_some_and(|c| !COUNTRIES.contains(&c)) {
            return Err("pays inconnu".into());
        }
        let mut prefs = self.prefs.lock().map_err(|_| "préférences indisponibles")?;
        match country {
            Some(c) => prefs.countries.insert(row.to_string(), c.to_string()),
            None => prefs.countries.remove(row),
        };
        self.save_prefs(&prefs)
    }

    /// Pays affichés dans chaque ligne à choix (« yields », « spreads »).
    pub fn selections(&self) -> BTreeMap<&'static str, Vec<&'static str>> {
        let prefs = self.prefs.lock().ok();
        CHOOSABLE
            .iter()
            .map(|(group, _)| {
                let chosen = prefs
                    .as_deref()
                    .and_then(|p| chosen_in(p, group))
                    .map(Vec::as_slice)
                    .unwrap_or_default();
                (*group, arrange_members(group, chosen))
            })
            .collect()
    }

    /// Choisit les pays d'une ligne (1 à `MAX_MEMBERS`, identifiants connus de ce groupe).
    pub fn set_selection(&self, group: &str, chosen: &[String]) -> Result<(), String> {
        let known = chosen
            .iter()
            .filter(|id| group_members(group).any(|m| m == id.as_str()))
            .count();
        if known == 0 || known > MAX_MEMBERS {
            return Err(format!("choisir entre 1 et {MAX_MEMBERS} pays"));
        }
        let members: Vec<String> = arrange_members(group, chosen)
            .into_iter()
            .map(str::to_string)
            .collect();
        let mut prefs = self.prefs.lock().map_err(|_| "préférences indisponibles")?;
        match group {
            "spreads" => prefs.spreads = members,
            "yields" => prefs.yields = members,
            _ => return Err(format!("ligne sans choix de pays : {group}")),
        }
        self.save_prefs(&prefs)
    }

    /// Courbe des taux du jour et celle de `months` mois plus tôt (1 à 5 ans).
    pub fn curve(&self, months: u32) -> CurveView {
        let months = months.clamp(1, MAX_DETAIL_MONTHS);
        match self.cache.lock() {
            Ok(cache) => curve_view(&cache, months),
            Err(_) => CurveView {
                latest: None,
                past: None,
            },
        }
    }

    /// Calendrier BCE / inflation en vigueur.
    pub fn calendar(&self) -> Calendar {
        self.calendar
            .lock()
            .map(|c| c.clone())
            .unwrap_or_else(|_| Calendar::builtin())
    }

    /// Récupère le calendrier publié sur GitHub (agenda.json) ; s'il est valide, il est gardé
    /// en copie locale et remplace, liste par liste, les dates qui vont moins loin.
    /// En cas d'échec, le calendrier en vigueur ne change pas (rien n'est signalé : les
    /// dates intégrées suffisent).
    pub fn refresh_calendar(&self) -> Result<(), String> {
        let text = agenda::fetch_remote(&self.agent)?;
        let remote = Calendar::parse(&text)?;
        if let Some(dir) = self.calendar_path.parent() {
            fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        fs::write(&self.calendar_path, &text).map_err(|e| e.to_string())?;
        if let Ok(mut calendar) = self.calendar.lock() {
            *calendar = Calendar::builtin().merged_with(&remote);
        }
        Ok(())
    }

    /// Rappels du jour J à envoyer maintenant, chacun une seule fois (même après un redémarrage).
    pub fn take_due_reminders(&self, today: NaiveDate, now: NaiveTime) -> Vec<Alert> {
        let lang = self.lang();
        let Ok(mut prefs) = self.prefs.lock() else {
            return Vec::new();
        };
        let calendar = self.calendar();
        let fresh: Vec<_> = due_reminders(&calendar, today, now, lang)
            .into_iter()
            .filter(|r| !prefs.reminded.contains(&r.key))
            .collect();
        if fresh.is_empty() {
            return Vec::new();
        }
        prefs.reminded.extend(fresh.iter().map(|r| r.key.clone()));
        let excess = prefs.reminded.len().saturating_sub(MAX_REMINDED);
        prefs.reminded.drain(..excess);
        let _ = self.save_prefs(&prefs);
        fresh.into_iter().map(|r| r.alert).collect()
    }

    fn save_prefs(&self, prefs: &Prefs) -> Result<(), String> {
        if let Some(dir) = self.prefs_path.parent() {
            fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let json = serde_json::to_string_pretty(prefs).map_err(|e| e.to_string())?;
        fs::write(&self.prefs_path, json).map_err(|e| e.to_string())
    }

    /// Écrit le cache dans un fichier temporaire puis le renomme :
    /// un arrêt brutal ne laisse jamais un cache.json à moitié écrit.
    fn save(&self, cache: &CacheFile) -> Result<(), String> {
        if let Some(dir) = self.path.parent() {
            fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let json = serde_json::to_string(cache).map_err(|e| e.to_string())?;
        let tmp = self.path.with_extension("json.tmp");
        fs::write(&tmp, json).map_err(|e| e.to_string())?;
        fs::rename(&tmp, &self.path).map_err(|e| e.to_string())
    }

    /// Historique d'un indicateur pour la vue détail (`months` limité à 5 ans).
    pub fn history(
        &self,
        id: &str,
        months: u32,
        today: NaiveDate,
    ) -> Result<Vec<Observation>, String> {
        let cache = self
            .cache
            .lock()
            .map_err(|_| "cache indisponible".to_string())?;
        let (observations, frequency, _) =
            series_data(&cache, id).ok_or_else(|| format!("indicateur inconnu : {id}"))?;
        Ok(history(
            &observations,
            frequency,
            months.clamp(1, MAX_DETAIL_MONTHS),
            today,
        ))
    }

    /// Toutes les séries (affichées, par pays, calculées) pour l'export Excel.
    pub fn export_series(&self) -> Vec<ExportSeries> {
        let Ok(cache) = self.cache.lock() else {
            return Vec::new();
        };
        SERIES
            .iter()
            .map(|def| def.id)
            .chain(BY_COUNTRY.iter().flat_map(|def| def.ids))
            .chain(DERIVED.iter().map(|(id, _)| *id))
            .filter_map(|id| {
                let (observations, _, step) = series_data(&cache, id)?;
                Some(ExportSeries {
                    id,
                    observations: observations.into_owned(),
                    step,
                    key: ecb::series_info(id).map(|info| info.key),
                })
            })
            .collect()
    }

    /// Dernier téléchargement complet réussi (secondes Unix).
    pub fn last_success(&self) -> Option<i64> {
        self.cache.lock().ok().and_then(|c| c.last_success)
    }

    /// Construit la vue envoyée à l'interface : les séries BCE du panneau, puis les indicateurs calculés.
    pub fn view(&self, today: NaiveDate) -> DashboardView {
        let cache = self.cache.lock().ok();
        let empty = CacheFile::default();
        let data = cache.as_deref().unwrap_or(&empty);

        // Séries BCE du panneau et membres de groupe (courbe des taux), séries par pays
        // (l'interface affiche celle du pays choisi, sans attendre Rust), puis calculées.
        let ids = SERIES
            .iter()
            .filter(|def| def.shown || group_of(def.id).is_some())
            .map(|def| (def.id, false))
            .chain(
                BY_COUNTRY
                    .iter()
                    .flat_map(|def| def.ids)
                    .map(|id| (id, false)),
            )
            .chain(DERIVED.iter().map(|(id, _)| (*id, true)));
        let indicators = ids
            .filter_map(|(id, derived)| {
                let (observations, frequency, step) = series_data(data, id)?;
                let group = group_of(id);
                Some(IndicatorView {
                    id,
                    derived,
                    group,
                    summary: summarize(&observations, frequency, step, today),
                    // Un membre de groupe n'a pas de mini-courbe propre : rien d'inutile à envoyer.
                    spark: match group {
                        Some(_) => Vec::new(),
                        None => sparkline(&observations, frequency, today),
                    },
                })
            })
            .collect();

        DashboardView {
            indicators,
            updated_at: data.last_success.map(|t| t * 1000),
            refreshing: self.refreshing.load(Ordering::SeqCst),
            error: self.last_error.lock().ok().and_then(|e| e.clone()),
            rows: self.rows(),
            selections: self.selections(),
            countries: self.countries(),
            agenda: agenda_view(&self.calendar(), today),
            notifications: self.notifications_enabled(),
            notification_error: None,
            language: self.lang(),
            version: String::new(),
            update: None,
            updating: false,
            update_error: None,
        }
    }
}

/// Résultat d'une requête : séries pour le cache, séries attendues mais absentes.
type DownloadResult = (Vec<CachedSeries>, Vec<&'static str>);

/// Observations en cache d'une série BCE, zone euro ou pays (vide si absente ou si sa clé
/// a changé).
fn cached<'a>(cache: &'a CacheFile, id: &str) -> &'a [Observation] {
    let Some(info) = ecb::series_info(id) else {
        return &[];
    };
    cache
        .series
        .iter()
        .find(|s| s.id == id && s.key == info.key)
        .map(|s| s.observations.as_slice())
        .unwrap_or_default()
}

/// Observations, fréquence et forme (escalier ou non) d'un indicateur, BCE ou calculé.
/// Les indicateurs calculés sont recalculés à la demande : rien de plus en cache.
fn series_data<'a>(
    cache: &'a CacheFile,
    id: &str,
) -> Option<(Cow<'a, [Observation]>, Frequency, bool)> {
    if let Some(info) = ecb::series_info(id) {
        return Some((Cow::Borrowed(cached(cache, id)), info.frequency, info.step));
    }
    let (_, frequency) = DERIVED.iter().find(|(derived, _)| *derived == id)?;
    let observations = match id {
        "real_rate" => real_rate(cached(cache, "deposit_rate"), cached(cache, "hicp")),
        "slope" => spread(cached(cache, "yc_10y"), cached(cache, "yc_2y")),
        // Écarts avec l'Allemagne (Bund) : mêmes mois des deux côtés.
        "spread_fr" => spread(cached(cache, "yield_fr"), cached(cache, "yield_de")),
        "spread_it" => spread(cached(cache, "yield_it"), cached(cache, "yield_de")),
        "spread_es" => spread(cached(cache, "yield_es"), cached(cache, "yield_de")),
        "spread_gr" => spread(cached(cache, "yield_gr"), cached(cache, "yield_de")),
        "spread_pt" => spread(cached(cache, "yield_pt"), cached(cache, "yield_de")),
        "spread_be" => spread(cached(cache, "yield_be"), cached(cache, "yield_de")),
        "spread_nl" => spread(cached(cache, "yield_nl"), cached(cache, "yield_de")),
        _ => return None,
    };
    Some((Cow::Owned(observations), *frequency, false))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn series(id: &str, data: &[(&str, f64)]) -> CachedSeries {
        CachedSeries {
            id: id.to_string(),
            key: ecb::series_info(id).unwrap().key,
            observations: data
                .iter()
                .map(|(date, value)| Observation {
                    date: date.to_string(),
                    value: *value,
                })
                .collect(),
        }
    }

    fn cache_with(series: Vec<CachedSeries>) -> CacheFile {
        CacheFile {
            version: CACHE_VERSION,
            last_success: None,
            series,
        }
    }

    #[test]
    fn indicateurs_calcules_depuis_le_cache() {
        let cache = CacheFile {
            version: CACHE_VERSION,
            last_success: None,
            series: vec![
                series("deposit_rate", &[("2025-06-11", 2.0)]),
                series("hicp", &[("2026-09", 3.8), ("2026-10", 4.0)]),
                series("yc_10y", &[("2026-11-23", 3.3209), ("2026-11-24", 3.3121)]),
                series("yc_2y", &[("2026-11-23", 3.03), ("2026-11-24", 3.0168)]),
            ],
        };

        let (real, frequency, step) = series_data(&cache, "real_rate").unwrap();
        assert_eq!(frequency, Frequency::Monthly);
        assert!(!step);
        assert_eq!(real.last().unwrap().value, -2.0);

        // Pente 2-10 ans de la courbe AAA (24/11/2026 : 3,3121 − 3,0168 = 29,5 pb).
        let (slope, frequency, _) = series_data(&cache, "slope").unwrap();
        assert_eq!(frequency, Frequency::Daily);
        assert_eq!(slope.last().unwrap().value, 0.2953);

        assert!(series_data(&cache, "inconnu").is_none());
    }

    #[test]
    fn noms_d_export_autorises() {
        assert_eq!(export_name("eurusd"), Some("eurusd"));
        assert_eq!(export_name("real_rate"), Some("real_rate")); // calculé : refusé avant la V2
        assert_eq!(export_name("spreads"), Some("spreads"));
        assert_eq!(export_name("yield_fr"), None); // membre de groupe : exporté avec « yields »
        assert_eq!(export_name("hicp_fr"), Some("hicp_fr")); // inflation française
        assert_eq!(export_name("../../etc"), None);
    }

    #[test]
    fn ecarts_de_taux_avec_l_allemagne() {
        let cache = CacheFile {
            version: CACHE_VERSION,
            last_success: Some(Utc::now().timestamp()),
            series: vec![
                series("yield_de", &[("2026-09", 3.473), ("2026-10", 3.416)]),
                series("yield_fr", &[("2026-09", 4.44), ("2026-10", 4.89)]),
                series("yield_it", &[("2026-09", 4.362), ("2026-10", 4.68)]),
                series("yield_gr", &[("2026-10", 4.41)]),
            ],
        };
        let (fr, frequency, _) = series_data(&cache, "spread_fr").unwrap();
        assert_eq!(frequency, Frequency::Monthly);
        assert_eq!(fr.len(), 2);
        assert_eq!(fr[0].value, 0.967);
        assert_eq!(fr[1].value, 1.474); // 147 pb en octobre 2026
        let (it, _, _) = series_data(&cache, "spread_it").unwrap();
        assert_eq!(it.last().unwrap().value, 1.264);
        let (gr, _, _) = series_data(&cache, "spread_gr").unwrap();
        assert_eq!(gr.last().unwrap().value, 0.994); // Grèce : 99 pb, moins que la France
                                                     // Espagne absente du cache : pas de valeur, pas de plantage.
        assert!(series_data(&cache, "spread_es").unwrap().0.is_empty());

        assert_eq!(group_of("spread_fr"), Some("spreads"));
        assert_eq!(group_of("slope"), None);
    }

    #[test]
    fn vue_avec_les_groupes_de_pays() {
        let dir = temp_dir("vue-ecarts");
        let store = Store::load(dir.clone(), Lang::Fr);
        if let Ok(mut cache) = store.cache.lock() {
            cache.series = vec![
                series("yield_de", &[("2026-10", 3.416)]),
                series("yield_fr", &[("2026-10", 4.89)]),
            ];
        }
        let view = store.view(NaiveDate::from_ymd_opt(2026, 11, 25).unwrap());
        // Les taux par pays sont envoyés comme membres de la ligne « yields », sans mini-courbe.
        let fr_yield = view.indicators.iter().find(|i| i.id == "yield_fr").unwrap();
        assert_eq!(fr_yield.group, Some("yields"));
        assert!(!fr_yield.derived && fr_yield.spark.is_empty());
        assert_eq!(fr_yield.summary.latest.as_ref().unwrap().value, 4.89);
        assert_eq!(export_name("yields"), Some("yields"));
        assert!(view.indicators.iter().any(|i| i.id == "yield_10y")); // taux zone euro : affiché
        let fr = view
            .indicators
            .iter()
            .find(|i| i.id == "spread_fr")
            .unwrap();
        assert_eq!(fr.group, Some("spreads"));
        assert!(fr.derived);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn nouvelle_serie_absente_du_cache_declenche_un_telechargement() {
        let dir = temp_dir("serie-manquante");
        let store = Store::load(dir.clone(), Lang::Fr);
        if let Ok(mut cache) = store.cache.lock() {
            cache.last_success = Some(Utc::now().timestamp()); // données fraîches…
            cache.series = SERIES
                .iter()
                .filter(|d| d.shown) // … mais sans les taux par pays (cache de la v0.1)
                .map(|d| series(d.id, &[]))
                .collect();
        }
        assert!(store.needs_refresh());
        if let Ok(mut cache) = store.cache.lock() {
            cache.series = SERIES.iter().map(|d| series(d.id, &[])).collect();
        }
        assert!(store.needs_refresh()); // séries par pays absentes (cache de la v0.2)
        if let Ok(mut cache) = store.cache.lock() {
            // Un seul pays reçu par indicateur suffit (un pays sans donnée ne relance rien).
            for def in &BY_COUNTRY {
                cache.series.push(series(def.ids[1], &[("2026-10", 1.0)]));
            }
        }
        assert!(!store.needs_refresh());
        let _ = fs::remove_dir_all(dir);
    }

    /// Dossier temporaire propre à un test.
    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("glaux-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn rappel_envoye_une_seule_fois_meme_apres_redemarrage() {
        let dir = temp_dir("rappels");
        let today = NaiveDate::from_ymd_opt(2026, 12, 17).unwrap();
        let morning = NaiveTime::from_hms_opt(9, 0, 0).unwrap();

        let store = Store::load(dir.clone(), Lang::Fr);
        assert_eq!(store.take_due_reminders(today, morning).len(), 1);
        assert!(store.take_due_reminders(today, morning).is_empty());

        // Redémarrage : prefs.json garde la trace du rappel envoyé.
        let reopened = Store::load(dir.clone(), Lang::Fr);
        assert!(reopened.take_due_reminders(today, morning).is_empty());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn preference_notifications_enregistree() {
        let dir = temp_dir("prefs");
        let store = Store::load(dir.clone(), Lang::Fr);
        assert!(store.notifications_enabled()); // activées par défaut
        store.set_notifications(false).unwrap();
        assert!(!Store::load(dir.clone(), Lang::Fr).notifications_enabled());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn langue_du_systeme_puis_choix_enregistre() {
        let dir = temp_dir("langue");
        let store = Store::load(dir.clone(), Lang::En);
        assert_eq!(store.lang(), Lang::En); // rien de choisi : langue du système
        store.set_language(Lang::Fr).unwrap();
        assert_eq!(store.lang(), Lang::Fr);
        // Redémarrage sur un système en anglais : le choix de l'utilisateur l'emporte.
        assert_eq!(Store::load(dir.clone(), Lang::En).lang(), Lang::Fr);
        let _ = fs::remove_dir_all(dir);
    }

    fn ids(rows: &[RowView]) -> Vec<&str> {
        rows.iter().map(|row| row.id).collect()
    }

    fn strings(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn lignes_par_defaut() {
        assert_eq!(
            default_rows(),
            [
                // Zone euro
                "deposit_rate",
                "estr",
                "yield_10y",
                "curve",
                "eurusd",
                "m3",
                "loans_nfc",
                "loans_hh",
                // Zone euro ou par pays
                "hicp",
                "core_inflation",
                "services_inflation",
                "unemployment",
                "yields",
                // Calculés par Glaux
                "real_rate",
                "slope",
                "spreads"
            ]
        );
        let rows = arrange_rows(&[], &[], &[]);
        assert_eq!(ids(&rows), default_rows());
        // Affichées par défaut : les 9 lignes de la v0.2, les taux par pays et la courbe ;
        // 5 nouvelles masquées.
        let visible: Vec<_> = rows.iter().filter(|r| !r.hidden).map(|r| r.id).collect();
        assert_eq!(
            visible,
            [
                "deposit_rate",
                "estr",
                "yield_10y",
                "curve",
                "eurusd",
                "hicp",
                "unemployment",
                "yields",
                "real_rate",
                "slope",
                "spreads"
            ]
        );
        let section = |id: &str| rows.iter().find(|r| r.id == id).unwrap().section;
        assert_eq!(section("deposit_rate"), Section::Euro);
        assert_eq!(section("curve"), Section::Euro); // courbe AAA : zone euro seulement
        assert_eq!(section("m3"), Section::Euro);
        assert_eq!(section("hicp"), Section::Country);
        assert_eq!(section("unemployment"), Section::Country);
        assert_eq!(section("yields"), Section::Country);
        assert_eq!(section("real_rate"), Section::Derived); // calculé avec l'inflation zone euro
        assert_eq!(section("spreads"), Section::Derived);
        assert_eq!(
            serde_json::to_string(&Section::Country).unwrap(),
            "\"country\""
        );
    }

    #[test]
    fn ordre_choisi_par_section_et_lignes_nouvelles_a_la_fin() {
        // L'utilisateur met les écarts en tête et l'inflation avant le taux de dépôt ;
        // « estr » et d'autres manquent (version plus ancienne), « inconnu » n'existe pas.
        let order = strings(&["spreads", "inconnu", "hicp", "deposit_rate", "slope"]);
        let rows = arrange_rows(&order, &[], &[]);
        assert_eq!(
            ids(&rows),
            [
                "deposit_rate",
                "estr", // absentes du choix : en fin de rubrique, ordre par défaut
                "yield_10y",
                "curve",
                "eurusd",
                "m3",
                "loans_nfc",
                "loans_hh",
                "hicp", // l'inflation reste dans sa rubrique « zone euro ou par pays »
                "core_inflation",
                "services_inflation",
                "unemployment",
                "yields",
                "spreads", // les calculés restent en dernier
                "slope",
                "real_rate"
            ]
        );
    }

    #[test]
    fn jamais_toutes_les_lignes_masquees() {
        let all = strings(&default_rows());
        // Tout masqué (fichier abîmé) : retour à l'affichage par défaut.
        let rows = arrange_rows(&[], &all, &[]);
        assert_eq!(rows.iter().filter(|row| !row.hidden).count(), 11);
        let dir = temp_dir("tout-masque");
        let store = Store::load(dir.clone(), Lang::Fr);
        assert!(store.set_layout(&[], &all).is_err());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn choix_d_affichage_enregistre_puis_reinitialise() {
        let dir = temp_dir("lignes");
        let store = Store::load(dir.clone(), Lang::Fr);
        let order = strings(&["eurusd", "deposit_rate", "spreads"]);
        store
            .set_layout(&order, &strings(&["unemployment", "slope", "inconnu"]))
            .unwrap();

        // Redémarrage : le choix est relu depuis prefs.json.
        let reopened = Store::load(dir.clone(), Lang::Fr);
        let rows = reopened.rows();
        assert_eq!(&ids(&rows)[..2], ["eurusd", "deposit_rate"]);
        assert_eq!(rows[13].id, "spreads");
        // Masquées : celles choisies ; les 5 nouvelles sont affichées (absentes de `hidden`).
        let hidden: Vec<_> = rows.iter().filter(|r| r.hidden).map(|r| r.id).collect();
        assert_eq!(hidden, ["unemployment", "slope"]);
        let view = reopened.view(NaiveDate::from_ymd_opt(2026, 11, 25).unwrap());
        assert_eq!(view.rows, rows);

        // « Par défaut » : rien d'enregistré, ordre d'origine, nouvelles lignes de nouveau masquées.
        reopened.reset_layout().unwrap();
        let rows = reopened.rows();
        assert_eq!(ids(&rows), default_rows());
        assert_eq!(
            rows.iter().filter(|r| r.hidden).count(),
            DEFAULT_HIDDEN.len()
        );
        let prefs = fs::read_to_string(dir.join("prefs.json")).unwrap();
        assert!(prefs.contains("\"order\": []"));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn ligne_masquee_par_defaut_affichee_puis_retenue() {
        let dir = temp_dir("masquees-par-defaut");
        let store = Store::load(dir.clone(), Lang::Fr);
        let hidden_now: Vec<String> = store
            .rows()
            .iter()
            .filter(|r| r.hidden && r.id != "core_inflation")
            .map(|r| r.id.to_string())
            .collect();
        let order: Vec<String> = store.rows().iter().map(|r| r.id.to_string()).collect();
        store.set_layout(&order, &hidden_now).unwrap();

        let rows = Store::load(dir.clone(), Lang::Fr).rows();
        let core = rows.iter().find(|r| r.id == "core_inflation").unwrap();
        assert!(!core.hidden); // affichée par l'utilisateur, même après redémarrage
        assert!(rows.iter().find(|r| r.id == "m3").unwrap().hidden); // les autres restent masquées
        let prefs = fs::read_to_string(dir.join("prefs.json")).unwrap();
        assert!(prefs.contains("core_inflation") && prefs.contains("\"order\": []"));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn courbe_du_jour_et_d_il_y_a_un_an() {
        fn day(d: &str, v: f64) -> (&str, f64) {
            (d, v)
        }
        let cache = cache_with(vec![
            series(
                "yc_2y",
                &[
                    day("2025-11-21", 2.05),
                    day("2025-11-24", 2.0),
                    day("2026-11-24", 3.2),
                ],
            ),
            series(
                "yc_10y",
                &[
                    day("2025-11-21", 2.62),
                    day("2025-11-24", 2.6),
                    day("2026-11-23", 3.95),
                    day("2026-11-24", 3.93),
                ],
            ),
            series("yc_30y", &[day("2025-11-24", 3.1), day("2026-11-20", 4.3)]),
        ]);
        let view = curve_view(&cache, 12);
        let latest = view.latest.unwrap();
        assert_eq!(latest.date, "2026-11-24");
        assert_eq!(latest.values.len(), CURVE.len());
        assert_eq!(latest.values[3], Some(3.2)); // 2 ans
        assert_eq!(latest.values[7], Some(3.93)); // 10 ans
        assert_eq!(latest.values[10], Some(4.3)); // 30 ans : dernier taux connu (20 nov.)
        assert_eq!(latest.values[0], None); // 3 mois absent du cache
        let past = view.past.unwrap();
        assert_eq!(past.date, "2025-11-24");
        assert_eq!(past.values[3], Some(2.0));
        assert_eq!(past.values[10], Some(3.1));

        // Pas assez d'historique : pas de courbe de comparaison.
        let view = curve_view(&cache, 60);
        assert!(view.latest.is_some() && view.past.is_none());
        assert_eq!(
            curve_view(&cache_with(vec![]), 12),
            CurveView {
                latest: None,
                past: None
            }
        );
    }

    #[test]
    fn membres_de_la_courbe_dans_la_vue_sans_mini_courbe() {
        let cache = cache_with(vec![
            series("yc_2y", &[("2026-11-23", 3.21), ("2026-11-24", 3.2)]),
            series("yc_10y", &[("2026-11-23", 3.32), ("2026-11-24", 3.31)]),
            series("yield_10y", &[("2026-11-23", 3.95), ("2026-11-24", 3.93)]),
        ]);
        let dir = temp_dir("courbe-vue");
        let store = Store::load(dir.clone(), Lang::Fr);
        *store.cache.lock().unwrap() = cache;
        let view = store.view(NaiveDate::from_ymd_opt(2026, 11, 25).unwrap());
        let member = view.indicators.iter().find(|i| i.id == "yc_2y").unwrap();
        assert_eq!(member.group, Some("curve"));
        assert!(member.spark.is_empty());
        let ten = view
            .indicators
            .iter()
            .find(|i| i.id == "yield_10y")
            .unwrap();
        assert_eq!(ten.group, None); // le 10 ans toutes notations garde sa ligne et sa mini-courbe
        assert!(!ten.spark.is_empty());
        let aaa = view.indicators.iter().find(|i| i.id == "yc_10y").unwrap();
        assert_eq!(aaa.group, Some("curve")); // le 10 ans AAA appartient à la courbe
        assert_eq!(export_name("curve"), Some("curve"));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn pays_des_lignes_a_choix() {
        assert_eq!(
            arrange_members("spreads", &[]),
            ["spread_fr", "spread_it", "spread_es"]
        );
        assert_eq!(
            arrange_members("yields", &[]),
            ["yield_de", "yield_fr", "yield_it"]
        );
        // Ordre d'affichage, sans doublon ni inconnu, 3 au plus.
        let chosen = strings(&[
            "spread_nl",
            "spread_gr",
            "spread_gr",
            "spread_xx",
            "spread_fr",
        ]);
        assert_eq!(
            arrange_members("spreads", &chosen),
            ["spread_fr", "spread_gr", "spread_nl"]
        );
        // Un écart ne peut pas entrer dans la ligne des taux, et inversement.
        assert_eq!(
            arrange_members("yields", &strings(&["spread_fr", "yield_es"])),
            ["yield_es"]
        );
        assert_eq!(group_members("spreads").count(), 7);
        assert_eq!(group_members("yields").count(), 8); // Allemagne comprise
        assert_eq!(group_of("yield_10y"), None); // taux zone euro : sa propre ligne
        assert_eq!(group_of("yield_de"), Some("yields"));

        let dir = temp_dir("pays");
        let store = Store::load(dir.clone(), Lang::Fr);
        let defaults = store.selections();
        assert_eq!(defaults["spreads"], ["spread_fr", "spread_it", "spread_es"]);
        assert_eq!(defaults["yields"], ["yield_de", "yield_fr", "yield_it"]);
        store
            .set_selection("spreads", &strings(&["spread_gr", "spread_pt"]))
            .unwrap();
        store
            .set_selection("yields", &strings(&["yield_nl", "yield_gr"]))
            .unwrap();
        let reopened = Store::load(dir.clone(), Lang::Fr).selections();
        assert_eq!(reopened["spreads"], ["spread_gr", "spread_pt"]);
        assert_eq!(reopened["yields"], ["yield_gr", "yield_nl"]);
        assert!(store.set_selection("spreads", &[]).is_err());
        assert!(store
            .set_selection("yields", &strings(&["spread_fr"]))
            .is_err());
        assert!(store.set_selection("curve", &strings(&["yc_10y"])).is_err());
        assert!(store
            .set_selection(
                "spreads",
                &strings(&["spread_fr", "spread_it", "spread_es", "spread_gr"])
            )
            .is_err());
        store.reset_layout().unwrap();
        assert_eq!(store.selections(), defaults);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn pays_choisi_par_indicateur() {
        let dir = temp_dir("pays-indicateur");
        let store = Store::load(dir.clone(), Lang::Fr);
        assert!(store.countries().is_empty()); // zone euro par défaut
        store.set_country("hicp", Some("fr")).unwrap();
        store.set_country("unemployment", Some("es")).unwrap();
        assert!(store.set_country("eurusd", Some("fr")).is_err()); // zone euro seulement
        assert!(store.set_country("hicp", Some("us")).is_err());
        let reopened = Store::load(dir.clone(), Lang::Fr);
        let countries = reopened.countries();
        assert_eq!(countries.get("hicp"), Some(&"fr"));
        assert_eq!(countries.get("unemployment"), Some(&"es"));
        reopened.set_country("hicp", None).unwrap(); // retour à la zone euro
        assert_eq!(reopened.countries().get("hicp"), None);
        reopened.reset_layout().unwrap();
        assert!(reopened.countries().is_empty());

        // Les séries par pays arrivent dans la vue, sans mini-courbe manquante.
        if let Ok(mut cache) = reopened.cache.lock() {
            cache.series = vec![
                series("hicp", &[("2026-09", 3.8), ("2026-10", 4.0)]),
                series("hicp_fr", &[("2026-09", 3.2), ("2026-10", 3.7)]),
            ];
        }
        let view = reopened.view(NaiveDate::from_ymd_opt(2026, 11, 25).unwrap());
        let fr = view.indicators.iter().find(|i| i.id == "hicp_fr").unwrap();
        assert_eq!(fr.summary.latest.as_ref().unwrap().value, 3.7);
        assert!(fr.group.is_none() && !fr.derived);
        assert!(view.indicators.iter().any(|i| i.id == "unemployment_nl")); // vide mais présent
        assert_eq!(
            reopened
                .history(
                    "hicp_fr",
                    12,
                    NaiveDate::from_ymd_opt(2026, 11, 25).unwrap()
                )
                .unwrap()
                .len(),
            2
        );
        // Le taux réel reste calculé avec l'inflation de la zone euro (2,5 − 4,0), même si
        // l'utilisateur affiche l'inflation française.
        reopened.set_country("hicp", Some("fr")).unwrap();
        if let Ok(mut cache) = reopened.cache.lock() {
            cache
                .series
                .push(series("deposit_rate", &[("2026-09-17", 2.5)]));
            let (real, _, _) = series_data(&cache, "real_rate").unwrap();
            assert_eq!(real.last().unwrap().value, -1.5);
        }
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn calendrier_publie_garde_hors_ligne() {
        let dir = temp_dir("calendrier");
        fs::create_dir_all(&dir).unwrap();
        // Copie locale d'un agenda.json qui va jusqu'en 2028 : reprise au lancement suivant.
        fs::write(
            dir.join("agenda.json"),
            r#"{"ecb_decisions": ["2027-12-16", "2028-02-03"], "hicp_flash": ["2028-01-05", "2028-02-01"]}"#,
        )
        .unwrap();
        let store = Store::load(dir.clone(), Lang::Fr);
        let view = store.view(NaiveDate::from_ymd_opt(2028, 1, 10).unwrap());
        assert_eq!(view.agenda.ecb, ["2028-02-03"]);
        assert_eq!(view.agenda.hicp, ["2028-02-01"]);
        // Copie illisible : dates intégrées.
        fs::write(dir.join("agenda.json"), "{").unwrap();
        let store = Store::load(dir.clone(), Lang::Fr);
        assert_eq!(store.calendar(), Calendar::builtin());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn une_cle_bce_modifiee_invalide_le_cache() {
        let mut stale = series("hicp", &[("2026-10", 4.0)]);
        stale.key = "ICP.M.U2.N.000000.4.ANR".to_string(); // ancienne série
        let cache = CacheFile {
            version: CACHE_VERSION,
            last_success: None,
            series: vec![stale],
        };
        assert!(cached(&cache, "hicp").is_empty());
    }
}
