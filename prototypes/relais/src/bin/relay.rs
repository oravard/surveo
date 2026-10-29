//! Relais du prototype : accepte agents et consoles en sortie, les
//! authentifie par la clé d'établissement, relaie les miniatures vers
//! les consoles et les commandes vers les agents. Il s'annonce sur le
//! réseau local et affiche sa consommation toutes les 5 s.
//!
//! Usage : relay [--keys keys] [--port 8443]

use anyhow::{anyhow, bail, Result};
use ed25519_dalek::{Signer, Verifier};
use futures_util::{SinkExt, StreamExt};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use surveo_proto_relais::*;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;

type Tx = mpsc::UnboundedSender<Message>;

struct State {
    me: Identity,
    trust: Trust,
    agents: Mutex<HashMap<String, Tx>>,
    consoles: Mutex<HashMap<String, Tx>>,
    frames: AtomicU64,
    bytes: AtomicU64,
    denied: AtomicU64,
}

impl State {
    fn to_consoles(&self, m: &Message) {
        for tx in self.consoles.lock().unwrap().values() {
            let _ = tx.send(m.clone());
        }
    }
    fn to_agents(&self, m: &Message) {
        for tx in self.agents.lock().unwrap().values() {
            let _ = tx.send(m.clone());
        }
    }
    fn presence(&self) {
        let n = self.agents.lock().unwrap().len();
        let m = Message::Text(serde_json::to_string(&Ctl::Presence { agents: n }).unwrap().into());
        self.to_consoles(&m);
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let keys = PathBuf::from(arg("--keys").unwrap_or_else(|| "keys".into()));
    let port: u16 = arg("--port").map(|s| s.parse()).transpose()?.unwrap_or(8443);
    let state = Arc::new(State {
        me: Identity::from_file(&load_json(&keys.join("relay.json"))?)?,
        trust: Trust::from_file(&load_json(&keys.join("etab.json"))?)?,
        agents: Mutex::new(HashMap::new()),
        consoles: Mutex::new(HashMap::new()),
        frames: AtomicU64::new(0),
        bytes: AtomicU64::new(0),
        denied: AtomicU64::new(0),
    });
    let tls = server_tls()?;
    let listener = TcpListener::bind(("0.0.0.0", port)).await?;
    println!("Relais « {} » à l'écoute sur le port {port}", state.me.cert.member);

    tokio::spawn(announce(state.trust.etab_id.clone(), port));
    tokio::spawn(report(state.clone()));

    loop {
        let (tcp, peer) = listener.accept().await?;
        let (state, tls) = (state.clone(), tls.clone());
        tokio::spawn(async move {
            if let Err(e) = handle(state.clone(), tls, tcp).await {
                if e.to_string().starts_with("refusé") {
                    state.denied.fetch_add(1, Ordering::Relaxed);
                    println!("Connexion de {peer} {e}");
                }
            }
        });
    }
}

async fn handle(state: Arc<State>, tls: tokio_rustls::TlsAcceptor, tcp: TcpStream) -> Result<()> {
    tcp.set_nodelay(true)?;
    let tls_stream = tls.accept(tcp).await?;
    let ekm: [u8; 32] = tls_stream.get_ref().1.export_keying_material([0u8; 32], EKM_LABEL, None)?;
    let mut ws = tokio_tungstenite::accept_async(tls_stream).await?;

    // Authentification mutuelle (SPEC.md, CONF-3).
    let my_nonce = new_nonce();
    send_ctl(&mut ws, &Ctl::Hello { cert: state.me.cert.clone(), nonce: my_nonce.clone() }).await?;
    let Ctl::Auth { cert, nonce, sig } = recv_ctl(&mut ws).await? else {
        bail!("le client n'a pas répondu au défi");
    };
    let checked = cert.verify(&state.trust).and_then(|pk| {
        pk.verify(&auth_message("client", &ekm, &my_nonce, &nonce), &sig_from_hex(&sig)?)
            .map_err(|_| anyhow!("signature du client invalide"))?;
        if cert.role == Role::Relay {
            bail!("un relais ne peut pas se connecter comme client");
        }
        Ok(())
    });
    if let Err(e) = checked {
        let _ = send_ctl(&mut ws, &Ctl::Denied { reason: e.to_string() }).await;
        let _ = ws.close(None).await;
        bail!("refusée ({}, {}) : {e}", cert.member, cert.etab);
    }
    let reply = state.me.key.sign(&auth_message("relay", &ekm, &nonce, &my_nonce));
    send_ctl(&mut ws, &Ctl::AuthOk { sig: hex::encode(reply.to_bytes()) }).await?;

    // Après l'authentification : un écrivain par connexion, alimenté par un canal.
    let (mut sink, mut stream) = ws.split();
    let (tx, mut rx) = mpsc::unbounded_channel::<Message>();
    let writer = tokio::spawn(async move {
        while let Some(m) = rx.recv().await {
            if sink.send(m).await.is_err() {
                break;
            }
        }
    });

    let member = cert.member.clone();
    match cert.role {
        Role::Agent => {
            state.agents.lock().unwrap().insert(member.clone(), tx);
            state.presence();
        }
        Role::Console => {
            state.consoles.lock().unwrap().insert(member.clone(), tx);
            state.presence();
        }
        Role::Relay => unreachable!(),
    }

    while let Some(msg) = stream.next().await {
        let Ok(msg) = msg else { break };
        match (cert.role, msg) {
            // Miniature d'un agent : préfixée par l'identifiant du poste, puis
            // envoyée aux consoles (une seule copie des octets, partagée).
            (Role::Agent, Message::Binary(b)) => {
                state.frames.fetch_add(1, Ordering::Relaxed);
                state.bytes.fetch_add(b.len() as u64, Ordering::Relaxed);
                let id = member.as_bytes();
                let mut out = Vec::with_capacity(1 + id.len() + b.len());
                out.push(id.len() as u8);
                out.extend_from_slice(id);
                out.extend_from_slice(&b);
                state.to_consoles(&Message::Binary(out.into()));
            }
            (Role::Agent, m @ Message::Text(_)) => state.to_consoles(&m),
            (Role::Console, m @ Message::Text(_)) => state.to_agents(&m),
            (_, Message::Close(_)) => break,
            _ => {}
        }
    }

    match cert.role {
        Role::Agent => {
            state.agents.lock().unwrap().remove(&member);
        }
        _ => {
            state.consoles.lock().unwrap().remove(&member);
        }
    }
    state.presence();
    writer.abort();
    Ok(())
}

/// Annonce du relais sur le réseau local, toutes les 2 s. L'annonce n'est
/// qu'une indication : la confiance vient de l'authentification (DECOUV-2).
async fn announce(etab: String, port: u16) {
    let Ok(sock) = tokio::net::UdpSocket::bind(("0.0.0.0", 0)).await else { return };
    let _ = sock.set_broadcast(true);
    let msg = serde_json::json!({ "surveo": "relais", "etab": etab, "port": port }).to_string();
    loop {
        let _ = sock.send_to(msg.as_bytes(), ("255.255.255.255", DISCOVERY_PORT)).await;
        let _ = sock.send_to(msg.as_bytes(), ("127.0.0.1", DISCOVERY_PORT)).await;
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
}

/// Consommation du relais toutes les 5 s (Linux : /proc/self).
async fn report(state: Arc<State>) {
    let mut last = (Instant::now(), cpu_ticks(), 0u64, 0u64);
    loop {
        tokio::time::sleep(Duration::from_secs(5)).await;
        let now = (
            Instant::now(),
            cpu_ticks(),
            state.frames.load(Ordering::Relaxed),
            state.bytes.load(Ordering::Relaxed),
        );
        let dt = now.0.duration_since(last.0).as_secs_f64();
        let cpu = (now.1 - last.1) as f64 / 100.0 / dt * 100.0;
        println!(
            "[relais] agents {:>3} · consoles {} · miniatures {:>6.1}/s ({:.2} Mo/s) · CPU {:>5.1} % d'un cœur · mémoire {:.1} Mo · refus {}",
            state.agents.lock().unwrap().len(),
            state.consoles.lock().unwrap().len(),
            (now.2 - last.2) as f64 / dt,
            (now.3 - last.3) as f64 / dt / 1e6,
            cpu,
            rss_mb(),
            state.denied.load(Ordering::Relaxed),
        );
        last = now;
    }
}

fn cpu_ticks() -> u64 {
    let s = std::fs::read_to_string("/proc/self/stat").unwrap_or_default();
    let after = s.rsplit_once(')').map(|x| x.1).unwrap_or("");
    let f: Vec<&str> = after.split_whitespace().collect();
    let get = |i: usize| f.get(i).and_then(|v| v.parse::<u64>().ok()).unwrap_or(0);
    get(11) + get(12)
}

fn rss_mb() -> f64 {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("VmRSS:"))
                .and_then(|l| l.split_whitespace().nth(1)?.parse::<f64>().ok())
        })
        .map(|kb| kb / 1024.0)
        .unwrap_or(0.0)
}
