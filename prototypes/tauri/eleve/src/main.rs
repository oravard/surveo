//! Prototype 4 de la phase 2 (IMPL.md) : panneau élève affiché dans une
//! vue web, côté agent (SPEC.md, AGENT-3). L'agent se connecte au relais
//! comme un poste, envoie une miniature par seconde, affiche les messages
//! et transmet les demandes d'aide.
//! Code jetable : il sert à vérifier, pas à être repris tel quel.
//!
//! Usage : surveo-proto-eleve [--keys DOSSIER] [--poste 31] [--relay ADRESSE:PORT]

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;
use surveo_proto_relais::*;
use tauri::{Emitter, Manager};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;

#[derive(Default)]
struct AppState {
    tx: Mutex<Option<mpsc::UnboundedSender<Message>>>,
    member: Mutex<String>,
    status: Mutex<serde_json::Value>,
}

/// Demande d'aide de l'élève, ou son annulation (SPEC.md, AIDE-1, AIDE-3).
#[tauri::command]
fn help(state: tauri::State<'_, AppState>, active: bool) -> Result<(), String> {
    let agent = state.member.lock().unwrap().clone();
    let tx = state.tx.lock().unwrap();
    let tx = tx.as_ref().ok_or("pas connecté au relais")?;
    let m = serde_json::to_string(&Ctl::Help { agent, active }).map_err(|e| e.to_string())?;
    tx.send(Message::Text(m.into())).map_err(|e| e.to_string())
}

#[tauri::command]
fn status(state: tauri::State<'_, AppState>) -> serde_json::Value {
    state.status.lock().unwrap().clone()
}

fn keys_dir() -> PathBuf {
    if let Some(k) = arg("--keys").or_else(|| std::env::var("SURVEO_KEYS").ok()) {
        return PathBuf::from(k);
    }
    let beside = std::env::current_exe().ok().and_then(|e| e.parent().map(|p| p.join("keys")));
    for c in [beside, Some(PathBuf::from("keys")), Some(PathBuf::from("../../relais/keys"))].into_iter().flatten() {
        if c.join("etab.json").exists() {
            return c;
        }
    }
    PathBuf::from("keys")
}

fn set_status(app: &tauri::AppHandle, v: serde_json::Value) {
    *app.state::<AppState>().status.lock().unwrap() = v.clone();
    let _ = app.emit("state", v);
}

async fn agent_loop(app: tauri::AppHandle) {
    let keys = keys_dir();
    let poste: usize = arg("--poste").and_then(|s| s.parse().ok()).unwrap_or(31);
    let setup = (|| -> anyhow::Result<(Trust, Identity)> {
        let trust = Trust::from_file(&load_json(&keys.join("etab.json"))?)?;
        let ids: Vec<IdentityFile> = load_json(&keys.join("agents.json"))?;
        let f = ids.get(poste - 1).ok_or_else(|| anyhow::anyhow!("pas d'identité pour le poste {poste}"))?;
        Ok((trust, Identity::from_file(f)?))
    })();
    let (trust, me) = match setup {
        Ok(x) => x,
        Err(e) => return set_status(&app, json!({ "connected": false, "error": format!("{e:#} (clés : {})", keys.display()) })),
    };
    let member = me.cert.member.clone();
    *app.state::<AppState>().member.lock().unwrap() = member.clone();
    let tls = match client_tls() {
        Ok(t) => t,
        Err(e) => return set_status(&app, json!({ "connected": false, "error": e.to_string() })),
    };

    loop {
        set_status(&app, json!({ "connected": false, "member": member, "info": "recherche du relais…" }));
        let addr = match arg("--relay") {
            Some(a) => Some(a),
            None => discover(Duration::from_secs(6)).await.ok().or_else(|| Some("127.0.0.1:8443".into())),
        };
        let Some(addr) = addr else { continue };
        match connect(&addr, &tls, &me, &trust).await {
            Ok(ws) => {
                set_status(&app, json!({ "connected": true, "member": member, "relay": addr }));
                run(&app, ws, &member, poste).await;
            }
            Err(e) => set_status(&app, json!({ "connected": false, "member": member, "error": format!("{e:#}") })),
        }
        *app.state::<AppState>().tx.lock().unwrap() = None;
        tokio::time::sleep(Duration::from_secs(3)).await;
    }
}

async fn run(app: &tauri::AppHandle, ws: ClientWs, member: &str, poste: usize) {
    let (mut sink, mut stream) = ws.split();
    let (tx, mut rx) = mpsc::unbounded_channel::<Message>();
    *app.state::<AppState>().tx.lock().unwrap() = Some(tx);
    let mut tick = tokio::time::interval(Duration::from_secs(1));
    let mut frame = 0u64;
    loop {
        tokio::select! {
            _ = tick.tick() => {
                frame += 1;
                if sink.send(Message::Binary(synthetic_jpeg(poste - 1, frame).into())).await.is_err() { break; }
            }
            Some(m) = rx.recv() => {
                if sink.send(m).await.is_err() { break; }
            }
            msg = stream.next() => {
                let Some(Ok(Message::Text(t))) = msg else {
                    if matches!(msg, Some(Ok(_))) { continue; }
                    break;
                };
                if let Ok(Ctl::Command { id, action, text, .. }) = serde_json::from_str::<Ctl>(t.as_str()) {
                    let _ = app.emit(&action, json!({ "text": text }));
                    let ack = serde_json::to_string(&Ctl::Ack { id, agent: member.to_string() }).unwrap();
                    if sink.send(Message::Text(ack.into())).await.is_err() { break; }
                }
            }
        }
    }
}

fn main() {
    tauri::Builder::default()
        .manage(AppState::default())
        .setup(|app| {
            tauri::async_runtime::spawn(agent_loop(app.handle().clone()));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![help, status])
        .run(tauri::generate_context!())
        .expect("échec du lancement du panneau élève");
}
