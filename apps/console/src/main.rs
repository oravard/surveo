//! Console Surveo de l'enseignant (SPEC.md, chap. 1 à 3 et 9), avec le
//! relais intégré (RELAIS-1 : relais dans la classe, sans aucun serveur).
//!
//! Usage : surveo-console [--dossier DOSSIER]
//!
//! Les rendus sont rangés dans Documents/Surveo/Rendus, ou dans
//! DOSSIER/rendus avec `--dossier` (RENDU-6).
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
use surveo_console::library::{Attachment, Model, PollModel};
use surveo_console::store::{Pos, Store};
use surveo_console::view::{ConsoleView, RelayView, Thumb};
use surveo_console::now_ms;
use surveo_identity::save_json;
use surveo_platform::Component;
use surveo_protocol::{decode_console_frame, BinKind, MemberId, ToConsole, WebPolicy, DISCOVERY_PORT, RELAY_PORT};
use surveo_platform::files::open_path;
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

// --- Restrictions (SPEC.md, chap. 2) : sans poste désigné, toute la salle.

#[tauri::command]
fn set_pause(app: tauri::AppHandle, members: Vec<MemberId>, message: Option<String>) -> Res<()> {
    act(&app, |e, now| e.set_pause(&members, message, now))
}

#[tauri::command]
fn set_web(app: tauri::AppHandle, members: Vec<MemberId>, policy: Option<WebPolicy>) -> Res<()> {
    act(&app, |e, now| e.set_web(&members, policy, now))
}

#[tauri::command]
fn set_banned_apps(app: tauri::AppHandle, members: Vec<MemberId>, apps: Vec<String>) -> Res<()> {
    act(&app, |e, now| e.set_banned_apps(&members, apps, now))
}

#[tauri::command]
fn set_exam(app: tauri::AppHandle, end_ms: Option<u64>, sites: Vec<String>) -> Res<()> {
    act(&app, |e, now| e.set_exam(end_ms.map(|t| (t, sites)), now))
}

#[tauri::command]
fn undo(app: tauri::AppHandle, id: u64) -> Res<()> {
    act(&app, |e, now| e.undo(id, now))
}

#[tauri::command]
fn list_apps(app: tauri::AppHandle, member: MemberId) -> Res<()> {
    act(&app, |e, _| e.list_apps(&member))
}

#[tauri::command]
fn close_app(app: tauri::AppHandle, member: MemberId, id: String) -> Res<()> {
    act(&app, |e, now| e.close_app(&member, &id, now))
}

#[tauri::command]
fn save_site_list(app: tauri::AppHandle, name: String, sites: Vec<String>) -> Res<()> {
    act(&app, |e, _| e.save_site_list(&name, &sites))
}

#[tauri::command]
fn delete_site_list(app: tauri::AppHandle, name: String) -> Res<()> {
    act(&app, |e, _| {
        e.delete_site_list(&name);
        Ok(())
    })
}

#[tauri::command]
fn save_app_list(app: tauri::AppHandle, name: String, apps: Vec<String>) -> Res<()> {
    act(&app, |e, _| e.save_app_list(&name, &apps))
}

#[tauri::command]
fn delete_app_list(app: tauri::AppHandle, name: String) -> Res<()> {
    act(&app, |e, _| {
        e.delete_app_list(&name);
        Ok(())
    })
}

// --- Bibliothèque (MODELE-1 à MODELE-4, SONDAGE-6) --------------------

/// Résultat d'un geste qui rend une valeur, après publication de l'état.
fn act_with<T>(app: &tauri::AppHandle, f: impl FnOnce(&mut Engine, u64) -> anyhow::Result<T>) -> Res<T> {
    let r = f(&mut app.state::<Shared>().engine.lock().unwrap(), now_ms());
    publish(app);
    r.map_err(|e| format!("{e:#}"))
}

#[tauri::command]
fn save_model(app: tauri::AppHandle, model: Model) -> Res<String> {
    act_with(&app, |e, _| e.save_model(model))
}

#[tauri::command]
fn delete_model(app: tauri::AppHandle, id: String) -> Res<()> {
    act(&app, |e, _| {
        e.delete_model(&id);
        Ok(())
    })
}

#[tauri::command]
fn duplicate_model(app: tauri::AppHandle, id: String) -> Res<String> {
    act_with(&app, |e, _| e.duplicate_model(&id))
}

#[tauri::command]
fn save_poll(app: tauri::AppHandle, poll: PollModel) -> Res<String> {
    act_with(&app, |e, _| e.save_poll(poll))
}

#[tauri::command]
fn delete_poll(app: tauri::AppHandle, id: String) -> Res<()> {
    act(&app, |e, _| {
        e.delete_poll(&id);
        Ok(())
    })
}

/// Fichiers choisis par l'enseignant, rangés dans le magasin de la
/// console : à joindre à un modèle ou à distribuer.
#[tauri::command]
fn add_files(app: tauri::AppHandle, paths: Vec<String>) -> Res<Vec<Attachment>> {
    act_with(&app, |e, _| paths.iter().map(|p| e.add_file(std::path::Path::new(p))).collect())
}

/// Export d'un modèle ou d'un sondage en un seul fichier (MODELE-4).
#[tauri::command]
fn export_library(app: tauri::AppHandle, models: Vec<String>, polls: Vec<String>, path: String) -> Res<()> {
    let text = app.state::<Shared>().engine.lock().unwrap().export_library(&models, &polls).map_err(|e| format!("{e:#}"))?;
    std::fs::write(&path, text).map_err(|e| format!("Enregistrement impossible : {e}"))
}

