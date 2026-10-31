//! Moteur du relais (SPEC.md, chap. 7 ; IMPL.md, « Architecture »).
//!
//! Le même moteur sert aux trois emplacements du relais (RELAIS-1,
//! RELAIS-2) ; dans la console, celle-ci s'y attache directement
//! ([`Relay::attach_console`]) au lieu de passer par le réseau.
//!
//! Le relais est mince : il authentifie (CONF-3), route entre membres de
//! son établissement (CONF-4) et ne garde rien de durable. Ses files sont
//! bornées : pour les miniatures, seule la plus récente de chaque poste
//! attend d'être remise à une console lente.

use anyhow::Result;
use bytes::Bytes;
use futures_util::{SinkExt, StreamExt};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use surveo_identity::{Identity, Role, Trust};
use surveo_net::{discovery, handshake, Refused, WsMessage};
use surveo_protocol::*;
use tokio::net::TcpListener;
use tokio::sync::{mpsc, Notify};
use tokio_util::sync::CancellationToken;

/// Intervalle des ping du relais vers chaque client.
pub const PING_INTERVAL: Duration = Duration::from_secs(10);
/// Un client dont rien n'est reçu pendant ce délai est tenu pour parti.
pub const IDLE_TIMEOUT: Duration = Duration::from_secs(30);
/// Profondeur de la file de messages de contrôle de chaque client.
const CONTROL_QUEUE: usize = 256;

/// Trame remise à une console embarquée.
#[derive(Debug, Clone)]
pub enum Frame {
    Text(String),
    Binary(Bytes),
}

pub struct RelayConfig {
    pub identity: Identity,
    pub trust: Trust,
    /// Port TCP souhaité ; un port libre est pris s'il est occupé.
    pub port: u16,
    /// Port UDP sur lequel répondre aux recherches des postes du réseau
    /// local ([`DISCOVERY_PORT`]) ; aucun : pas de réponse.
    pub discovery: Option<u16>,
}

/// Relais en fonctionnement.
#[derive(Clone)]
pub struct Relay {
    inner: Arc<Inner>,
    port: u16,
    discovery_error: Option<String>,
}

struct Inner {
    me: Identity,
    trust: Trust,
    peers: Mutex<Peers>,
    next_conn: AtomicU64,
}

#[derive(Default)]
struct Peers {
    agents: HashMap<MemberId, AgentPeer>,
    consoles: HashMap<MemberId, Peer>,
}

struct AgentPeer {
    presence: AgentPresence,
    peer: Peer,
}

/// Une connexion vue du relais.
struct Peer {
    conn: u64,
    out: Outbox,
}

/// Ce qui attend d'être envoyé à un client.
#[derive(Clone)]
struct Outbox {
    control: mpsc::Sender<Frame>,
    thumbs: Arc<ThumbSlot>,
    closed: CancellationToken,
}

/// Dernière miniature de chaque poste, en attente de remise à une console.
#[derive(Default)]
struct ThumbSlot {
    latest: Mutex<HashMap<MemberId, Bytes>>,
    ready: Notify,
}

impl ThumbSlot {
    fn put(&self, member: &str, frame: Bytes) {
        self.latest.lock().unwrap().insert(member.to_string(), frame);
        self.ready.notify_one();
    }

    fn take(&self) -> Vec<Bytes> {
        self.latest.lock().unwrap().drain().map(|(_, f)| f).collect()
    }
}

fn new_outbox() -> (Outbox, mpsc::Receiver<Frame>) {
    let (control, rx) = mpsc::channel(CONTROL_QUEUE);
    (Outbox { control, thumbs: Arc::default(), closed: CancellationToken::new() }, rx)
}

