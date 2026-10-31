//! Agent Surveo du poste élève : application sans fenêtre hors séance
//! (SPEC.md, AGENT-1), qui affiche l'indicateur de séance (REJ-4,
//! AGENT-2) et les restrictions en cours (MAIT-1), le numéro
//! d'identification (SALLE-4), l'écran en pause (ECRAN-1) et les
//! notifications à l'élève (AGENT-4).
//!
//! Usage : surveo-agent [--sur-place] [--dossier DOSSIER]
//!                      [--ecran-factice N] [--desinstaller]
//!
//! Lancé depuis un dossier qui contient le fichier de rattachement,
//! l'agent s'installe dans le compte de l'utilisateur, s'inscrit au
//! démarrage de session et se relance depuis là (INST-1, INST-3).
//! `--sur-place` le fait tourner sans l'installer ; `--dossier` change
//! son dossier (plusieurs agents sur un même poste, pour les essais) ;
//! `--ecran-factice` envoie des miniatures de démonstration.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use surveo_agent::core::{self, AgentSetup, SessionView, Ui};
use surveo_agent::restrict::RestrictionsView;
use surveo_agent::setup;
use surveo_platform::Component;
use tauri::{Emitter, Manager, PhysicalPosition, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

const INDICATOR: &str = "indicator";
const IDENTIFY: &str = "identify";
const NOTICE: &str = "notice";
/// Fenêtres de l'écran en pause, une par écran : `pause-0`, `pause-1`…
const PAUSE: &str = "pause-";
/// Taille de l'indicateur, déplié et réduit (AGENT-2) ; il gagne une
/// ligne quand des restrictions s'appliquent (MAIT-1).
const INDICATOR_SIZE: (f64, f64) = (560.0, 46.0);
const INDICATOR_TALL: f64 = 74.0;
const INDICATOR_COMPACT: (f64, f64) = (150.0, 46.0);
const NOTICE_SIZE: (f64, f64) = (480.0, 64.0);
/// Durée d'affichage d'une notification.
const NOTICE_SECONDS: u64 = 8;

fn arg(name: &str) -> Option<String> {
    let mut it = std::env::args();
    while let Some(a) = it.next() {
        if a == name {
            return it.next();
        }
    }
    None
}

fn flag(name: &str) -> bool {
    std::env::args().any(|a| a == name)
}

#[derive(Default)]
struct AppState {
    session: Mutex<Option<SessionView>>,
    restrictions: Mutex<RestrictionsView>,
    compact: Mutex<bool>,
    pause: Mutex<Option<String>>,
    notice: Mutex<Option<String>>,
    /// Numéro de la dernière notification, pour ne fermer que la sienne.
    notice_gen: AtomicU64,
    error: Mutex<Option<String>>,
}

/// Ce que l'indicateur affiche.
#[derive(serde::Serialize)]
struct IndicatorState {
    session: Option<SessionView>,
    restrictions: RestrictionsView,
}

fn indicator_size(app: &tauri::AppHandle) -> (f64, f64) {
    let state = app.state::<AppState>();
    if *state.compact.lock().unwrap() {
        INDICATOR_COMPACT
    } else if state.restrictions.lock().unwrap().items.is_empty() {
        INDICATOR_SIZE
    } else {
        (INDICATOR_SIZE.0, INDICATOR_TALL)
    }
}

type Job = Box<dyn FnOnce(&tauri::AppHandle) + Send>;

/// Affichage par fenêtres Tauri, créées à la demande.
///
/// Les fenêtres sont créées et fermées par un fil à part, dans l'ordre des
/// demandes : une création peut prendre plusieurs secondes et ne doit
/// retarder ni le proxy de filtrage, ni les réponses à la console. Chaque
/// tâche met les fenêtres en accord avec l'état du moment, si bien qu'une
/// demande dépassée ne laisse rien derrière elle.
struct TauriUi {
    app: tauri::AppHandle,
    jobs: std::sync::mpsc::Sender<Job>,
}

impl TauriUi {
    fn new(app: tauri::AppHandle) -> Self {
        let (jobs, rx) = std::sync::mpsc::channel::<Job>();
        let a = app.clone();
        std::thread::Builder::new()
            .name("fenetres".into())
            .spawn(move || {
                for job in rx {
                    job(&a);
                }
            })
            .expect("fil des fenêtres");
        Self { app, jobs }
    }

    fn windows(&self, job: impl FnOnce(&tauri::AppHandle) + Send + 'static) {
        let _ = self.jobs.send(Box::new(job));
    }
}

/// Indicateur présent pendant la séance seulement (REJ-4).
fn sync_indicator(app: &tauri::AppHandle) {
    let wanted = app.state::<AppState>().session.lock().unwrap().is_some();
    match (wanted, app.get_webview_window(INDICATOR)) {
        (true, None) => {
            if let Err(e) = indicator_window(app) {
                log::warn!("indicateur de séance impossible à afficher : {e}");
            }
        }
        (false, Some(w)) => {
            let _ = w.destroy();
        }
        (true, Some(w)) => {
            let (wd, h) = indicator_size(app);
            let _ = w.set_size(tauri::LogicalSize::new(wd, h));
        }
        (false, None) => {}
    }
}

/// Fenêtres de pause présentes tant que la pause dure (ECRAN-1).
fn sync_pause(app: &tauri::AppHandle) {
    let wanted = app.state::<AppState>().pause.lock().unwrap().is_some();
    let windows: Vec<WebviewWindow> = app.webview_windows().into_iter().filter(|(l, _)| l.starts_with(PAUSE)).map(|(_, w)| w).collect();
    if !wanted {
        for w in windows {
            let _ = w.destroy();
        }
        return;
    }
    if !windows.is_empty() {
        return;
    }
    if let Err(e) = pause_windows(app) {
        log::warn!("écran en pause impossible à afficher : {e}");
        return;
    }
    // L'élève ne peut plus agir sur son poste (ECRAN-1) : la pause reprend
    // le premier plan si une autre fenêtre le lui prend.
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(1)).await;
            let Some(w) = app.get_webview_window(&format!("{PAUSE}0")) else { return };
            if !w.is_focused().unwrap_or(true) {
                let _ = w.set_focus();
            }
        }
    });
}