/// Import d'un export : rend le nombre de modèles et de sondages ajoutés.
#[tauri::command]
fn import_library(app: tauri::AppHandle, path: String) -> Res<(usize, usize)> {
    let text = std::fs::read_to_string(&path).map_err(|_| "Ce fichier est illisible.".to_string())?;
    act_with(&app, |e, _| e.import_library(&text))
}

// --- Pédagogie en séance : sans poste désigné, toute la séance ---------

#[tauri::command]
fn launch_tp(app: tauri::AppHandle, members: Vec<MemberId>, model: String) -> Res<()> {
    act(&app, |e, now| e.launch_tp(&members, &model, now))
}

#[tauri::command]
fn stop_tp(app: tauri::AppHandle, members: Vec<MemberId>) -> Res<()> {
    act(&app, |e, now| e.stop_tp(&members, now))
}

#[tauri::command]
fn distribute(app: tauri::AppHandle, members: Vec<MemberId>, files: Vec<Attachment>) -> Res<()> {
    act(&app, |e, now| e.distribute(&members, files, now))
}

/// Fichier glissé sur le plan ou sur une tuile (DISTRIB-1), reçu en
/// mémoire : en-têtes `x-name` (nom, encodé pour une adresse) et
/// `x-members` (postes visés, en JSON).
#[tauri::command]
fn distribute_dropped(app: tauri::AppHandle, request: tauri::ipc::Request<'_>) -> Res<()> {
    let tauri::ipc::InvokeBody::Raw(data) = request.body() else { return Err("Fichier illisible.".into()) };
    let header = |k: &str| request.headers().get(k).and_then(|v| v.to_str().ok()).unwrap_or("").to_string();
    let name = percent_decode(&header("x-name"));
    let members: Vec<MemberId> = serde_json::from_str(&header("x-members")).unwrap_or_default();
    act(&app, |e, now| {
        let f = e.add_file_bytes(&name, data)?;
        e.distribute(&members, vec![f], now)
    })
}

/// Décode « %C3%A9 » en « é ».
fn percent_decode(s: &str) -> String {
    let hex = |c: u8| (c as char).to_digit(16).map(|d| d as u8);
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match (b[i], b.get(i + 1).copied().and_then(hex), b.get(i + 2).copied().and_then(hex)) {
            (b'%', Some(h), Some(l)) => {
                out.push(h << 4 | l);
                i += 3;
            }
            (c, _, _) => {
                out.push(c);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[tauri::command]
fn launch_poll(app: tauri::AppHandle, members: Vec<MemberId>, poll: PollModel, minutes: Option<u32>) -> Res<()> {
    act(&app, |e, now| e.launch_poll(&members, poll, minutes, now))
}

#[tauri::command]
fn close_poll(app: tauri::AppHandle) -> Res<()> {
    act(&app, |e, now| e.close_poll(now))
}

#[tauri::command]
fn show_poll(app: tauri::AppHandle, shown: bool) -> Res<()> {
    act(&app, |e, now| e.show_poll(shown, now))
}

#[tauri::command]
fn clear_poll(app: tauri::AppHandle) -> Res<()> {
    act(&app, |e, _| e.clear_poll())
}

/// Ouvre le dossier des rendus de la séance, ou d'un poste (RENDU-6).
#[tauri::command]
fn open_rendus(app: tauri::AppHandle, member: Option<MemberId>) -> Res<()> {
    let dir = app.state::<Shared>().engine.lock().unwrap().rendus_dir(member.as_deref()).map_err(|e| format!("{e:#}"))?;
    open_path(&dir).map_err(|e| format!("{e:#}"))
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
                        Frame::Binary(b) => match decode_console_frame(&b) {
                            Some((BinKind::Thumbnail, member, jpeg)) => {
                                let data = base64::engine::general_purpose::STANDARD.encode(jpeg);
                                let _ = app.emit("thumb", Thumb { member: member.into(), data });
                            }
                            // Morceau d'un rendu (RENDU-1).
                            Some((BinKind::Chunk, member, payload)) => {
                                app.state::<Shared>().engine.lock().unwrap().handle_chunk(member, payload, now_ms());
                                publish(&app);
                            }
                            None => {}
                        },
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
    let store = match arg("--dossier").map(PathBuf::from) {
        Some(d) => Store::new(d),
        None => Store::with_rendus(surveo_platform::data_dir(Component::Console), surveo_platform::files::documents_dir().join("Surveo").join("Rendus")),
    };
    let dir = store.dir.clone();
    let engine = match Engine::new(store, now_ms()) {
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
            set_pause,
            set_web,
            set_banned_apps,
            set_exam,
            undo,
            list_apps,
            close_app,
            save_site_list,
            delete_site_list,
            save_app_list,
            delete_app_list,
            save_model,
            delete_model,
            duplicate_model,
            save_poll,
            delete_poll,
            add_files,
            export_library,
            import_library,
            launch_tp,
            stop_tp,
            distribute,
            distribute_dropped,
            launch_poll,
            close_poll,
            show_poll,
            clear_poll,
            open_rendus,
            export_package,
            package_file_name
        ])
        .run(tauri::generate_context!())
        .expect("échec du lancement de la console");
}