impl Relay {
    /// Démarre le relais : écoute TCP, authentification, réponse aux
    /// recherches du réseau local.
    pub async fn start(cfg: RelayConfig) -> Result<Relay> {
        let listener = match TcpListener::bind(("0.0.0.0", cfg.port)).await {
            Ok(l) => l,
            Err(e) if cfg.port != 0 => {
                log::warn!("port {} indisponible ({e}) : port libre choisi", cfg.port);
                TcpListener::bind(("0.0.0.0", 0)).await?
            }
            Err(e) => return Err(e.into()),
        };
        let port = listener.local_addr()?.port();
        let inner = Arc::new(Inner {
            me: cfg.identity,
            trust: cfg.trust,
            peers: Mutex::default(),
            next_conn: AtomicU64::new(1),
        });
        let discovery_error = if let Some(udp) = cfg.discovery {
            match discovery::respond(inner.trust.etab_id().to_string(), port, udp).await {
                Ok(_) => None,
                Err(e) => {
                    log::warn!("recherche locale indisponible : {e:#}");
                    Some(format!("{e:#}"))
                }
            }
        } else {
            None
        };
        let acceptor = surveo_net::tls::server_acceptor()?;
        let st = inner.clone();
        tokio::spawn(async move {
            loop {
                let Ok((tcp, addr)) = listener.accept().await else { continue };
                let (st, acceptor) = (st.clone(), acceptor.clone());
                tokio::spawn(async move {
                    match handshake::accept(tcp, &acceptor, &st.me, &st.trust).await {
                        Ok(a) => st.serve(a).await,
                        Err(e) => {
                            if let Some(r) = e.downcast_ref::<Refused>() {
                                log::warn!("connexion de {addr} refusée : {}", r.0);
                                st.to_consoles(&ToConsole::Refused { peer: addr.ip().to_string(), reason: r.0.clone() });
                            } else {
                                log::debug!("connexion de {addr} interrompue : {e:#}");
                            }
                        }
                    }
                });
            }
        });
        log::info!("relais « {} » à l'écoute sur le port {port}", inner.me.member());
        Ok(Relay { inner, port, discovery_error })
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    /// Motif pour lequel le relais ne répond pas aux recherches locales.
    pub fn discovery_error(&self) -> Option<&str> {
        self.discovery_error.as_deref()
    }

    pub fn agent_count(&self) -> usize {
        self.inner.peers.lock().unwrap().agents.len()
    }

    /// Attache une console embarquée (RELAIS-1, relais dans la classe).
    /// Elle reçoit les mêmes trames qu'une console distante.
    pub fn attach_console(&self, member: &str) -> LocalConsole {
        let (out, rx) = new_outbox();
        let conn = self.inner.register_console(member, out.clone());
        let (tx, frames) = mpsc::channel(CONTROL_QUEUE);
        let st = self.inner.clone();
        let member = member.to_string();
        let sender = LocalSender { inner: self.inner.clone(), member: member.clone() };
        tokio::spawn(async move {
            pump(rx, out, Dest::Local(tx)).await;
            st.unregister_console(&member, conn);
        });
        LocalConsole { sender, frames }
    }
}

/// Console embarquée, attachée au relais sans passer par le réseau.
pub struct LocalConsole {
    pub sender: LocalSender,
    pub frames: mpsc::Receiver<Frame>,
}

/// Envoi d'une console embarquée vers les postes.
#[derive(Clone)]
pub struct LocalSender {
    inner: Arc<Inner>,
    member: MemberId,
}

impl LocalSender {
    pub fn send(&self, m: &FromConsole) {
        self.inner.from_console(&self.member, m);
    }
}

impl Inner {
    async fn serve(self: Arc<Self>, a: handshake::Accepted) {
        let (sink, mut stream) = a.ws.split();
        let (out, rx) = new_outbox();
        let member = a.peer.member.clone();
        let role = a.peer.role;
        let conn = match role {
            Role::Agent => {
                let presence = AgentPresence { member: member.clone(), room: a.peer.room.clone(), info: a.info.expect("poste décrit") };
                self.register_agent(presence, out.clone())
            }
            _ => self.register_console(&member, out.clone()),
        };
        let closed = out.closed.clone();
        let writer = tokio::spawn(pump(rx, out, Dest::Ws(sink)));

        loop {
            let next = tokio::select! {
                _ = closed.cancelled() => break,
                n = tokio::time::timeout(IDLE_TIMEOUT, stream.next()) => n,
            };
            let msg = match next {
                Ok(Some(Ok(m))) => m,
                Ok(_) => break,
                Err(_) => {
                    log::info!("{member} : plus de nouvelles depuis {} s, connexion close", IDLE_TIMEOUT.as_secs());
                    break;
                }
            };
            match (role, msg) {
                (Role::Agent, WsMessage::Binary(b)) => self.from_agent_binary(&member, &b),
                (Role::Agent, WsMessage::Text(t)) => match serde_json::from_str::<AgentMsg>(t.as_str()) {
                    Ok(m) => self.to_consoles(&ToConsole::FromAgent { from: member.clone(), msg: m }),
                    Err(e) => log::debug!("{member} : message illisible ({e})"),
                },
                (Role::Console, WsMessage::Text(t)) => match serde_json::from_str::<FromConsole>(t.as_str()) {
                    Ok(m) => self.from_console(&member, &m),
                    Err(e) => log::debug!("{member} : message illisible ({e})"),
                },
                (_, WsMessage::Close(_)) => break,
                _ => {}
            }
        }
        closed.cancel();
        let _ = writer.await;
        match role {
            Role::Agent => self.unregister_agent(&member, conn),
            _ => self.unregister_console(&member, conn),
        }
    }

    fn register_agent(&self, presence: AgentPresence, out: Outbox) -> u64 {
        let conn = self.next_conn.fetch_add(1, Ordering::Relaxed);
        let member = presence.member.clone();
        let notice = ToConsole::Online { agent: presence.clone() };
        let old = self.peers.lock().unwrap().agents.insert(member.clone(), AgentPeer { presence, peer: Peer { conn, out } });
        // Un poste qui se reconnecte remplace son ancienne connexion.
        if let Some(old) = old {
            old.peer.out.closed.cancel();
        }
        log::info!("poste {member} connecté");
        self.to_consoles(&notice);
        conn
    }

