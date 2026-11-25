// Glaux — cœur de l'application (Rust).
// - Icône dans la barre des menus (Mac) / zone de notification (Windows)
//   qui ouvre et ferme un panneau compact.
// - Données de la BCE : téléchargées en arrière-plan, gardées en cache local.

// En version finale sur Windows, évite l'ouverture d'une console noire à côté de l'app.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod agenda;
mod alerts;
mod data;
mod ecb;
mod i18n;
#[cfg(target_os = "macos")]
mod macos_notify;
mod store;
mod updates;

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use alerts::Alert;
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, Runtime, State, WebviewWindow,
    WindowEvent,
};
// Module de notifications de Tauri : Windows et Linux seulement (Mac : macos_notify).
#[cfg(not(target_os = "macos"))]
use tauri_plugin_notification::NotificationExt;

use data::Observation;
use i18n::Lang;
use store::{DashboardView, Store};
use updates::UpdateState;

/// Nom (label) de la fenêtre déclarée dans tauri.conf.json.
const PANEL: &str = "main";

/// Identifiant de l'icône de la barre des menus / zone de notification.
const TRAY_ID: &str = "glaux-tray";

/// Délai pendant lequel on ignore un clic sur l'icône juste après la fermeture
/// du panneau. Sans ça : cliquer sur l'icône fait perdre le focus au panneau
/// (qui se masque), puis le même clic le rouvre aussitôt.
const REOPEN_GUARD: Duration = Duration::from_millis(250);

/// Espace entre l'icône et le panneau, et marge avec le bord de l'écran (en points).
const GAP: f64 = 6.0;

/// Fréquence à laquelle le fil d'arrière-plan vérifie si les données sont trop vieilles.
const CHECK_INTERVAL: Duration = Duration::from_secs(15 * 60);

/// Nom de l'événement envoyé à l'interface quand les données changent.
const DASHBOARD_EVENT: &str = "dashboard-updated";

/// État partagé de l'app.
struct PanelState {
    /// Moment où le panneau a été masqué pour la dernière fois.
    last_hidden: Mutex<Option<Instant>>,
    /// Position et taille de l'icône à l'écran (en pixels physiques),
    /// mises à jour à chaque survol ou clic sur l'icône.
    tray_rect: Mutex<Option<(PhysicalPosition<f64>, PhysicalSize<f64>)>>,
}

/// Mémorise où se trouve l'icône, pour placer le panneau juste à côté.
fn remember_tray_rect(app: &AppHandle, event: &TrayIconEvent) {
    let rect = match event {
        TrayIconEvent::Click { rect, .. }
        | TrayIconEvent::Enter { rect, .. }
        | TrayIconEvent::Move { rect, .. }
        | TrayIconEvent::Leave { rect, .. } => rect,
        _ => return,
    };
    // Le système fournit déjà des pixels physiques : facteur d'échelle 1.
    let position = rect.position.to_physical::<f64>(1.0);
    let size = rect.size.to_physical::<f64>(1.0);
    if let Ok(mut saved) = app.state::<PanelState>().tray_rect.lock() {
        *saved = Some((position, size));
    }
}

/// Place le panneau contre l'icône :
/// - icône en haut de l'écran (barre des menus Mac) → panneau en dessous ;
/// - icône en bas (barre des tâches Windows) → panneau au-dessus.
fn place_panel(app: &AppHandle, window: &WebviewWindow) {
    let tray = app
        .state::<PanelState>()
        .tray_rect
        .lock()
        .ok()
        .and_then(|saved| *saved);
    // Position de l'icône encore inconnue : le panneau garde sa position actuelle.
    let Some((tray_pos, tray_size)) = tray else {
        return;
    };
    let Ok(panel) = window.outer_size() else {
        return;
    };
    let Ok(Some(monitor)) = window.monitor_from_point(tray_pos.x, tray_pos.y) else {
        return;
    };

    let scale = monitor.scale_factor();
    let gap = GAP * scale;
    let (panel_w, panel_h) = (panel.width as f64, panel.height as f64);
    let screen_x = monitor.position().x as f64;
    let screen_y = monitor.position().y as f64;
    let screen_w = monitor.size().width as f64;
    let screen_h = monitor.size().height as f64;

    // Horizontal : centré sous l'icône, sans dépasser du bord de l'écran.
    let mut x = tray_pos.x + tray_size.width / 2.0 - panel_w / 2.0;
    let (min_x, max_x) = (screen_x + gap, screen_x + screen_w - panel_w - gap);
    if min_x <= max_x {
        x = x.clamp(min_x, max_x);
    }

    // Vertical : selon que l'icône est dans la moitié haute ou basse de l'écran.
    let icon_on_top = tray_pos.y < screen_y + screen_h / 2.0;
    let y = if icon_on_top {
        tray_pos.y + tray_size.height + gap
    } else {
        tray_pos.y - panel_h - gap
    };

    let _ = window.set_position(PhysicalPosition::new(x.round() as i32, y.round() as i32));
}

