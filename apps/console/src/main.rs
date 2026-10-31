//! Console Surveo de l'enseignant (SPEC.md, chap. 1 et 9), avec le relais
//! intégré (RELAIS-1 : relais dans la classe, sans aucun serveur).
//!
//! Usage : surveo-console [--dossier DOSSIER]
//!
//! L'interface (ui/console) reçoit l'état complet par l'événement `state`
//! à chaque changement, les miniatures par `thumb` et les retours
//! d'action par `notice`.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use base64::Engine as _;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;
use surveo_console::engine::Engine;
use surveo_console::store::{Pos, Store};
use surveo_console::view::{ConsoleView, RelayView, Thumb};
use surveo_console::now_ms;
use surveo_identity::save_json;
use surveo_platform::Component;
use surveo_protocol::{decode_console_frame, BinKind, MemberId, ToConsole, DISCOVERY_PORT, RELAY_PORT};
use surveo_relay_core::{Frame, Relay, RelayConfig};
use tauri::{Emitter, Manager};

struct Shared {
    engine: Mutex<Engine>,
    /// Relais intégré, gardé en vie avec la console.
    relay: Mutex<Option<Relay>>,
}

type Res<T> = Result<T, String>;

fn arg(name: &str) -> Option<String> {
    let mut it = std::env::args();
    while let Some(a) = it.next() {
        if a == name {
            return it.next();
        }
    }
    None
}

/// Envoie l'état et les retours d'action à l'interface.
fn publish(app: &tauri::AppHandle) {
    let (view, notices) = {
        let shared = app.state::<Shared>();
        let mut e = shared.engine.lock().unwrap();
        (e.view(), e.take_notices())
    };
    let _ = app.emit("state", view);
    for n in notices {
        let _ = app.emit("notice", n);
    }
}

/// Applique un geste de l'enseignant au moteur, puis publie l'état.
fn act(app: &tauri::AppHandle, f: impl FnOnce(&mut Engine, u64) -> anyhow::Result<()>) -> Res<()> {
    let r = f(&mut app.state::<Shared>().engine.lock().unwrap(), now_ms());
    publish(app);
    r.map_err(|e| format!("{e:#}"))
}

#[tauri::command]
fn state(app: tauri::AppHandle) -> ConsoleView {
    app.state::<Shared>().engine.lock().unwrap().view()
}

#[tauri::command]
fn create_establishment(app: tauri::AppHandle, etab_name: String, teacher: String) -> Res<()> {
    act(&app, |e, _| e.create_establishment(&etab_name, &teacher))?;
    start_relay(app);
    Ok(())
}

#[tauri::command]
fn create_room(app: tauri::AppHandle, name: String, cols: u8, rows: u8) -> Res<()> {
    act(&app, |e, _| e.create_room(&name, cols, rows).map(|_| ()))
}

#[tauri::command]
fn select_room(app: tauri::AppHandle, id: String) -> Res<()> {
    act(&app, |e, _| e.select_room(&id))
}

#[tauri::command]
fn resize_room(app: tauri::AppHandle, cols: u8, rows: u8) -> Res<()> {
    act(&app, |e, _| e.resize_room(cols, rows))
}

#[tauri::command]
fn place(app: tauri::AppHandle, member: String, col: u8, row: u8) -> Res<()> {
    act(&app, |e, _| e.place(&member, Pos { col, row }))
}

#[tauri::command]
fn unplace(app: tauri::AppHandle, member: String) -> Res<()> {
    act(&app, |e, _| e.unplace(&member))
}

#[tauri::command]
fn open_session(app: tauri::AppHandle, end_ms: u64) -> Res<()> {
    act(&app, |e, now| e.open_session(end_ms, now))
}

#[tauri::command]
fn set_end(app: tauri::AppHandle, end_ms: u64) -> Res<()> {
    act(&app, |e, now| e.set_end(end_ms, now))
}

#[tauri::command]
fn close_session(app: tauri::AppHandle) -> Res<()> {
    act(&app, |e, now| e.close_session(now, false))
}