    fn unregister_agent(&self, member: &str, conn: u64) {
        let removed = {
            let mut p = self.peers.lock().unwrap();
            if p.agents.get(member).is_some_and(|a| a.peer.conn == conn) {
                p.agents.remove(member);
                true
            } else {
                false
            }
        };
        if removed {
            log::info!("poste {member} déconnecté");
            self.to_consoles(&ToConsole::Offline { member: member.to_string() });
        }
    }

    fn register_console(&self, member: &str, out: Outbox) -> u64 {
        let conn = self.next_conn.fetch_add(1, Ordering::Relaxed);
        let mut p = self.peers.lock().unwrap();
        let roster = ToConsole::Roster { agents: p.agents.values().map(|a| a.presence.clone()).collect() };
        let _ = out.control.try_send(Frame::Text(serde_json::to_string(&roster).unwrap()));
        if let Some(old) = p.consoles.insert(member.to_string(), Peer { conn, out }) {
            old.out.closed.cancel();
        }
        conn
    }

    fn unregister_console(&self, member: &str, conn: u64) {
        let mut p = self.peers.lock().unwrap();
        if p.consoles.get(member).is_some_and(|c| c.conn == conn) {
            p.consoles.remove(member);
        }
    }

    fn to_consoles(&self, m: &ToConsole) {
        let text = serde_json::to_string(m).unwrap();
        for (member, c) in self.peers.lock().unwrap().consoles.iter() {
            if c.out.control.try_send(Frame::Text(text.clone())).is_err() {
                log::warn!("console {member} saturée : message perdu");
            }
        }
    }

    fn from_console(&self, console: &str, m: &FromConsole) {
        let FromConsole::Send { id, to, cmd } = m;
        let text = serde_json::to_string(&ToAgent::Command { from: console.to_string(), id: *id, cmd: cmd.clone() }).unwrap();
        let peers = self.peers.lock().unwrap();
        let targets = peers.agents.values().filter(|a| match to {
            Target::All => true,
            Target::Room(r) => a.presence.room.as_ref() == Some(r),
            Target::Members(ms) => ms.contains(&a.presence.member),
        });
        for a in targets {
            if a.peer.out.control.try_send(Frame::Text(text.clone())).is_err() {
                log::warn!("poste {} saturé : commande perdue", a.presence.member);
            }
        }
    }

    fn from_agent_binary(&self, member: &str, frame: &[u8]) {
        let Some((kind, payload)) = decode_agent_frame(frame) else { return };
        let out = Bytes::from(encode_console_frame(kind, member, payload));
        for c in self.peers.lock().unwrap().consoles.values() {
            c.out.thumbs.put(member, out.clone());
        }
    }
}

/// Où une connexion remet ses trames.
enum Dest {
    /// Console embarquée.
    Local(mpsc::Sender<Frame>),
    /// Client réseau.
    Ws(futures_util::stream::SplitSink<surveo_net::ServerWs, WsMessage>),
}

impl Dest {
    async fn deliver(&mut self, f: Frame) -> bool {
        match self {
            Dest::Local(tx) => tx.send(f).await.is_ok(),
            Dest::Ws(sink) => {
                let m = match f {
                    Frame::Text(t) => WsMessage::Text(t.into()),
                    Frame::Binary(b) => WsMessage::Binary(b),
                };
                sink.send(m).await.is_ok()
            }
        }
    }

    async fn ping(&mut self) -> bool {
        match self {
            Dest::Local(_) => true,
            Dest::Ws(sink) => sink.send(WsMessage::Ping(Bytes::new())).await.is_ok(),
        }
    }
}

/// Remet à un client ce qui l'attend : messages de contrôle d'abord, puis
/// les dernières miniatures, et un ping régulier pour une connexion
/// réseau. S'arrête quand le client part ou que la remise échoue.
async fn pump(mut rx: mpsc::Receiver<Frame>, out: Outbox, mut dest: Dest) {
    let mut ticker = tokio::time::interval(PING_INTERVAL);
    ticker.tick().await;
    loop {
        let ok = tokio::select! {
            biased;
            _ = out.closed.cancelled() => break,
            f = rx.recv() => match f {
                Some(f) => dest.deliver(f).await,
                None => break,
            },
            _ = out.thumbs.ready.notified() => {
                let mut ok = true;
                for f in out.thumbs.take() {
                    ok = ok && dest.deliver(Frame::Binary(f)).await;
                }
                ok
            }
            _ = ticker.tick() => dest.ping().await,
        };
        if !ok {
            break;
        }
    }
    out.closed.cancel();
}
