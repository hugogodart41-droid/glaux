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
mod excel;
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
    AppHandle, Emitter, LogicalSize, Manager, Monitor, PhysicalPosition, PhysicalSize, Runtime,
    State, WebviewWindow, WindowEvent,
};
use tauri_plugin_clipboard_manager::ClipboardExt;
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

/// Hauteur minimale du panneau (en points) : en-tête, une ligne et pied.
const MIN_PANEL_HEIGHT: f64 = 160.0;

/// Largeur minimale du panneau (en points) : celle du panneau normal.
const MIN_PANEL_WIDTH: f64 = 400.0;

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

/// Place le panneau contre l'icône, à sa taille actuelle.
fn place_panel(app: &AppHandle, window: &WebviewWindow) {
    if let Ok(size) = window.outer_size() {
        place_panel_sized(
            app,
            window,
            PhysicalSize::new(size.width as f64, size.height as f64),
        );
    }
}

/// Écran qui contient un point (en pixels physiques).
///
/// Pas `monitor_from_point` de Tauri : sur Mac, il compare ces pixels physiques à des points
/// (écran Retina = 2 pixels par point) et ne trouve aucun écran dès que l'icône est dans la
/// moitié droite de la barre des menus. Le panneau n'était alors jamais recalé sur l'écran.
fn monitor_at(window: &WebviewWindow, x: f64, y: f64) -> Option<Monitor> {
    let inside = |m: &Monitor| {
        let (p, s) = (m.position(), m.size());
        let (left, top) = (p.x as f64, p.y as f64);
        x >= left && x < left + s.width as f64 && y >= top && y < top + s.height as f64
    };
    window
        .available_monitors()
        .ok()
        .and_then(|monitors| monitors.into_iter().find(inside))
        .or_else(|| window.current_monitor().ok().flatten())
}

/// Position et taille de l'icône, si on l'a déjà vue passer (survol ou clic).
fn tray_rect(app: &AppHandle) -> Option<(PhysicalPosition<f64>, PhysicalSize<f64>)> {
    app.state::<PanelState>()
        .tray_rect
        .lock()
        .ok()
        .and_then(|saved| *saved)
}

/// Écran du panneau : celui de l'icône (ou, à défaut, celui où est la fenêtre).
fn panel_monitor(app: &AppHandle, window: &WebviewWindow) -> Option<Monitor> {
    match tray_rect(app) {
        Some((pos, size)) => {
            monitor_at(window, pos.x + size.width / 2.0, pos.y + size.height / 2.0)
        }
        None => window.current_monitor().ok().flatten(),
    }
}

/// Place le panneau contre l'icône, pour une taille donnée (en pixels physiques) :
/// - icône en haut de l'écran (barre des menus Mac) → panneau en dessous ;
/// - icône en bas (barre des tâches Windows) → panneau au-dessus ;
/// - toujours dans la zone utile de l'écran (sans barre des menus, Dock ni barre des tâches).
///
/// Renvoie faux si la position de l'icône est encore inconnue (panneau laissé en place).
fn place_panel_sized(app: &AppHandle, window: &WebviewWindow, panel: PhysicalSize<f64>) -> bool {
    let Some((tray_pos, tray_size)) = tray_rect(app) else {
        return false;
    };
    let Some(monitor) = panel_monitor(app, window) else {
        return false;
    };

    let gap = GAP * monitor.scale_factor();
    let (panel_w, panel_h) = (panel.width, panel.height);
    let area = monitor.work_area();
    let (area_x, area_y) = (area.position.x as f64, area.position.y as f64);
    let (area_w, area_h) = (area.size.width as f64, area.size.height as f64);
    let screen_mid = monitor.position().y as f64 + monitor.size().height as f64 / 2.0;

    // Horizontal : centré sous l'icône, sans sortir de la zone utile.
    let x = (tray_pos.x + tray_size.width / 2.0 - panel_w / 2.0)
        .min(area_x + area_w - panel_w - gap)
        .max(area_x + gap);

    // Vertical : selon que l'icône est dans la moitié haute ou basse de l'écran,
    // puis ramené dans la zone utile (le Dock ne recouvre pas le bas du panneau).
    let below_icon = tray_pos.y < screen_mid;
    let y = if below_icon {
        tray_pos.y + tray_size.height + gap
    } else {
        tray_pos.y - panel_h - gap
    }
    .min(area_y + area_h - panel_h - gap)
    .max(area_y + gap);

    let _ = window.set_position(PhysicalPosition::new(x.round() as i32, y.round() as i32));
    true
}

