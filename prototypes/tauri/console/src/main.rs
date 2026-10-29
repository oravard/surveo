//! Prototype 4 de la phase 2 (IMPL.md) : console Tauri qui embarque le
//! relais du prototype 1 (SPEC.md, RELAIS-1 : relais dans la classe) et
//! affiche le plan de salle alimenté par les agents.
//! Code jetable : il sert à vérifier, pas à être repris tel quel.
//!
//! Usage : surveo-proto-console [--keys DOSSIER] [--port 8443]

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use base64::Engine;
use serde_json::json;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use surveo_proto_relais::arg;
use surveo_proto_relais::relay_core::{Relay, RelayEvents};
use tauri::{Emitter, Manager};

/// Événements du relais transmis à l'interface.
struct Events {
    app: tauri::AppHandle,
}

impl RelayEvents for Events {
    fn agent_joined(&self, member: &str) {
        let _ = self.app.emit("agent", json!({ "member": member, "online": true }));
    }
    fn agent_left(&self, member: &str) {
        let _ = self.app.emit("agent", json!({ "member": member, "online": false }));
    }
    fn thumbnail(&self, member: &str, jpeg: &[u8]) {
        let data = base64::engine::general_purpose::STANDARD.encode(jpeg);
        let _ = self.app.emit("thumb", json!({ "member": member, "data": data }));
    }
    fn ack(&self, id: u64, agent: &str) {
        let _ = self.app.emit("ack", json!({ "id": id, "agent": agent }));
    }
    fn help(&self, agent: &str, active: bool) {
        let _ = self.app.emit("help", json!({ "member": agent, "active": active }));
    }
    fn denied(&self, who: &str, reason: &str) {
        let _ = self.app.emit("denied", json!({ "who": who, "reason": reason }));
    }
}

#[derive(Default)]
struct AppState {
    relay: Mutex<Option<Relay>>,
    next_id: AtomicU64,
    status: Mutex<serde_json::Value>,
}

/// Envoie une commande aux postes (tous si `target` est vide).
#[tauri::command]
fn send(state: tauri::State<'_, AppState>, action: String, text: Option<String>, target: Option<Vec<String>>) -> Result<u64, String> {
    let relay = state.relay.lock().unwrap();
    let relay = relay.as_ref().ok_or("le relais n'est pas démarré")?;
    let id = state.next_id.fetch_add(1, Ordering::Relaxed) + 1;
    let target = target.filter(|t| !t.is_empty());
    relay.command(id, &action, text, target);
    Ok(id)
}

/// État du relais intégré (l'interface le demande à son chargement).
#[tauri::command]
fn relay_status(state: tauri::State<'_, AppState>) -> serde_json::Value {
    state.status.lock().unwrap().clone()
}

/// Dossier des clés : --keys, puis SURVEO_KEYS, puis « keys » à côté de
/// l'exécutable, puis celles du prototype 1.
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

fn main() {
    let port: u16 = arg("--port").and_then(|s| s.parse().ok()).unwrap_or(8443);
    tauri::Builder::default()
        .manage(AppState::default())
        .setup(move |app| {
            let handle = app.handle().clone();
            let keys = keys_dir();
            tauri::async_runtime::spawn(async move {
                let status = match Relay::start(&keys, port, Arc::new(Events { app: handle.clone() })).await {
                    Ok(r) => {
                        let s = json!({ "ok": true, "member": r.member(), "port": r.port, "keys": keys.display().to_string() });
                        handle.state::<AppState>().relay.lock().unwrap().replace(r);
                        s
                    }
                    Err(e) => json!({ "ok": false, "error": format!("{e:#}"), "keys": keys.display().to_string() }),
                };
                *handle.state::<AppState>().status.lock().unwrap() = status.clone();
                let _ = handle.emit("relay", status);
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![send, relay_status])
        .run(tauri::generate_context!())
        .expect("échec du lancement de la console");
}