#[tauri::command]
fn set_screens(app: tauri::AppHandle, on: bool) -> Res<()> {
    act(&app, |e, _| e.set_screens(on))
}

#[tauri::command]
fn identify(app: tauri::AppHandle, members: Vec<MemberId>) -> Res<()> {
    act(&app, |e, now| e.identify(&members, now))
}

/// Enregistre le paquet de rattachement d'une salle (INST-3) à
/// l'emplacement choisi par l'enseignant.
#[tauri::command]
fn export_package(app: tauri::AppHandle, room_id: String, path: String) -> Res<()> {
    let package = app.state::<Shared>().engine.lock().unwrap().package(&room_id).map_err(|e| format!("{e:#}"))?;
    save_json(&PathBuf::from(path), &package, true).map_err(|e| format!("{e:#}"))
}

/// Nom de fichier proposé pour le paquet d'une salle.
#[tauri::command]
fn package_file_name() -> &'static str {
    "surveo-rattachement.json"
}

/// Démarre le relais intégré et y attache la console.
fn start_relay(app: tauri::AppHandle) {
    tauri::async_runtime::spawn(async move {
        let Some((identity, trust, console)) = app.state::<Shared>().engine.lock().unwrap().relay_identity() else { return };
        let cfg = RelayConfig { identity, trust, port: RELAY_PORT, discovery: Some(DISCOVERY_PORT) };
        match Relay::start(cfg).await {
            Ok(relay) => {
                let local = relay.attach_console(&console);
                {
                    let shared = app.state::<Shared>();
                    let mut e = shared.engine.lock().unwrap();
                    e.relay = RelayView {
                        running: true,
                        port: relay.port(),
                        addresses: surveo_net::discovery::local_addresses().iter().map(|a| a.to_string()).collect(),
                        discovery_error: relay.discovery_error().map(Into::into),
                        error: None,
                    };
                    e.attach(Box::new(local.sender.clone()));
                }
                *app.state::<Shared>().relay.lock().unwrap() = Some(relay);
                publish(&app);
                let mut frames = local.frames;
                while let Some(f) = frames.recv().await {
                    match f {
                        Frame::Text(t) => match serde_json::from_str::<ToConsole>(&t) {
                            Ok(m) => {
                                app.state::<Shared>().engine.lock().unwrap().handle(m, now_ms());
                                publish(&app);
                            }
                            Err(e) => log::debug!("message du relais illisible : {e}"),
                        },
                        Frame::Binary(b) => {
                            if let Some((BinKind::Thumbnail, member, jpeg)) = decode_console_frame(&b) {
                                let data = base64::engine::general_purpose::STANDARD.encode(jpeg);
                                let _ = app.emit("thumb", Thumb { member: member.into(), data });
                            }
                        }
                    }
                }
            }
            Err(e) => {
                log::error!("relais intégré impossible à démarrer : {e:#}");
                app.state::<Shared>().engine.lock().unwrap().relay.error = Some(format!("{e:#}"));
                publish(&app);
            }
        }
    });
}

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let dir = arg("--dossier").map(PathBuf::from).unwrap_or_else(|| surveo_platform::data_dir(Component::Console));
    let engine = match Engine::new(Store::new(dir.clone()), now_ms()) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("Données de la console illisibles ({}) : {e:#}", dir.display());
            std::process::exit(1);
        }
    };
    let set_up = engine.is_set_up();
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(Shared { engine: Mutex::new(engine), relay: Mutex::new(None) })
        .setup(move |app| {
            if set_up {
                start_relay(app.handle().clone());
            }
            // Échéances de la séance et des retours d'action.
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let mut tick = tokio::time::interval(Duration::from_secs(1));
                loop {
                    tick.tick().await;
                    let changed = handle.state::<Shared>().engine.lock().unwrap().tick(now_ms());
                    if changed {
                        publish(&handle);
                    }
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            state,
            create_establishment,
            create_room,
            select_room,
            resize_room,
            place,
            unplace,
            open_session,
            set_end,
            close_session,
            set_screens,
            identify,
            export_package,
            package_file_name
        ])
        .run(tauri::generate_context!())
        .expect("échec du lancement de la console");
}
