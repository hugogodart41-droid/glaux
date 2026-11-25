// Mémoire locale des données : chargée au démarrage, mise à jour depuis la BCE,
// enregistrée sur le disque pour un affichage immédiat et hors ligne.
// Emplacement : dossier de données de l'app
//   Mac     : ~/Library/Application Support/com.howzen.glaux/cache.json
//   Windows : %APPDATA%\com.howzen.glaux\cache.json
// Les préférences (alertes activées, rappels déjà envoyés, langue, indicateurs affichés)
// sont à côté, dans prefs.json.

use std::borrow::Cow;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use chrono::{Local, Months, NaiveDate, NaiveTime, Utc};
use serde::{Deserialize, Serialize};

use crate::agenda::{agenda_view, due_reminders, AgendaView};
use crate::alerts::{new_inflation_alert, rate_change_alert, Alert};
use crate::data::{
    history, real_rate, sparkline, spread, summarize, Frequency, Observation, Summary,
};
use crate::ecb::{self, SERIES};
use crate::i18n::Lang;

/// Au-delà de cet âge, les données sont retéléchargées.
pub const MAX_AGE: Duration = Duration::from_secs(6 * 60 * 60);

/// Historique conservé : 5 ans (vue détail) + 1 mois (variation).
const HISTORY_MONTHS: u32 = 5 * 12 + 1;

/// Période maximale affichable dans la vue détail.
pub const MAX_DETAIL_MONTHS: u32 = 5 * 12;

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
    /// Pays affichés dans la ligne des écarts de taux (ex. « spread_fr ») ; vide : `DEFAULT_SPREADS`.
    spreads: Vec<String>,
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
        }
    }
}

/// Nombre de rappels déjà envoyés gardés en mémoire (largement plus d'un an).
const MAX_REMINDED: usize = 60;

/// Indicateurs calculés par Glaux à partir des séries BCE (après les séries BCE, dans cet ordre).
/// - « real_rate » : taux réel = taux de dépôt − inflation (mensuel) ;
/// - « slope » : pente des taux = taux souverain 10 ans − €STR (quotidien) ;
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

/// Pays de la ligne des écarts de taux par défaut, et nombre maximal (place disponible).
const DEFAULT_SPREADS: [&str; 3] = ["spread_fr", "spread_it", "spread_es"];
pub const MAX_SPREADS: usize = 3;

/// Tous les écarts de taux disponibles, dans l'ordre d'affichage.
fn all_spreads() -> impl Iterator<Item = &'static str> {
    DERIVED
        .iter()
        .map(|(id, _)| *id)
        .filter(|id| group_of(id) == Some("spreads"))
}

/// Pays retenus pour la ligne des écarts : identifiants connus, sans doublon, dans l'ordre
/// d'affichage, entre 1 et `MAX_SPREADS` ; sinon, le choix par défaut.
fn arrange_spreads(chosen: &[String]) -> Vec<&'static str> {
    let picked: Vec<&'static str> = all_spreads()
        .filter(|id| chosen.iter().any(|c| c == id))
        .take(MAX_SPREADS)
        .collect();
    if picked.is_empty() {
        DEFAULT_SPREADS.to_vec()
    } else {
        picked
    }
}

/// Groupe d'affichage d'un indicateur (plusieurs indicateurs sur une même ligne du panneau).
fn group_of(id: &str) -> Option<&'static str> {
    id.starts_with("spread_").then_some("spreads")
}

/// Lignes du panneau dans l'ordre par défaut : les séries BCE affichées, puis les indicateurs
/// calculés par Glaux (un groupe n'occupe qu'une ligne, ex. « spreads »).
pub fn default_rows() -> Vec<&'static str> {
    let mut rows: Vec<&'static str> = SERIES
        .iter()
        .filter(|def| def.shown)
        .map(|def| def.id)
        .collect();
    for (id, _) in &DERIVED {
        let row = group_of(id).unwrap_or(id);
        if !rows.contains(&row) {
            rows.push(row);
        }
    }
    rows
}

