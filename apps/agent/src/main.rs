//! Agent Surveo du poste élève : application sans fenêtre hors séance
//! (SPEC.md, AGENT-1), qui affiche l'indicateur de séance (REJ-4,
//! AGENT-2) et le numéro d'identification (SALLE-4).
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
use std::sync::{Arc, Mutex};
use surveo_agent::core::{self, AgentSetup, SessionView, Ui};
use surveo_agent::setup;
use surveo_platform::Component;
use tauri::{Emitter, Manager, PhysicalPosition, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

const INDICATOR: &str = "indicator";
const IDENTIFY: &str = "identify";
/// Taille de l'indicateur, déplié et réduit (AGENT-2).
const INDICATOR_SIZE: (f64, f64) = (560.0, 46.0);
const INDICATOR_COMPACT: (f64, f64) = (150.0, 46.0);

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
    error: Mutex<Option<String>>,
}

/// Affichage par fenêtres Tauri, créées à la demande.
struct TauriUi {
    app: tauri::AppHandle,
}

impl Ui for TauriUi {
    fn session(&self, s: Option<&SessionView>) {
        *self.app.state::<AppState>().session.lock().unwrap() = s.cloned();
        let _ = self.app.emit("session", s);
        match (s, self.app.get_webview_window(INDICATOR)) {
            (Some(_), None) => {
                if let Err(e) = indicator_window(&self.app) {
                    log::warn!("indicateur de séance impossible à afficher : {e}");
                }
            }
            (None, Some(w)) => {
                let _ = w.destroy();
            }
            _ => {}
        }
    }

    fn identify(&self, number: &str, seconds: u32) {
        if let Some(w) = self.app.get_webview_window(IDENTIFY) {
            let _ = w.destroy();
        }
        let url = WebviewUrl::App(format!("index.html#identify/{number}").into());
        let built = WebviewWindowBuilder::new(&self.app, IDENTIFY, url)
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
                    tokio::time::sleep(std::time::Duration::from_secs(seconds as u64)).await;
                    let _ = w.destroy();
                });
            }
            Err(e) => log::warn!("identification impossible à afficher : {e}"),
        }
    }
}

/// Indicateur de séance : petite barre en haut de l'écran, toujours
/// visible, qu'on peut réduire ou déplacer mais pas fermer (AGENT-2).
fn indicator_window(app: &tauri::AppHandle) -> tauri::Result<WebviewWindow> {
    let w = WebviewWindowBuilder::new(app, INDICATOR, WebviewUrl::App("index.html#indicator".into()))
        .title("Surveo · séance en cours")
        .inner_size(INDICATOR_SIZE.0, INDICATOR_SIZE.1)
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
fn indicator_state(state: tauri::State<'_, AppState>) -> Option<SessionView> {
    state.session.lock().unwrap().clone()
}

#[tauri::command]
fn indicator_compact(app: tauri::AppHandle, compact: bool) {
    if let Some(w) = app.get_webview_window(INDICATOR) {
        let (wd, h) = if compact { INDICATOR_COMPACT } else { INDICATOR_SIZE };
        let _ = w.set_size(tauri::LogicalSize::new(wd, h));
    }
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
                    let ui = Arc::new(TauriUi { app: app.handle().clone() });
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
        .invoke_handler(tauri::generate_handler![indicator_state, indicator_compact, startup_error])
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
            // …et l'indicateur ne se ferme pas pendant la séance (AGENT-2).
            tauri::RunEvent::WindowEvent { label, event: tauri::WindowEvent::CloseRequested { api, .. }, .. }
                if label == INDICATOR =>
            {
                api.prevent_close();
            }
            _ => {}
        });
}