/// Masque le panneau et note l'heure.
fn hide_panel(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(PANEL) {
        let _ = window.hide();
        if let Ok(mut last) = app.state::<PanelState>().last_hidden.lock() {
            *last = Some(Instant::now());
        }
    }
}

/// Place le panneau contre l'icône puis l'affiche au premier plan.
fn show_panel(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(PANEL) {
        place_panel(app, &window);
        let _ = window.show();
        let _ = window.set_focus();
    }
}

/// Clic sur l'icône : ouvre le panneau s'il est fermé, le ferme sinon.
fn toggle_panel(app: &AppHandle) {
    let Some(window) = app.get_webview_window(PANEL) else {
        return;
    };

    if window.is_visible().unwrap_or(false) {
        hide_panel(app);
        return;
    }

    let just_hidden = app
        .state::<PanelState>()
        .last_hidden
        .lock()
        .ok()
        .and_then(|last| *last)
        .map(|t| t.elapsed() < REOPEN_GUARD)
        .unwrap_or(false);

    if !just_hidden {
        show_panel(app);
    }
}

/// Données actuelles du tableau de bord (appelée par l'interface au démarrage).
#[tauri::command]
fn get_dashboard(app: AppHandle) -> DashboardView {
    dashboard_view(&app)
}

/// Vue complète : données + version et mises à jour + éventuel refus des notifications (Mac).
fn dashboard_view(app: &AppHandle) -> DashboardView {
    let mut view = app.state::<Store>().view(chrono::Local::now().date_naive());
    view.version = app.package_info().version.to_string();
    let updates = app.state::<UpdateState>();
    view.update = updates.available();
    view.updating = updates.installing();
    view.update_error = updates.error();
    #[cfg(target_os = "macos")]
    {
        view.notification_error = macos_notify::last_error().map(|e| e.message(view.language));
    }
    view
}

/// Historique d'un indicateur pour la vue détail (`months` : 12 ou 60).
#[tauri::command]
fn get_history(
    store: State<'_, Store>,
    id: String,
    months: u32,
) -> Result<Vec<Observation>, String> {
    store.history(&id, months, chrono::Local::now().date_naive())
}

/// Enregistre l'image exportée (PNG) dans Images/Glaux, puis la montre dans le Finder
/// (ou l'Explorateur sous Windows). Renvoie le chemin du fichier.
///
/// L'image arrive en binaire brut (pas de JSON) ; l'indicateur dans l'en-tête
/// « x-indicator ». Le nom du fichier est construit ici, jamais par l'interface.
#[tauri::command]
fn save_png(app: AppHandle, request: tauri::ipc::Request<'_>) -> Result<String, String> {
    let tauri::ipc::InvokeBody::Raw(bytes) = request.body() else {
        return Err("image manquante".into());
    };
    // Vérification minimale : un PNG commence toujours par ces 8 octets.
    if !bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
        return Err("le fichier reçu n'est pas une image PNG".into());
    }

    let id = request
        .headers()
        .get("x-indicator")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    let name = store::export_name(id).ok_or_else(|| "indicateur inconnu".to_string())?;

    let dir = app
        .path()
        .picture_dir()
        .map_err(|e| format!("dossier Images introuvable : {e}"))?
        .join("Glaux");
    std::fs::create_dir_all(&dir).map_err(|e| format!("création du dossier impossible : {e}"))?;

    let stamp = chrono::Local::now().format("%Y-%m-%d_%H%M%S");
    let path: PathBuf = dir.join(format!("Glaux_{name}_{stamp}.png"));
    std::fs::write(&path, bytes).map_err(|e| format!("enregistrement impossible : {e}"))?;

    reveal_in_file_manager(&path);
    Ok(path.to_string_lossy().into_owned())
}

/// Ouvre le dossier du fichier et le sélectionne (sans attendre ; échec silencieux).
fn reveal_in_file_manager(path: &Path) {
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open")
        .arg("-R")
        .arg(path)
        .spawn();
    #[cfg(target_os = "windows")]
    let _ = std::process::Command::new("explorer")
        .arg(format!("/select,{}", path.display()))
        .spawn();
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let _ = path;
}

/// Bouton « Actualiser » : lance un téléchargement en arrière-plan.
/// Le résultat arrive dans l'interface par l'événement « dashboard-updated ».
#[tauri::command]
fn refresh_now(app: AppHandle) {
    std::thread::spawn(move || refresh_and_notify(&app));
}