impl Ui for TauriUi {
    fn session(&self, s: Option<&SessionView>) {
        *self.app.state::<AppState>().session.lock().unwrap() = s.cloned();
        let _ = self.app.emit("session", s);
        self.windows(sync_indicator);
    }

    fn identify(&self, number: &str, seconds: u32) {
        let number = number.to_string();
        self.windows(move |app| {
            if let Some(w) = app.get_webview_window(IDENTIFY) {
                let _ = w.destroy();
            }
            let url = WebviewUrl::App(format!("index.html#identify/{number}").into());
            let built = WebviewWindowBuilder::new(app, IDENTIFY, url)
                .title("Surveo · identification")
                .inner_size(420.0, 300.0)
                .center()
                .decorations(false)
                .always_on_top(true)
                .skip_taskbar(true)
                .resizable(false)
                .focused(false)
                .build();
            match built {
                Ok(w) => {
                    tauri::async_runtime::spawn(async move {
                        tokio::time::sleep(Duration::from_secs(seconds as u64)).await;
                        let _ = w.destroy();
                    });
                }
                Err(e) => log::warn!("identification impossible à afficher : {e}"),
            }
        });
    }

    fn restrictions(&self, r: &RestrictionsView) {
        *self.app.state::<AppState>().restrictions.lock().unwrap() = r.clone();
        let _ = self.app.emit("restrictions", r);
        self.windows(sync_indicator);
    }

    fn pause(&self, message: Option<&str>) -> Result<(), String> {
        *self.app.state::<AppState>().pause.lock().unwrap() = message.map(Into::into);
        if let Some(m) = message {
            let _ = self.app.emit("pause", m);
        }
        self.windows(sync_pause);
        Ok(())
    }

    fn notify(&self, text: &str) {
        let state = self.app.state::<AppState>();
        *state.notice.lock().unwrap() = Some(text.into());
        let gen = state.notice_gen.fetch_add(1, Ordering::SeqCst) + 1;
        let _ = self.app.emit("notice", text);
        let jobs = self.jobs.clone();
        self.windows(move |app| {
            if app.get_webview_window(NOTICE).is_none() {
                if let Err(e) = notice_window(app) {
                    log::warn!("notification impossible à afficher : {e}");
                    return;
                }
            }
            // Fermée au bout de quelques secondes, sauf si une autre l'a
            // remplacée entre-temps.
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(Duration::from_secs(NOTICE_SECONDS)).await;
                let _ = jobs.send(Box::new(move |app: &tauri::AppHandle| {
                    if app.state::<AppState>().notice_gen.load(Ordering::SeqCst) == gen {
                        if let Some(w) = app.get_webview_window(NOTICE) {
                            let _ = w.destroy();
                        }
                    }
                }));
            });
        });
    }
}

