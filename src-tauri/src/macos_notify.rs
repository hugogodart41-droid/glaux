// Notifications sur Mac avec le système actuel d'Apple (UNUserNotificationCenter).
//
// Pourquoi pas le module de notifications de Tauri sur Mac ? Il passe par
// NSUserNotificationCenter, déprécié depuis macOS 11 : sur les macOS récents, ses
// notifications n'arrivent plus (testé le 24/11/2026). Sous Windows, le module Tauri
// reste utilisé.
//
// UNUserNotificationCenter ne fonctionne que dans une app empaquetée (Glaux.app).
// En mode développement (`npm run tauri dev`, binaire hors .app), on se rabat sur
// `osascript` : la notification s'affiche au nom de « Éditeur de script ».

use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use crate::i18n::Lang;

use block2::RcBlock;
use objc2::runtime::Bool;
use objc2_foundation::{NSBundle, NSError, NSString};
use objc2_user_notifications::{
    UNAuthorizationOptions, UNMutableNotificationContent, UNNotificationRequest,
    UNNotificationSound, UNUserNotificationCenter,
};

/// Numéro des notifications (identifiant unique pour macOS).
static COUNTER: AtomicU64 = AtomicU64::new(0);

/// Refus ou erreur de macOS, affiché dans le panneau (« Alertes : bloquées », raison au survol).
#[derive(Clone, Debug)]
pub enum NotifyError {
    /// Notifications refusées pour Glaux (Réglages Système › Notifications).
    Denied,
    /// Erreur de macOS pendant la demande d'autorisation (texte d'Apple).
    Authorization(String),
    /// Erreur de macOS à l'envoi (texte d'Apple).
    NotSent(String),
    /// Mode développement : osascript n'a pas pu être lancé.
    Osascript(String),
}

impl NotifyError {
    /// Texte affiché au survol de « Alertes : bloquées ».
    pub fn message(&self, lang: Lang) -> String {
        match self {
            NotifyError::Denied => lang
                .pick(
                    "Notifications refusées par macOS : à autoriser dans Réglages Système › Notifications › Glaux.",
                    "Notifications blocked by macOS: allow them in System Settings › Notifications › Glaux.",
                )
                .into(),
            NotifyError::Authorization(m) => lang.pick(
                format!("Autorisation impossible : {m}"),
                format!("Permission request failed: {m}"),
            ),
            NotifyError::NotSent(m) => lang.pick(
                format!("Notification non envoyée : {m}"),
                format!("Notification not sent: {m}"),
            ),
            NotifyError::Osascript(m) => lang.pick(
                format!("Notification impossible (osascript) : {m}"),
                format!("Notification failed (osascript): {m}"),
            ),
        }
    }
}

/// Dernier refus ou dernière erreur de macOS (None : tout va bien).
static LAST_ERROR: Mutex<Option<NotifyError>> = Mutex::new(None);

fn record(error: Option<NotifyError>) {
    if let Ok(mut last) = LAST_ERROR.lock() {
        *last = error;
    }
}

/// Dernier refus ou dernière erreur de macOS (None si tout va bien).
pub fn last_error() -> Option<NotifyError> {
    LAST_ERROR.lock().ok().and_then(|e| e.clone())
}

/// Vrai si Glaux tourne depuis son paquet .app (seul cas où UNUserNotificationCenter est utilisable :
/// hors paquet, macOS lève une exception qui ferme l'app).
fn running_from_app_bundle() -> bool {
    let bundle = NSBundle::mainBundle();
    bundle.bundleIdentifier().is_some() && bundle.bundlePath().to_string().ends_with(".app")
}

/// Texte d'une erreur Apple (pointeur éventuellement nul).
fn describe(error: *mut NSError) -> Option<String> {
    // SAFETY : macOS passe soit un pointeur nul, soit une NSError valide le temps de l'appel.
    unsafe { error.as_ref() }.map(|e| e.localizedDescription().to_string())
}

/// Demande l'autorisation d'envoyer des notifications. La première fois, macOS affiche
/// « Glaux souhaite vous envoyer des notifications » ; ensuite, la réponse est mémorisée
/// (modifiable dans Réglages Système › Notifications › Glaux).
pub fn request_authorization() {
    if !running_from_app_bundle() {
        return;
    }
    let center = UNUserNotificationCenter::currentNotificationCenter();
    let handler = RcBlock::new(|granted: Bool, error: *mut NSError| {
        if let Some(message) = describe(error) {
            record(Some(NotifyError::Authorization(message)));
        } else if !granted.as_bool() {
            record(Some(NotifyError::Denied));
        } else {
            record(None);
        }
    });
    center.requestAuthorizationWithOptions_completionHandler(
        UNAuthorizationOptions::Alert | UNAuthorizationOptions::Sound,
        &handler,
    );
}

/// Affiche une notification (sans attendre : macOS la traite en arrière-plan).
pub fn show(title: &str, body: &str) {
    if !running_from_app_bundle() {
        show_with_osascript(title, body);
        return;
    }

    let content = UNMutableNotificationContent::new();
    content.setTitle(&NSString::from_str(title));
    content.setBody(&NSString::from_str(body));
    content.setSound(Some(&UNNotificationSound::defaultSound()));

    let id = format!("glaux-{}", COUNTER.fetch_add(1, Ordering::Relaxed));
    let request = UNNotificationRequest::requestWithIdentifier_content_trigger(
        &NSString::from_str(&id),
        &content,
        None, // pas de déclencheur : affichage immédiat
    );

    let center = UNUserNotificationCenter::currentNotificationCenter();
    // On (re)demande l'autorisation à chaque envoi : sans effet si elle est déjà donnée,
    // et c'est ce qui fait apparaître la question de macOS la toute première fois.
    let handler = RcBlock::new(move |granted: Bool, error: *mut NSError| {
        if let Some(message) = describe(error) {
            record(Some(NotifyError::Authorization(message)));
            return;
        }
        if !granted.as_bool() {
            record(Some(NotifyError::Denied)); // refusé par l'utilisateur : on respecte son choix
            return;
        }
        let on_added = RcBlock::new(|error: *mut NSError| {
            record(describe(error).map(NotifyError::NotSent));
        });
        UNUserNotificationCenter::currentNotificationCenter()
            .addNotificationRequest_withCompletionHandler(&request, Some(&on_added));
    });
    center.requestAuthorizationWithOptions_completionHandler(
        UNAuthorizationOptions::Alert | UNAuthorizationOptions::Sound,
        &handler,
    );
}

/// Échappe un texte pour une chaîne AppleScript entre guillemets.
fn applescript_string(text: &str) -> String {
    format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\""))
}

/// Mode développement : notification via AppleScript.
fn show_with_osascript(title: &str, body: &str) {
    let script = format!(
        "display notification {} with title {}",
        applescript_string(body),
        applescript_string(title)
    );
    let result = Command::new("osascript").arg("-e").arg(script).spawn();
    record(result.err().map(|e| NotifyError::Osascript(e.to_string())));
}

#[cfg(test)]
mod tests {
    use super::applescript_string;

    #[test]
    fn echappement_applescript() {
        assert_eq!(applescript_string("Taux : 2,75 %"), "\"Taux : 2,75 %\"");
        assert_eq!(applescript_string("dit \"oui\""), "\"dit \\\"oui\\\"\"");
        assert_eq!(applescript_string("a\\b"), "\"a\\\\b\"");
    }
}
