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
//
// Clic sur une notification : un « délégué » (objet que macOS prévient) lit l'indicateur
// glissé dans l'identifiant de la notification (« glaux-hicp-3 ») et ouvre le panneau dessus.

use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

use crate::i18n::Lang;

use block2::{DynBlock, RcBlock};
use objc2::rc::Retained;
use objc2::runtime::{Bool, NSObject, NSObjectProtocol, ProtocolObject};
use objc2::{define_class, msg_send, AllocAnyThread};
use objc2_foundation::{NSBundle, NSError, NSString};
use objc2_user_notifications::{
    UNAuthorizationOptions, UNMutableNotificationContent, UNNotificationRequest,
    UNNotificationResponse, UNNotificationSound, UNUserNotificationCenter,
    UNUserNotificationCenterDelegate,
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

/// Indicateurs qu'un clic sur une notification peut ouvrir.
const CLICKABLE: [&str; 2] = ["deposit_rate", "hicp"];

/// Identifiant d'une notification : « glaux-{indicateur}-{numéro} » (« glaux-info-4 » sans
/// indicateur). Unique pour macOS, et porteur de l'indicateur à ouvrir au clic.
fn notification_id(indicator: Option<&str>, number: u64) -> String {
    format!("glaux-{}-{number}", indicator.unwrap_or("info"))
}

/// Indicateur à ouvrir d'après l'identifiant d'une notification (None : panneau seul).
fn indicator_of(id: &str) -> Option<&'static str> {
    let (indicator, _) = id.strip_prefix("glaux-")?.rsplit_once('-')?;
    CLICKABLE.iter().copied().find(|c| *c == indicator)
}

/// Action au clic sur une notification (indicateur à ouvrir, ou None).
type ClickHandler = Box<dyn Fn(Option<&'static str>) + Send + Sync>;
static ON_CLICK: OnceLock<ClickHandler> = OnceLock::new();

define_class!(
    // SAFETY : NSObject n'impose rien à ses sous-classes ; pas de Drop.
    #[unsafe(super(NSObject))]
    #[name = "GlauxNotificationDelegate"]
    struct NotificationDelegate;

    unsafe impl NSObjectProtocol for NotificationDelegate {}

    unsafe impl UNUserNotificationCenterDelegate for NotificationDelegate {
        /// Clic sur une notification de Glaux (ou sur « Afficher ») : ouvre le panneau.
        #[unsafe(method(userNotificationCenter:didReceiveNotificationResponse:withCompletionHandler:))]
        fn did_receive(
            &self,
            _center: &UNUserNotificationCenter,
            response: &UNNotificationResponse,
            completion: &DynBlock<dyn Fn()>,
        ) {
            let id = response.notification().request().identifier().to_string();
            if let Some(on_click) = ON_CLICK.get() {
                on_click(indicator_of(&id));
            }
            completion.call(()); // macOS attend cet appel une fois la réponse traitée
        }
    }
);

impl NotificationDelegate {
    fn new() -> Retained<Self> {
        let this = Self::alloc().set_ivars(());
        // SAFETY : `init` de NSObject, sur un objet tout juste alloué.
        unsafe { msg_send![super(this), init] }
    }
}

/// Enregistre l'action à faire au clic sur une notification (à appeler au lancement, avant
/// toute notification : un clic peut aussi relancer Glaux).
pub fn on_click(handler: impl Fn(Option<&'static str>) + Send + Sync + 'static) {
    if !running_from_app_bundle() || ON_CLICK.set(Box::new(handler)).is_err() {
        return;
    }
    let delegate = NotificationDelegate::new();
    UNUserNotificationCenter::currentNotificationCenter()
        .setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
    // macOS ne garde qu'une référence faible au délégué : on le garde en vie pour toujours.
    std::mem::forget(delegate);
}

/// Affiche une notification (sans attendre : macOS la traite en arrière-plan).
/// `indicator` : ouvert dans le panneau si l'on clique dessus.
pub fn show(title: &str, body: &str, indicator: Option<&str>) {
    if !running_from_app_bundle() {
        show_with_osascript(title, body);
        return;
    }

    let content = UNMutableNotificationContent::new();
    content.setTitle(&NSString::from_str(title));
    content.setBody(&NSString::from_str(body));
    content.setSound(Some(&UNNotificationSound::defaultSound()));

    let id = notification_id(indicator, COUNTER.fetch_add(1, Ordering::Relaxed));
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

/// Mode développement : notification via AppleScript (pas d'action au clic).
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
    use super::{applescript_string, indicator_of, notification_id};

    #[test]
    fn identifiant_porte_l_indicateur() {
        assert_eq!(notification_id(Some("hicp"), 3), "glaux-hicp-3");
        assert_eq!(indicator_of("glaux-hicp-3"), Some("hicp"));
        assert_eq!(
            indicator_of(&notification_id(Some("deposit_rate"), 12)),
            Some("deposit_rate")
        );
        assert_eq!(indicator_of(&notification_id(None, 4)), None); // « glaux-info-4 »
        assert_eq!(indicator_of("glaux-7"), None); // ancien format
        assert_eq!(indicator_of("autre-hicp-1"), None);
        assert_eq!(indicator_of("glaux-eurusd-1"), None); // pas ouvrable au clic
    }

    #[test]
    fn echappement_applescript() {
        assert_eq!(applescript_string("Taux : 2,75 %"), "\"Taux : 2,75 %\"");
        assert_eq!(applescript_string("dit \"oui\""), "\"dit \\\"oui\\\"\"");
        assert_eq!(applescript_string("a\\b"), "\"a\\\\b\"");
    }
}
