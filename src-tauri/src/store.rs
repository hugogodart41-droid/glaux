// Mémoire locale des données : chargée au démarrage, mise à jour depuis la BCE,
// enregistrée sur le disque pour un affichage immédiat et hors ligne.
// Emplacement : dossier de données de l'app
//   Mac     : ~/Library/Application Support/com.howzen.glaux/cache.json
//   Windows : %APPDATA%\com.howzen.glaux\cache.json
// Les préférences (alertes activées, rappels déjà envoyés, langue) sont à côté, dans prefs.json.

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
    /// Dernière actualisation où les 6 séries ont été téléchargées (secondes Unix).
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
}

impl Default for Prefs {
    fn default() -> Self {
        Self {
            notifications: true,
            reminded: Vec::new(),
            language: None,
        }
    }
}

/// Nombre de rappels déjà envoyés gardés en mémoire (largement plus d'un an).
const MAX_REMINDED: usize = 60;

/// Indicateurs calculés par Glaux à partir des séries BCE (après les 6 séries, dans cet ordre).
/// - « real_rate » : taux réel = taux de dépôt − inflation (mensuel) ;
/// - « slope » : pente des taux = taux souverain 10 ans − €STR (quotidien).
const DERIVED: [(&str, Frequency); 2] = [
    ("real_rate", Frequency::Monthly),
    ("slope", Frequency::Daily),
];

/// Un indicateur tel qu'envoyé à l'interface.
#[derive(Clone, Serialize)]
pub struct IndicatorView {
    pub id: &'static str,
    /// Vrai pour un indicateur calculé par Glaux (affiché dans une section à part).
    pub derived: bool,
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

    /// Vrai si les données ont plus de 6 h, ou s'il n'y en a jamais eu.
    pub fn needs_refresh(&self) -> bool {
        let last = self.cache.lock().ok().and_then(|c| c.last_success);
        match last {
            None => true,
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

    /// Télécharge les 6 séries (bloquant : à appeler hors du fil principal).
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

    /// Construit la vue envoyée à l'interface : les 6 séries BCE, puis les indicateurs calculés.
    pub fn view(&self, today: NaiveDate) -> DashboardView {
        let cache = self.cache.lock().ok();
        let empty = CacheFile::default();
        let data = cache.as_deref().unwrap_or(&empty);

        let ids = SERIES
            .iter()
            .map(|def| (def.id, false))
            .chain(DERIVED.iter().map(|(id, _)| (*id, true)));
        let indicators = ids
            .filter_map(|(id, derived)| {
                let (observations, frequency, step) = series_data(data, id)?;
                Some(IndicatorView {
                    id,
                    derived,
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