/// Écran en pause : une fenêtre plein écran par écran, au premier plan,
/// qui ne se ferme pas (ECRAN-1). Rien n'est fermé dessous (ECRAN-4).
fn pause_windows(app: &tauri::AppHandle) -> tauri::Result<()> {
    let monitors = app.available_monitors()?;
    let places: Vec<Option<(f64, f64, f64, f64)>> = if monitors.is_empty() {
        vec![None]
    } else {
        monitors
            .iter()
            .map(|m| {
                let (p, s) = (m.position().to_logical::<f64>(m.scale_factor()), m.size().to_logical::<f64>(m.scale_factor()));
                Some((p.x, p.y, s.width, s.height))
            })
            .collect()
    };
    for (i, place) in places.into_iter().enumerate() {
        let mut b = WebviewWindowBuilder::new(app, format!("{PAUSE}{i}"), WebviewUrl::App("index.html#pause".into()))
            .title("Surveo · écran en pause")
            .decorations(false)
            .always_on_top(true)
            .skip_taskbar(true)
            .closable(false)
            .resizable(false)
            .focused(i == 0);
        if let Some((x, y, w, h)) = place {
            b = b.position(x, y).inner_size(w, h);
        }
        b.fullscreen(true).build()?;
    }
    Ok(())
}

/// Notification discrète, sous l'indicateur de séance (AGENT-4).
fn notice_window(app: &tauri::AppHandle) -> tauri::Result<WebviewWindow> {
    let w = WebviewWindowBuilder::new(app, NOTICE, WebviewUrl::App("index.html#notice".into()))
        .title("Surveo · notification")
        .inner_size(NOTICE_SIZE.0, NOTICE_SIZE.1)
        .min_inner_size(NOTICE_SIZE.0, NOTICE_SIZE.1)
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .focused(false)
        .build()?;
    if let Ok(Some(m)) = w.primary_monitor() {
        let scale = m.scale_factor();
        let x = m.position().x + ((m.size().width as f64 - NOTICE_SIZE.0 * scale) / 2.0) as i32;
        let y = m.position().y + ((16.0 + INDICATOR_TALL) * scale) as i32;
        let _ = w.set_position(PhysicalPosition::new(x, y));
    }
    Ok(w)
}

/// Indicateur de séance : petite barre en haut de l'écran, toujours
/// visible, qu'on peut réduire ou déplacer mais pas fermer (AGENT-2).
fn indicator_window(app: &tauri::AppHandle) -> tauri::Result<WebviewWindow> {
    let (wd, h) = indicator_size(app);
    let w = WebviewWindowBuilder::new(app, INDICATOR, WebviewUrl::App("index.html#indicator".into()))
        .title("Surveo · séance en cours")
        .inner_size(wd, h)
        .min_inner_size(INDICATOR_COMPACT.0, INDICATOR_COMPACT.1)
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .focused(false)
        .closable(false)
        .build()?;
    if let Ok(Some(m)) = w.primary_monitor() {
        let scale = m.scale_factor();
        let x = m.position().x + ((m.size().width as f64 - INDICATOR_SIZE.0 * scale) / 2.0) as i32;
        let _ = w.set_position(PhysicalPosition::new(x, m.position().y + (8.0 * scale) as i32));
    }
    Ok(w)
}

#[tauri::command]
fn indicator_state(state: tauri::State<'_, AppState>) -> IndicatorState {
    IndicatorState { session: state.session.lock().unwrap().clone(), restrictions: state.restrictions.lock().unwrap().clone() }
}

#[tauri::command]
fn indicator_compact(app: tauri::AppHandle, compact: bool) {
    *app.state::<AppState>().compact.lock().unwrap() = compact;
    if let Some(w) = app.get_webview_window(INDICATOR) {
        let (wd, h) = indicator_size(&app);
        let _ = w.set_size(tauri::LogicalSize::new(wd, h));
    }
}

#[tauri::command]
fn pause_message(state: tauri::State<'_, AppState>) -> Option<String> {
    state.pause.lock().unwrap().clone()
}

