// Mises à jour automatiques (module « updater » de Tauri).
//
// - Vérification au lancement puis une fois par jour : une seule requête vers GitHub
//   (fichier latest.json de la dernière version publiée). Aucune donnée personnelle envoyée.
// - Une nouvelle version s'affiche en bas du panneau (« Installer la v0.2.0 ») :
//   rien ne s'installe sans un clic.
// - Le fichier téléchargé est vérifié avec la clé publique de tauri.conf.json : seule une
//   version signée avec la clé privée de Glaux (secret GitHub) peut s'installer.
// - Pas de vérification en mode développement (`npm run tauri dev`).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use tauri::{AppHandle, Manager};
use tauri_plugin_updater::UpdaterExt;

/// Intervalle entre deux vérifications.
const CHECK_EVERY: Duration = Duration::from_secs(24 * 60 * 60);

/// État des mises à jour, partagé entre le fil d'arrière-plan et les commandes.
#[derive(Default)]
pub struct UpdateState {
    /// Version disponible (None : Glaux est à jour, ou pas encore vérifié).
    available: Mutex<Option<String>>,
    /// Téléchargement / installation en cours.
    installing: AtomicBool,
    /// Échec de la dernière installation (affiché au survol du bouton).
    error: Mutex<Option<String>>,
    /// Dernière vérification.
    last_check: Mutex<Option<Instant>>,
}

impl UpdateState {
    /// Vrai si la dernière vérification date de plus de 24 h (ou n'a jamais eu lieu).
    pub fn check_due(&self) -> bool {
        if cfg!(debug_assertions) {
            return false; // mode développement : pas de mise à jour
        }
        self.last_check
            .lock()
            .map(|last| last.is_none_or(|t| t.elapsed() >= CHECK_EVERY))
            .unwrap_or(false)
    }

    pub fn available(&self) -> Option<String> {
        self.available.lock().ok().and_then(|v| v.clone())
    }

    pub fn installing(&self) -> bool {
        self.installing.load(Ordering::SeqCst)
    }

    pub fn error(&self) -> Option<String> {
        self.error.lock().ok().and_then(|e| e.clone())
    }
}

/// Cherche une nouvelle version (bloquant : à appeler depuis le fil d'arrière-plan).
/// Hors ligne, ou tant qu'aucune version n'est publiée : on garde l'état précédent.
pub fn check(app: &AppHandle) {
    let state = app.state::<UpdateState>();
    if let Ok(mut last) = state.last_check.lock() {
        *last = Some(Instant::now());
    }
    let result = tauri::async_runtime::block_on(async { app.updater()?.check().await });
    match result {
        Ok(update) => {
            if let Ok(mut available) = state.available.lock() {
                *available = update.map(|u| u.version);
            }
        }
        Err(e) => eprintln!("Vérification des mises à jour impossible : {e}"),
    }
}

/// Réserve l'installation (faux si elle est déjà en cours : double clic).
pub fn begin_install(app: &AppHandle) -> bool {
    let state = app.state::<UpdateState>();
    if state.installing.swap(true, Ordering::SeqCst) {
        return false;
    }
    if let Ok(mut error) = state.error.lock() {
        *error = None;
    }
    true
}

/// Télécharge, vérifie la signature, installe, puis relance Glaux.
/// (Windows : l'installeur ferme et relance l'app lui-même.)
/// À appeler après `begin_install`.
pub async fn install(app: &AppHandle) -> Result<(), String> {
    let result: Result<bool, String> = async {
        let update = app
            .updater()
            .map_err(|e| e.to_string())?
            .check()
            .await
            .map_err(|e| e.to_string())?;
        let Some(update) = update else {
            return Ok(false); // déjà à jour
        };
        update
            .download_and_install(|_, _| {}, || {})
            .await
            .map_err(|e| e.to_string())?;
        Ok(true)
    }
    .await;

    let state = app.state::<UpdateState>();
    match result {
        Ok(true) => app.restart(),
        Ok(false) => {
            if let Ok(mut available) = state.available.lock() {
                *available = None;
            }
            state.installing.store(false, Ordering::SeqCst);
            Ok(())
        }
        Err(e) => {
            if let Ok(mut error) = state.error.lock() {
                *error = Some(e.clone());
            }
            state.installing.store(false, Ordering::SeqCst);
            Err(e)
        }
    }
}

#[cfg(test)]
mod tests {
    /// La configuration du module (tauri.conf.json › plugins › updater) doit être lisible :
    /// sinon Glaux ne démarrerait pas.
    #[test]
    fn configuration_des_mises_a_jour_valide() {
        let conf: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        let updater = conf["plugins"]["updater"].clone();
        let config: tauri_plugin_updater::Config = serde_json::from_value(updater).unwrap();
        assert!(config.require_signed_version);
        assert!(config.endpoints[0]
            .as_str()
            .ends_with("/releases/latest/download/latest.json"));
    }
}