/// Ajuste la taille du panneau (calculée par l'interface, en points) sans le décoller de
/// l'icône : il raccourcit quand on masque des indicateurs ; `width` : panneau agrandi.
///
/// Mac : changer la taille d'une fenêtre garde son coin bas-gauche fixe ; le panneau
/// est donc replacé juste après, pour que son bord haut reste sous l'icône.
#[tauri::command]
fn resize_panel(app: AppHandle, height: f64, width: Option<f64>) -> Result<(), String> {
    if !height.is_finite() || width.is_some_and(|w| !w.is_finite()) {
        return Err("taille invalide".into());
    }
    let window = app.get_webview_window(PANEL).ok_or("panneau introuvable")?;
    let scale = window.scale_factor().map_err(|e| e.to_string())?;
    let current = window.inner_size().map_err(|e| e.to_string())?;

    // Pas plus grand que la zone utile de l'écran (sans barre des menus ni Dock) :
    // au-delà, la liste défile.
    let area = panel_monitor(&app, &window)
        .map(|m| {
            let (area, s) = (m.work_area(), m.scale_factor());
            (area.size.width as f64 / s, area.size.height as f64 / s)
        })
        .unwrap_or((1440.0, 820.0));
    let max_height = (area.1 - 2.0 * GAP).max(MIN_PANEL_HEIGHT);
    let height = height.round().clamp(MIN_PANEL_HEIGHT, max_height);
    // Largeur : celle demandée (panneau agrandi) ou la largeur actuelle.
    let width = match width {
        Some(w) => w
            .round()
            .clamp(MIN_PANEL_WIDTH, (area.0 - 2.0 * GAP).max(MIN_PANEL_WIDTH)),
        None => current.width as f64 / scale,
    };
    let same_height = (current.height as f64 / scale - height).abs() < 1.0;
    let same_width = (current.width as f64 / scale - width).abs() < 1.0;
    if same_height && same_width {
        return Ok(()); // déjà à la bonne taille
    }

    let top_left = window.outer_position().ok();
    window
        .set_size(LogicalSize::new(width, height))
        .map_err(|e| e.to_string())?;
    let size = PhysicalSize::new(width * scale, height * scale);
    if !place_panel_sized(&app, &window, size) {
        // Icône pas encore repérée : le bord haut du panneau reste où il était.
        if let Some(position) = top_left {
            let _ = window.set_position(position);
        }
    }
    Ok(())
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

/// Image PNG reçue de l'interface en binaire brut (pas de JSON).
/// Vérification minimale : un PNG commence toujours par ces 8 octets.
fn png_body<'a>(request: &'a tauri::ipc::Request<'_>) -> Result<&'a [u8], String> {
    let tauri::ipc::InvokeBody::Raw(bytes) = request.body() else {
        return Err("image manquante".into());
    };
    if !bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
        return Err("le fichier reçu n'est pas une image PNG".into());
    }
    Ok(bytes)
}

/// Valeur d'un en-tête de la requête (vide si absent).
fn header<'a>(request: &'a tauri::ipc::Request<'_>, name: &str) -> &'a str {
    request
        .headers()
        .get(name)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
}

/// Formats d'image connus (nom du fichier) : portrait 4:5, paysage 16:9, story 9:16.
const EXPORT_FORMATS: [&str; 3] = ["portrait", "landscape", "story"];

/// Enregistre l'image exportée (PNG) dans Images/Glaux, puis la montre dans le Finder
/// (ou l'Explorateur sous Windows). Renvoie le chemin du fichier.
///
/// En-têtes : « x-indicator » (indicateur ou groupe) et « x-format ».
/// Le nom du fichier est construit ici à partir de valeurs connues, jamais du texte reçu.
#[tauri::command]
fn save_png(app: AppHandle, request: tauri::ipc::Request<'_>) -> Result<String, String> {
    let bytes = png_body(&request)?;
    let name = store::export_name(header(&request, "x-indicator"))
        .ok_or_else(|| "indicateur inconnu".to_string())?;
    let format = EXPORT_FORMATS
        .into_iter()
        .find(|f| *f == header(&request, "x-format"))
        .unwrap_or("portrait");

    let dir = app
        .path()
        .picture_dir()
        .map_err(|e| format!("dossier Images introuvable : {e}"))?
        .join("Glaux");
    std::fs::create_dir_all(&dir).map_err(|e| format!("création du dossier impossible : {e}"))?;

    let stamp = chrono::Local::now().format("%Y-%m-%d_%H%M%S");
    let path: PathBuf = dir.join(format!("Glaux_{name}_{format}_{stamp}.png"));
    std::fs::write(&path, bytes).map_err(|e| format!("enregistrement impossible : {e}"))?;

    reveal_in_file_manager(&path);
    Ok(path.to_string_lossy().into_owned())
}