/// Délai avant la notification de confirmation : le temps que macOS rende la main
/// à l'app précédente (voir `set_notifications`).
const CONFIRMATION_DELAY: Duration = Duration::from_millis(900);

/// Active ou désactive les notifications. À l'activation, une notification de
/// confirmation montre tout de suite que ça marche.
///
/// macOS n'affiche pas de bannière pour l'app active : il la range directement dans
/// le Centre de notifications. Comme le clic sur « Alertes » rend Glaux active, on
/// ferme d'abord le panneau et on rend la main à l'app précédente, puis on envoie
/// la confirmation un instant plus tard. (Les vraies alertes arrivent en général
/// quand on est dans une autre app : elles s'affichent normalement.)
#[tauri::command]
fn set_notifications(app: AppHandle, enabled: bool) -> Result<(), String> {
    let store = app.state::<Store>();
    store.set_notifications(enabled)?;
    let lang = store.lang();
    notify(&app);
    if enabled {
        hide_panel(&app);
        #[cfg(target_os = "macos")]
        let _ = app.hide();
        std::thread::spawn(move || {
            std::thread::sleep(CONFIRMATION_DELAY);
            send_alerts(
                &app,
                &[Alert {
                    title: lang
                        .pick("Alertes Glaux activées", "Glaux alerts are on")
                        .into(),
                    body: lang
                        .pick(
                            "Décisions de la BCE, nouvelles inflations et rappels le jour J.",
                            "ECB decisions, new inflation figures and same-day reminders.",
                        )
                        .into(),
                }],
            );
            // Le temps que macOS réponde (autorisation, envoi), puis on met le panneau à jour :
            // un refus s'y affiche (« Alertes : bloquées »).
            std::thread::sleep(Duration::from_secs(3));
            notify(&app);
        });
    }
    Ok(())
}

/// Installe la nouvelle version de Glaux, puis relance l'app.
/// Le panneau affiche « Mise à jour… » pendant le téléchargement ; en cas d'échec,
/// le bouton redevient cliquable avec la raison au survol.
#[tauri::command]
async fn install_update(app: AppHandle) -> Result<(), String> {
    if !updates::begin_install(&app) {
        return Ok(()); // déjà en cours
    }
    notify(&app);
    let result = updates::install(&app).await;
    notify(&app);
    result
}

/// Change la langue de Glaux : interface (renvoyée à l'écran), notifications, menu de l'icône.
#[tauri::command]
fn set_language(app: AppHandle, language: Lang) -> Result<(), String> {
    app.state::<Store>().set_language(language)?;
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let menu = tray_menu(&app, language).map_err(|e| e.to_string())?;
        tray.set_menu(Some(menu)).map_err(|e| e.to_string())?;
    }
    notify(&app);
    Ok(())
}

/// Menu du clic droit sur l'icône, dans la langue de Glaux.
fn tray_menu<R: Runtime, M: Manager<R>>(manager: &M, lang: Lang) -> tauri::Result<Menu<R>> {
    let open = MenuItem::with_id(
        manager,
        "open",
        lang.pick("Ouvrir Glaux", "Open Glaux"),
        true,
        None::<&str>,
    )?;
    let quit = MenuItem::with_id(
        manager,
        "quit",
        lang.pick("Quitter Glaux", "Quit Glaux"),
        true,
        None::<&str>,
    )?;
    Menu::with_items(manager, &[&open, &quit])
}

/// Affiche des notifications système, si l'utilisateur les a activées dans Glaux.
fn send_alerts(app: &AppHandle, alerts: &[Alert]) {
    if alerts.is_empty() || !app.state::<Store>().notifications_enabled() {
        return;
    }
    for alert in alerts {
        // Mac : système natif d'Apple (le module Tauri n'y fonctionne plus).
        #[cfg(target_os = "macos")]
        macos_notify::show(&alert.title, &alert.body);

        // Windows et Linux : module de notifications de Tauri.
        #[cfg(not(target_os = "macos"))]
        if let Err(e) = app
            .notification()
            .builder()
            .title(&alert.title)
            .body(&alert.body)
            .show()
        {
            eprintln!("Notification impossible : {e}");
        }
    }
}

/// Rappels du jour J (décision BCE, inflation), envoyés une seule fois.
fn send_due_reminders(app: &AppHandle) {
    let now = chrono::Local::now();
    let reminders = app
        .state::<Store>()
        .take_due_reminders(now.date_naive(), now.time());
    send_alerts(app, &reminders);
}