/// Une ligne du panneau telle qu'envoyée à l'interface : ordre et visibilité choisis.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct RowView {
    pub id: &'static str,
    /// Ligne calculée par Glaux (section « Calculés par Glaux »).
    pub derived: bool,
    pub hidden: bool,
}

/// Range les lignes selon le choix de l'utilisateur :
/// - identifiants inconnus ignorés (fichier modifié à la main, ligne retirée de Glaux) ;
/// - lignes absentes du choix (nouvelles dans une version de Glaux) : à la fin de leur section ;
/// - les séries BCE restent toujours avant les indicateurs calculés ;
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
            derived: !SERIES.iter().any(|def| def.id == id),
            hidden: is_hidden(id),
        })
        .collect();
    // Tri stable : à rang égal (lignes absentes du choix), l'ordre par défaut est conservé.
    rows.sort_by_key(|row| (row.derived, rank(row.id)));
    if rows.iter().all(|row| row.hidden) {
        rows.iter_mut()
            .for_each(|row| row.hidden = DEFAULT_HIDDEN.contains(&row.id));
    }
    rows
}

/// Nom sûr pour un fichier exporté : l'identifiant connu correspondant (série affichée,
/// indicateur calculé ou groupe), jamais le texte reçu de l'interface.
pub fn export_name(id: &str) -> Option<&'static str> {
    SERIES
        .iter()
        .filter(|def| def.shown)
        .map(|def| def.id)
        .chain(DERIVED.iter().map(|(derived, _)| *derived))
        .chain(["spreads"])
        .find(|known| *known == id)
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
    /// Écarts de taux affichés dans la ligne « spreads » (1 à 3 pays, ex. « spread_fr »).
    pub spreads: Vec<&'static str>,
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

        Self {
            path,
            prefs_path,
            prefs: Mutex::new(prefs),
            agent: ecb::new_agent(),
            cache: Mutex::new(cache),
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
        });
        match cache.last_success {
            None => true,
            Some(_) if missing => true,
            Some(t) => Utc::now().timestamp() - t > MAX_AGE.as_secs() as i64,
        }
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

        // Réseau d'abord, sans bloquer le cache (l'interface reste réactive).
        let mut fresh = Vec::new();
        let mut errors = Vec::new();
        for def in &SERIES {
            match ecb::fetch_series(&self.agent, def, start) {
                Ok(observations) => fresh.push(CachedSeries {
                    id: def.id.to_string(),
                    key: def.full_key(),
                    observations,
                }),
                Err(e) => errors.push(format!("{} : {e}", def.id)),
            }
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

    /// « Par défaut » : ordre d'origine, lignes habituelles, pays par défaut des écarts.
    pub fn reset_layout(&self) -> Result<(), String> {
        let mut prefs = self.prefs.lock().map_err(|_| "préférences indisponibles")?;
        prefs.order.clear();
        prefs.hidden.clear();
        prefs.shown.clear();
        prefs.spreads.clear();
        self.save_prefs(&prefs)
    }

    /// Écarts de taux affichés dans la ligne « spreads ».
    pub fn spreads(&self) -> Vec<&'static str> {
        match self.prefs.lock() {
            Ok(prefs) => arrange_spreads(&prefs.spreads),
            Err(_) => DEFAULT_SPREADS.to_vec(),
        }
    }

    /// Choisit les pays de la ligne des écarts (1 à `MAX_SPREADS`, identifiants connus).
    pub fn set_spreads(&self, chosen: &[String]) -> Result<(), String> {
        let known = chosen
            .iter()
            .filter(|id| all_spreads().any(|s| s == id.as_str()))
            .count();
        if known == 0 || known > MAX_SPREADS {
            return Err(format!("choisir entre 1 et {MAX_SPREADS} pays"));
        }
        let mut prefs = self.prefs.lock().map_err(|_| "préférences indisponibles")?;
        prefs.spreads = arrange_spreads(chosen)
            .into_iter()
            .map(str::to_string)
            .collect();
        self.save_prefs(&prefs)
    }

    /// Rappels du jour J à envoyer maintenant, chacun une seule fois (même après un redémarrage).
    pub fn take_due_reminders(&self, today: NaiveDate, now: NaiveTime) -> Vec<Alert> {
        let lang = self.lang();
        let Ok(mut prefs) = self.prefs.lock() else {
            return Vec::new();
        };
        let fresh: Vec<_> = due_reminders(today, now, lang)
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

    /// Toutes les séries (affichées, taux par pays, calculées) pour l'export Excel.
    pub fn export_series(&self) -> Vec<ExportSeries> {
        let Ok(cache) = self.cache.lock() else {
            return Vec::new();
        };
        SERIES
            .iter()
            .map(|def| def.id)
            .chain(DERIVED.iter().map(|(id, _)| *id))
            .filter_map(|id| {
                let (observations, _, step) = series_data(&cache, id)?;
                Some(ExportSeries {
                    id,
                    observations: observations.into_owned(),
                    step,
                    key: SERIES.iter().find(|d| d.id == id).map(|d| d.full_key()),
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

        let ids = SERIES
            .iter()
            .filter(|def| def.shown)
            .map(|def| (def.id, false))
            .chain(DERIVED.iter().map(|(id, _)| (*id, true)));
        let indicators = ids
            .filter_map(|(id, derived)| {
                let (observations, frequency, step) = series_data(data, id)?;
                Some(IndicatorView {
                    id,
                    derived,
                    group: group_of(id),
                    summary: summarize(&observations, frequency, step, today),
                    spark: sparkline(&observations, frequency, today),
                })
            })
            .collect();

        DashboardView {
            indicators,
            updated_at: data.last_success.map(|t| t * 1000),
            refreshing: self.refreshing.load(Ordering::SeqCst),
            error: self.last_error.lock().ok().and_then(|e| e.clone()),
            rows: self.rows(),
            spreads: self.spreads(),
            agenda: agenda_view(today),
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

/// Observations en cache d'une série BCE (vide si absente ou si sa clé a changé).
fn cached<'a>(cache: &'a CacheFile, id: &str) -> &'a [Observation] {
    let Some(def) = SERIES.iter().find(|d| d.id == id) else {
        return &[];
    };
    cache
        .series
        .iter()
        .find(|s| s.id == def.id && s.key == def.full_key())
        .map(|s| s.observations.as_slice())
        .unwrap_or_default()
}

/// Observations, fréquence et forme (escalier ou non) d'un indicateur, BCE ou calculé.
/// Les indicateurs calculés sont recalculés à la demande : rien de plus en cache.
fn series_data<'a>(
    cache: &'a CacheFile,
    id: &str,
) -> Option<(Cow<'a, [Observation]>, Frequency, bool)> {
    if let Some(def) = SERIES.iter().find(|d| d.id == id) {
        return Some((Cow::Borrowed(cached(cache, id)), def.frequency, def.step));
    }
    let (_, frequency) = DERIVED.iter().find(|(derived, _)| *derived == id)?;
    let observations = match id {
        "real_rate" => real_rate(cached(cache, "deposit_rate"), cached(cache, "hicp")),
        "slope" => spread(cached(cache, "yield_10y"), cached(cache, "estr")),
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
        let def = SERIES.iter().find(|d| d.id == id).unwrap();
        CachedSeries {
            id: id.to_string(),
            key: def.full_key(),
            observations: data
                .iter()
                .map(|(date, value)| Observation {
                    date: date.to_string(),
                    value: *value,
                })
                .collect(),
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
                series("yield_10y", &[("2026-11-23", 3.93)]),
                series("estr", &[("2026-11-20", 2.443)]),
            ],
        };

        let (real, frequency, step) = series_data(&cache, "real_rate").unwrap();
        assert_eq!(frequency, Frequency::Monthly);
        assert!(!step);
        assert_eq!(real.last().unwrap().value, -2.0);

        let (slope, _, _) = series_data(&cache, "slope").unwrap();
        assert_eq!(slope.last().unwrap().value, 1.487);

        assert!(series_data(&cache, "inconnu").is_none());
    }

    #[test]
    fn noms_d_export_autorises() {
        assert_eq!(export_name("eurusd"), Some("eurusd"));
        assert_eq!(export_name("real_rate"), Some("real_rate")); // calculé : refusé avant la V2
        assert_eq!(export_name("spreads"), Some("spreads"));
        assert_eq!(export_name("yield_fr"), None); // série cachée
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
    fn vue_sans_les_taux_par_pays_et_avec_le_groupe() {
        let dir = temp_dir("vue-ecarts");
        let store = Store::load(dir.clone(), Lang::Fr);
        if let Ok(mut cache) = store.cache.lock() {
            cache.series = vec![
                series("yield_de", &[("2026-10", 3.416)]),
                series("yield_fr", &[("2026-10", 4.89)]),
            ];
        }
        let view = store.view(NaiveDate::from_ymd_opt(2026, 11, 25).unwrap());
        // Les taux par pays servent au calcul mais ne sont pas des lignes du panneau.
        let country_yields = [
            "yield_de", "yield_fr", "yield_it", "yield_es", "yield_gr", "yield_pt", "yield_be",
            "yield_nl",
        ];
        assert!(view
            .indicators
            .iter()
            .all(|i| !country_yields.contains(&i.id)));
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
                "deposit_rate",
                "hicp",
                "core_inflation",
                "services_inflation",
                "estr",
                "yield_10y",
                "eurusd",
                "unemployment",
                "m3",
                "loans_nfc",
                "loans_hh",
                "real_rate",
                "slope",
                "spreads"
            ]
        );
        let rows = arrange_rows(&[], &[], &[]);
        assert_eq!(ids(&rows), default_rows());
        // Affichées par défaut : les 9 lignes de la v0.2 ; les 5 nouvelles sont masquées.
        let visible: Vec<_> = rows.iter().filter(|r| !r.hidden).map(|r| r.id).collect();
        assert_eq!(
            visible,
            [
                "deposit_rate",
                "hicp",
                "estr",
                "yield_10y",
                "eurusd",
                "unemployment",
                "real_rate",
                "slope",
                "spreads"
            ]
        );
        assert!(!rows[0].derived && rows[13].derived); // taux de dépôt : BCE ; écarts : calculés
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
                "hicp",
                "deposit_rate",
                "core_inflation", // absentes du choix : en fin de section, ordre par défaut
                "services_inflation",
                "estr",
                "yield_10y",
                "eurusd",
                "unemployment",
                "m3",
                "loans_nfc",
                "loans_hh",
                "spreads", // les calculés restent après les séries BCE
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
        assert_eq!(rows.iter().filter(|row| !row.hidden).count(), 9);
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
        assert_eq!(rows[11].id, "spreads");
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
    fn pays_des_ecarts_de_taux() {
        assert_eq!(arrange_spreads(&[]), DEFAULT_SPREADS);
        // Ordre d'affichage, sans doublon ni inconnu, 3 au plus.
        let chosen = strings(&[
            "spread_nl",
            "spread_gr",
            "spread_gr",
            "spread_xx",
            "spread_fr",
        ]);
        assert_eq!(
            arrange_spreads(&chosen),
            ["spread_fr", "spread_gr", "spread_nl"]
        );
        assert_eq!(all_spreads().count(), 7);

        let dir = temp_dir("pays");
        let store = Store::load(dir.clone(), Lang::Fr);
        assert_eq!(store.spreads(), DEFAULT_SPREADS);
        store
            .set_spreads(&strings(&["spread_gr", "spread_pt"]))
            .unwrap();
        assert_eq!(
            Store::load(dir.clone(), Lang::Fr).spreads(),
            ["spread_gr", "spread_pt"]
        );
        assert!(store.set_spreads(&[]).is_err());
        assert!(store
            .set_spreads(&strings(&[
                "spread_fr",
                "spread_it",
                "spread_es",
                "spread_gr"
            ]))
            .is_err());
        store.reset_layout().unwrap();
        assert_eq!(store.spreads(), DEFAULT_SPREADS);
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