/// Exporte toutes les séries dans un fichier Excel (Documents/Glaux), puis le montre dans le
/// Finder (ou l'Explorateur). Construit à partir du cache, sans réseau. Renvoie le chemin.
#[tauri::command]
async fn export_excel(app: AppHandle) -> Result<String, String> {
    // Hors du fil principal : le panneau reste réactif pendant la compression du fichier.
    tauri::async_runtime::spawn_blocking(move || {
        let store = app.state::<Store>();
        let lang = store.lang();
        let downloaded = store
            .last_success()
            .and_then(|t| chrono::DateTime::from_timestamp(t, 0))
            .map(|t| {
                t.with_timezone(&chrono::Local)
                    .format("%d/%m/%Y %H:%M")
                    .to_string()
            })
            .unwrap_or_else(|| "—".into());
        let version = app.package_info().version.to_string();
        let bytes = excel::build_workbook(&store.export_series(), lang, &downloaded, &version)?;

        let dir = app
            .path()
            .document_dir()
            .map_err(|e| format!("dossier Documents introuvable : {e}"))?
            .join("Glaux");
        std::fs::create_dir_all(&dir)
            .map_err(|e| format!("création du dossier impossible : {e}"))?;
        let stamp = chrono::Local::now().format("%Y-%m-%d_%H%M%S");
        let name = lang.pick("donnees", "data");
        let path: PathBuf = dir.join(format!("Glaux_{name}_{stamp}.xlsx"));
        std::fs::write(&path, bytes).map_err(|e| format!("enregistrement impossible : {e}"))?;

        reveal_in_file_manager(&path);
        Ok(path.to_string_lossy().into_owned())
    })
    .await
    .map_err(|e| format!("export interrompu : {e}"))?
}

/// Copie l'image exportée dans le presse-papiers, prête à coller dans un post.
#[tauri::command]
fn copy_png(app: AppHandle, request: tauri::ipc::Request<'_>) -> Result<(), String> {
    let bytes = png_body(&request)?;
    let image =
        tauri::image::Image::from_bytes(bytes).map_err(|e| format!("image illisible : {e}"))?;
    app.clipboard()
        .write_image(&image)
        .map_err(|e| format!("copie impossible : {e}"))
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

/// Ordre et visibilité des lignes du panneau (écran « Indicateurs affichés »).
/// `hidden` : toutes les lignes masquées à l'écran.
#[tauri::command]
fn set_layout(app: AppHandle, order: Vec<String>, hidden: Vec<String>) -> Result<(), String> {
    app.state::<Store>().set_layout(&order, &hidden)?;
    notify(&app);
    Ok(())
}

/// Courbe des taux du jour et celle de `months` mois plus tôt (vue détail, image exportée).
#[tauri::command]
fn get_curve(store: State<'_, Store>, months: u32) -> store::CurveView {
    store.curve(months)
}

/// « Par défaut » : ordre d'origine, lignes habituelles, pays par défaut des écarts de taux.
#[tauri::command]
fn reset_layout(app: AppHandle) -> Result<(), String> {
    app.state::<Store>().reset_layout()?;
    notify(&app);
    Ok(())
}

/// Pays d'une ligne à choix (1 à 3) : « yields » (ex. « yield_fr ») ou « spreads » (« spread_gr »).
#[tauri::command]
fn set_selection(app: AppHandle, group: String, members: Vec<String>) -> Result<(), String> {
    app.state::<Store>().set_selection(&group, &members)?;
    notify(&app);
    Ok(())
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
        // Presse-papiers (copie de l'image exportée).
        .plugin(tauri_plugin_clipboard_manager::init())
        // Mises à jour automatiques (src/updates.rs ; adresse et clé publique : tauri.conf.json).
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(UpdateState::default())
        // Commandes Rust que l'interface peut appeler via invoke().
        .invoke_handler(tauri::generate_handler![
            get_dashboard,
            get_history,
            refresh_now,
            save_png,
            copy_png,
            export_excel,
            set_notifications,
            set_language,
            set_layout,
            reset_layout,
            set_selection,
            get_curve,
            resize_panel,
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