#[tauri::command]
fn notice_text(state: tauri::State<'_, AppState>) -> Option<String> {
    state.notice.lock().unwrap().clone()
}

#[tauri::command]
fn startup_error(state: tauri::State<'_, AppState>) -> Option<String> {
    state.error.lock().unwrap().clone()
}

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let dir = arg("--dossier").map(PathBuf::from).unwrap_or_else(|| surveo_platform::data_dir(Component::Agent));

    if flag("--desinstaller") {
        match setup::uninstall(&dir) {
            Ok(()) => println!("Agent Surveo désinstallé : {} effacé.", dir.display()),
            Err(e) => {
                eprintln!("Désinstallation incomplète : {e:#}");
                std::process::exit(1);
            }
        }
        return;
    }

    // Premier lancement depuis le dossier du paquet : installation, puis
    // relance depuis le dossier de l'agent (INST-1).
    if !flag("--sur-place") && !setup::is_installed(&dir) {
        let beside = std::env::current_exe().ok().and_then(|e| e.parent().map(|p| p.join(setup::PACKAGE_FILE)));
        if beside.is_some_and(|p| p.exists()) {
            match setup::install(&dir) {
                Ok(exe) => {
                    log::info!("agent installé dans {}", dir.display());
                    match std::process::Command::new(&exe).spawn() {
                        Ok(_) => return,
                        Err(e) => log::warn!("relance de l'agent installé impossible : {e}"),
                    }
                }
                Err(e) => log::warn!("installation impossible, l'agent tourne sur place : {e:#}"),
            }
        }
    }

    let _lock = match setup::lock(&dir) {
        Ok(Some(l)) => l,
        Ok(None) => {
            log::info!("un agent tourne déjà pour ce compte");
            return;
        }
        Err(e) => {
            eprintln!("Dossier de l'agent inutilisable ({}) : {e:#}", dir.display());
            std::process::exit(1);
        }
    };

    let prepared = setup::load_package(&dir).and_then(|p| {
        let id = setup::load_or_enroll(&dir, &p)?;
        let mut s = AgentSetup::new(dir.clone(), p, id);
        s.synthetic_screen = arg("--ecran-factice").and_then(|v| v.parse().ok());
        Ok(s)
    });

    tauri::Builder::default()
        .manage(AppState::default())
        .setup(move |app| {
            match prepared {
                Ok(s) => {
                    log::info!(
                        "poste {} de la salle {} ({})",
                        s.identity.member(),
                        s.package.room_name,
                        s.package.etab.name
                    );
                    let ui = Arc::new(TauriUi::new(app.handle().clone()));
                    tauri::async_runtime::spawn(async move {
                        if let Err(e) = core::run(s, ui).await {
                            log::error!("agent arrêté : {e:#}");
                        }
                    });
                }
                Err(e) => {
                    // Seul cas où l'agent ouvre une fenêtre hors séance : il ne
                    // peut rien faire sans rattachement.
                    let text = format!(
                        "Ce poste n'est rattaché à aucun établissement : {e:#}. \
                         Placez le fichier {} à côté de l'agent, puis relancez-le.",
                        setup::PACKAGE_FILE
                    );
                    log::error!("{text}");
                    *app.state::<AppState>().error.lock().unwrap() = Some(text);
                    WebviewWindowBuilder::new(app, "error", WebviewUrl::App("index.html#error".into()))
                        .title("Surveo")
                        .inner_size(520.0, 220.0)
                        .center()
                        .build()?;
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![indicator_state, indicator_compact, pause_message, notice_text, startup_error])
        .build(tauri::generate_context!())
        .expect("échec du lancement de l'agent")
        .run(|app, event| match event {
            // Sans fenêtre, l'agent continue de tourner (AGENT-1), sauf s'il
            // n'a pas pu démarrer…
            tauri::RunEvent::ExitRequested { api, code: None, .. } => {
                if app.state::<AppState>().error.lock().unwrap().is_none() {
                    api.prevent_exit();
                }
            }
            // …et ni l'indicateur ni l'écran en pause ne se ferment pendant
            // la séance (AGENT-2, ECRAN-1).
            tauri::RunEvent::WindowEvent { label, event: tauri::WindowEvent::CloseRequested { api, .. }, .. }
                if label == INDICATOR || label.starts_with(PAUSE) =>
            {
                api.prevent_close();
            }
            _ => {}
        });
}