/// Envoie l'état actuel des données à l'interface.
fn notify(app: &AppHandle) {
    let view = dashboard_view(app);
    let _ = app.emit(DASHBOARD_EVENT, view);
}

/// Télécharge les données (bloquant) en prévenant l'interface au début et à la fin.
fn refresh_and_notify(app: &AppHandle) {
    let store = app.state::<Store>();
    // Une actualisation est déjà en cours : on ne la double pas.
    let Some(guard) = store.try_begin_refresh() else {
        return;
    };
    notify(app); // l'interface affiche « Actualisation… »
    let alerts = store.download_all();
    drop(guard);
    notify(app); // nouvelles valeurs (ou message d'erreur)
    send_alerts(app, &alerts); // taux BCE modifié, nouvelle inflation
}

/// Quitte l'app. Appelée par l'interface : bouton « Quitter » du panneau ou ⌘Q.
/// Sert de solution de secours au menu du clic droit, que macOS n'ouvre pas
/// avec un Ctrl + clic ni si le clic secondaire du trackpad est désactivé.
#[tauri::command]
fn quit_app(app: AppHandle) {
    app.exit(0);
}

fn main() {
    let builder = tauri::Builder::default();
    // Notifications Windows et Linux : module de Tauri (sur Mac : src/macos_notify.rs).
    #[cfg(not(target_os = "macos"))]
    let builder = builder.plugin(tauri_plugin_notification::init());

    builder
        // Mises à jour automatiques (src/updates.rs ; adresse et clé publique : tauri.conf.json).
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(UpdateState::default())
        // Commandes Rust que l'interface peut appeler via invoke().
        .invoke_handler(tauri::generate_handler![
            get_dashboard,
            get_history,
            refresh_now,
            save_png,
            set_notifications,
            set_language,
            install_update,
            quit_app
        ])
        .manage(PanelState {
            last_hidden: Mutex::new(None),
            tray_rect: Mutex::new(None),
        })
        .setup(|app| {
            // Mac : pas d'icône dans le Dock ni dans ⌘+Tab (en plus de Info.plist).
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            // Cache local des données, dans le dossier de données de l'app.
            let data_dir = app.path().app_data_dir()?;
            app.manage(Store::load(data_dir, Lang::system()));

            // Mac : demande l'autorisation dès le lancement si les alertes sont activées
            // (macOS affiche « Glaux souhaite vous envoyer des notifications » la première fois).
            #[cfg(target_os = "macos")]
            if app.state::<Store>().notifications_enabled() {
                macos_notify::request_authorization();
            }

            // Fil d'arrière-plan : télécharge au démarrage si besoin, puis vérifie
            // toutes les 15 min si les données ont plus de 6 h.
            let handle = app.handle().clone();
            std::thread::Builder::new()
                .name("glaux-actualisation".into())
                .spawn(move || loop {
                    if handle.state::<Store>().needs_refresh() {
                        refresh_and_notify(&handle);
                    }
                    send_due_reminders(&handle);
                    // Nouvelle version de Glaux ? (au lancement, puis une fois par jour)
                    if handle.state::<UpdateState>().check_due() {
                        updates::check(&handle);
                        notify(&handle);
                    }
                    std::thread::sleep(CHECK_INTERVAL);
                })?;

            // Menu du clic droit sur l'icône.
            let menu = tray_menu(app, app.state::<Store>().lang())?;

            let tray = TrayIconBuilder::with_id(TRAY_ID)
                .tooltip("Glaux")
                .menu(&menu)
                // Clic gauche = panneau ; clic droit = menu.
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id().as_ref() {
                    "open" => show_panel(app),
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    remember_tray_rect(tray.app_handle(), &event);

                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        toggle_panel(tray.app_handle());
                    }
                });

            // Mac : icône monochrome dédiée, en mode « template » pour que macOS
            // l'affiche en noir ou en blanc selon le thème de la barre des menus.
            #[cfg(target_os = "macos")]
            let tray = tray
                .icon(tauri::image::Image::from_bytes(include_bytes!(
                    "../icons/tray.png"
                ))?)
                .icon_as_template(true);

            // Windows : icône couleur de l'app (une icône noire serait invisible
            // sur une barre des tâches sombre).
            #[cfg(not(target_os = "macos"))]
            let tray = match app.default_window_icon() {
                Some(icon) => tray.icon(icon.clone()),
                None => tray,
            };

            tray.build(app)?;
            Ok(())
        })
        .on_window_event(|window, event| {
            // Comme un menu macOS : le panneau se ferme dès qu'on clique ailleurs.
            if window.label() == PANEL {
                if let WindowEvent::Focused(false) = event {
                    hide_panel(window.app_handle());
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("Glaux n'a pas pu démarrer");
}
