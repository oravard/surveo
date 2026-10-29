//! Cœur du relais, utilisable seul (binaire `relay`) ou embarqué dans la
//! console (prototype `tauri`, SPEC.md RELAIS-1 : le même relais, quel
//! que soit son emplacement). Les événements utiles à une console locale
//! sont remontés par le trait `RelayEvents`.

use crate::*;
use anyhow::{anyhow, bail, Result};
use ed25519_dalek::{Signer, Verifier};
use futures_util::{SinkExt, StreamExt};
use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;

/// Ce qu'une console embarquée veut savoir du relais.
pub trait RelayEvents: Send + Sync + 'static {
    fn agent_joined(&self, _member: &str) {}
    fn agent_left(&self, _member: &str) {}
    fn thumbnail(&self, _member: &str, _jpeg: &[u8]) {}
    fn ack(&self, _id: u64, _agent: &str) {}
    fn help(&self, _agent: &str, _active: bool) {}
    fn denied(&self, _who: &str, _reason: &str) {}
}

pub struct NoEvents;
impl RelayEvents for NoEvents {}

type Tx = mpsc::UnboundedSender<Message>;

pub struct State {
    me: Identity,
    trust: Trust,
    agents: Mutex<HashMap<String, Tx>>,
    consoles: Mutex<HashMap<String, Tx>>,
    events: Arc<dyn RelayEvents>,
    pub frames: AtomicU64,
    pub bytes: AtomicU64,
    pub denied: AtomicU64,
}

impl State {
    fn to_consoles(&self, m: &Message) {
        for tx in self.consoles.lock().unwrap().values() {
            let _ = tx.send(m.clone());
        }
    }
    fn to_agents(&self, m: &Message, target: Option<&[String]>) {
        for (member, tx) in self.agents.lock().unwrap().iter() {
            if target.is_none_or(|t| t.iter().any(|x| x == member)) {
                let _ = tx.send(m.clone());
            }
        }
    }
    fn presence(&self) {
        let n = self.agents.lock().unwrap().len();
        let m = Message::Text(serde_json::to_string(&Ctl::Presence { agents: n }).unwrap().into());
        self.to_consoles(&m);
    }
    pub fn agent_count(&self) -> usize {
        self.agents.lock().unwrap().len()
    }
    pub fn console_count(&self) -> usize {
        self.consoles.lock().unwrap().len()
    }
}

/// Relais en fonctionnement.
#[derive(Clone)]
pub struct Relay {
    pub state: Arc<State>,
    pub port: u16,
}

impl Relay {
    /// Démarre le relais : écoute, authentification, annonce sur le réseau local.
    pub async fn start(keys: &Path, port: u16, events: Arc<dyn RelayEvents>) -> Result<Relay> {
        let state = Arc::new(State {
            me: Identity::from_file(&load_json(&keys.join("relay.json"))?)?,
            trust: Trust::from_file(&load_json(&keys.join("etab.json"))?)?,
            agents: Mutex::new(HashMap::new()),
            consoles: Mutex::new(HashMap::new()),
            events,
            frames: AtomicU64::new(0),
            bytes: AtomicU64::new(0),
            denied: AtomicU64::new(0),
        });
        let tls = server_tls()?;
        let listener = TcpListener::bind(("0.0.0.0", port)).await?;
        let port = listener.local_addr()?.port();
        tokio::spawn(announce(state.trust.etab_id.clone(), port));
        let st = state.clone();
        tokio::spawn(async move {
            loop {
                let Ok((tcp, peer)) = listener.accept().await else { continue };
                let (st, tls) = (st.clone(), tls.clone());
                tokio::spawn(async move {
                    if let Err(e) = handle(st.clone(), tls, tcp).await {
                        if e.to_string().starts_with("refusé") {
                            st.denied.fetch_add(1, Ordering::Relaxed);
                            st.events.denied(&peer.to_string(), &e.to_string());
                            println!("Connexion de {peer} {e}");
                        }
                    }
                });
            }
        });
        Ok(Relay { state, port })
    }

    pub fn member(&self) -> &str {
        &self.state.me.cert.member
    }

    /// Commande d'une console embarquée vers des agents (tous si `target` est vide).
    pub fn command(&self, id: u64, action: &str, text: Option<String>, target: Option<Vec<String>>) {
        let c = Ctl::Command { id, action: action.into(), text, target: target.clone() };
        let m = Message::Text(serde_json::to_string(&c).unwrap().into());
        self.state.to_agents(&m, target.as_deref());
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
            state.events.agent_joined(&member);
        }
        Role::Console => {
            state.consoles.lock().unwrap().insert(member.clone(), tx);
        }
        Role::Relay => unreachable!(),
    }
    state.presence();

    while let Some(msg) = stream.next().await {
        let Ok(msg) = msg else { break };
        match (cert.role, msg) {
            (Role::Agent, Message::Binary(b)) => {
                state.frames.fetch_add(1, Ordering::Relaxed);
                state.bytes.fetch_add(b.len() as u64, Ordering::Relaxed);
                state.events.thumbnail(&member, &b);
                let id = member.as_bytes();
                let mut out = Vec::with_capacity(1 + id.len() + b.len());
                out.push(id.len() as u8);
                out.extend_from_slice(id);
                out.extend_from_slice(&b);
                state.to_consoles(&Message::Binary(out.into()));
            }
            (Role::Agent, m @ Message::Text(_)) => {
                if let Message::Text(t) = &m {
                    match serde_json::from_str::<Ctl>(t.as_str()) {
                        Ok(Ctl::Ack { id, agent }) => state.events.ack(id, &agent),
                        Ok(Ctl::Help { active, .. }) => state.events.help(&member, active),
                        _ => {}
                    }
                }
                state.to_consoles(&m);
            }
            (Role::Console, Message::Text(t)) => {
                let target = match serde_json::from_str::<Ctl>(t.as_str()) {
                    Ok(Ctl::Command { target, .. }) => target,
                    _ => None,
                };
                state.to_agents(&Message::Text(t), target.as_deref());
            }
            (_, Message::Close(_)) => break,
            _ => {}
        }
    }

    match cert.role {
        Role::Agent => {
            state.agents.lock().unwrap().remove(&member);
            state.events.agent_left(&member);
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
