//! Cœur de l'agent : trouver le relais, rejoindre la séance, envoyer les
//! miniatures, revenir à l'état d'origine (SPEC.md, chap. 1).
//!
//! L'agent est autonome pendant une coupure : il garde la séance et ses
//! restrictions jusqu'à son heure de fin, marge comprise, et la quitte de
//! lui-même ensuite (RESTR-1, RESTR-2). Au démarrage, il est toujours hors
//! séance (RESTR-3, DEMAR-2) ; la console le relie à la séance ouverte de
//! sa salle dès qu'il se connecte (REJ-1), et lui renvoie ses restrictions
//! ([`crate::restrict`]), son TP, ses fichiers et le sondage
//! ([`crate::peda`]).

use crate::peda::{PanelView, Peda};
use crate::restrict::{Restrictor, RestrictionsView};
use crate::setup;
use anyhow::{anyhow, Result};
use futures_util::{SinkExt, StreamExt};
use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use surveo_identity::{Identity, Package, Trust};
use surveo_net::{discovery, handshake, ClientWs, Refused, WsMessage};
use surveo_platform::capture::{self, Capture, CaptureEvent, CaptureOptions};
use surveo_platform::proxy::{SystemProxy, UserProxy};
use surveo_protocol::*;
use tokio::sync::mpsc;
use tokio::time::Instant;

/// Sans nouvelles du relais pendant ce délai, la connexion est tenue pour
/// perdue (le relais envoie un ping toutes les 10 s).
const IDLE_TIMEOUT: Duration = Duration::from_secs(30);
/// Durée d'écoute des réponses à une recherche du relais.
const SEARCH_WAIT: Duration = Duration::from_millis(1500);
const MAX_BACKOFF: Duration = Duration::from_secs(10);
/// Période des vérifications des restrictions : fin du mode examen,
/// applications interdites, réglage du proxy.
const CHECK_PERIOD: Duration = Duration::from_secs(2);

/// Ce que l'indicateur de séance affiche (REJ-4).
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct SessionView {
    pub teacher: String,
    /// Heure de fin prévue, sur l'horloge du poste (ms depuis 1970).
    pub ends_at_ms: u64,
}

/// Affichage côté élève, fourni par l'application.
pub trait Ui: Send + Sync + 'static {
    /// Le poste entre dans une séance, en change l'heure de fin, ou la
    /// quitte (`None`).
    fn session(&self, s: Option<&SessionView>);
    /// Afficher un grand numéro pendant quelques secondes (SALLE-4).
    fn identify(&self, number: &str, seconds: u32);
    /// Restrictions en cours, énoncées par l'indicateur (MAIT-1, EXAM-1).
    fn restrictions(&self, r: &RestrictionsView);
    /// Recouvrir chaque écran avec ce message, ou lever la pause (`None`)
    /// (ECRAN-1, ECRAN-2).
    fn pause(&self, message: Option<&str>) -> Result<(), String>;
    /// Notification discrète à l'élève (AGENT-4) : site bloqué,
    /// application fermée, fichier reçu.
    fn notify(&self, text: &str);
    /// Contenu à jour du panneau élève (AGENT-3) et du sondage au premier
    /// plan (SONDAGE-2).
    fn panel(&self, _v: &PanelView) {}
    /// Ouvrir le panneau élève, sans prendre le premier plan : lancement
    /// d'un TP, consignes d'une étape en retard (MODELE-2), fin du TP.
    fn show_panel(&self) {}
}

pub struct AgentSetup {
    pub dir: PathBuf,
    pub package: Package,
    pub identity: Identity,
    pub info: AgentInfo,
    /// Miniatures de démonstration au lieu de l'écran réel (essais).
    pub synthetic_screen: Option<usize>,
    /// Port de recherche du relais (tests).
    pub discovery_port: u16,
    /// Réglage du proxy de l'utilisateur (simulé pour les essais).
    pub proxy: Arc<dyn UserProxy>,
    /// Dossier Surveo de l'élève, où arrivent les fichiers distribués
    /// (DISTRIB-2).
    pub files_dir: PathBuf,
}

impl AgentSetup {
    pub fn new(dir: PathBuf, package: Package, identity: Identity) -> Self {
        let info = AgentInfo {
            label: surveo_platform::hostname(),
            os: surveo_platform::os_label(),
            version: env!("CARGO_PKG_VERSION").into(),
            kind: package.kind,
            mode: DeployMode::SansAdmin,
        };
        let files_dir = surveo_platform::files::documents_dir().join("Surveo");
        Self { dir, package, identity, info, synthetic_screen: None, discovery_port: DISCOVERY_PORT, proxy: Arc::new(SystemProxy), files_dir }
    }
}

/// Ce que l'élève fait depuis le panneau élève (AGENT-3).
#[derive(Clone)]
pub struct Student(Arc<Mutex<Peda>>);

impl Student {
    pub fn view(&self) -> PanelView {
        self.0.lock().unwrap().view(now_ms())
    }

    pub fn set_step(&self, step: u16, done: bool) {
        self.0.lock().unwrap().set_step(step, done, now_ms());
    }

    /// Dépose un rendu ; rend le refus, en mots simples (RENDU-7, RENDU-8).
    pub fn submit(&self, path: &Path) -> Result<(), String> {
        crate::peda::submit(&self.0, path, now_ms)
    }

    pub fn answer(&self, choices: Vec<u16>) {
        self.0.lock().unwrap().answer(choices, now_ms());
    }

    pub fn dismiss_poll(&self) {
        self.0.lock().unwrap().dismiss_poll(now_ms());
    }

    pub fn file_path(&self, id: &str) -> Option<PathBuf> {
        self.0.lock().unwrap().file_path(id)
    }

    pub fn folder(&self) -> Option<PathBuf> {
        self.0.lock().unwrap().folder()
    }
}

struct Active {
    info: SessionInfo,
    /// Fin prévue plus la marge : la séance se termine d'elle-même.
    deadline: Instant,
    view: SessionView,
}

struct Agent {
    setup: AgentSetup,
    trust: Trust,
    ui: Arc<dyn Ui>,
    session: Option<Active>,
    /// L'enseignant affiche les écrans (AFF-4).
    screens: bool,
    capture: Option<Capture>,
    /// Numéro de la capture en cours, pour écarter les images d'une
    /// capture déjà arrêtée.
    generation: u64,
    screen: ScreenState,
    frames_tx: mpsc::Sender<(u64, CaptureEvent)>,
    frames_rx: mpsc::Receiver<(u64, CaptureEvent)>,
    restrictor: Arc<Mutex<Restrictor>>,
    peda: Arc<Mutex<Peda>>,
    /// Messages du TP, des rendus et du sondage à envoyer.
    peda_rx: mpsc::UnboundedReceiver<AgentMsg>,
    /// Signale un changement de l'état annoncé, trouvé par les
    /// vérifications périodiques ou venu de l'élève.
    changes_rx: mpsc::Receiver<()>,
}

/// Réponse du poste à une commande.
enum Reply {
    Msg(AgentMsg),
    /// Morceau d'un rendu ([`Command::Fetch`]).
    Chunk(Vec<u8>),
}

/// Prépare l'agent : rend la poignée du panneau élève, et la tâche qui
/// fait tourner l'agent indéfiniment.
pub fn prepare(setup: AgentSetup, ui: Arc<dyn Ui>) -> Result<(Student, impl Future<Output = Result<()>> + Send)> {
    let trust = Trust::new(setup.package.etab.clone())?;
    // Retour à l'état d'origine avant toute connexion (DEMAR-2) ; les
    // fichiers distribués d'une séance finie sont retirés (DISTRIB-6).
    crate::restrict::restore_leftovers(&setup.dir, setup.proxy.as_ref());
    let (changes_tx, changes_rx) = mpsc::channel(4);
    let (peda_tx, peda_rx) = mpsc::unbounded_channel();
    let peda = Peda::new(ui.clone(), setup.dir.clone(), setup.files_dir.clone(), setup.info.kind, peda_tx, changes_tx.clone(), now_ms());
    let peda = Arc::new(Mutex::new(peda));
    let restrictor = Arc::new(Mutex::new(Restrictor::new(ui.clone(), setup.dir.clone(), setup.proxy.clone())));
    let student = Student(peda.clone());
    let run = async move {
        let (frames_tx, frames_rx) = mpsc::channel(2);
        {
            let (r, p) = (restrictor.clone(), peda.clone());
            tokio::spawn(async move {
                let mut every = tokio::time::interval(CHECK_PERIOD);
                loop {
                    every.tick().await;
                    let restricted = r.lock().unwrap().tick();
                    let taught = p.lock().unwrap().tick(now_ms());
                    if restricted || taught {
                        let _ = changes_tx.try_send(());
                    }
                }
            });
        }
        let mut agent = Agent {
            setup,
            trust,
            ui,
            session: None,
            screens: false,
            capture: None,
            generation: 0,
            screen: ScreenState::Off,
            frames_tx,
            frames_rx,
            restrictor,
            peda,
            peda_rx,
            changes_rx,
        };
        agent.run().await
    };
    Ok((student, run))
}

/// Fait tourner l'agent indéfiniment.
pub async fn run(setup: AgentSetup, ui: Arc<dyn Ui>) -> Result<()> {
    prepare(setup, ui)?.1.await
}

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

impl Agent {
    async fn run(&mut self) -> Result<()> {
        let mut backoff = Duration::from_secs(1);
        loop {
            let deadline = self.deadline();
            let attempt = tokio::select! {
                r = self.connect() => r,
                _ = sleep_until(deadline) => { self.leave_session(); continue; }
            };
            match attempt {
                Ok(ws) => {
                    backoff = Duration::from_secs(1);
                    if let Err(e) = self.serve(ws).await {
                        log::info!("connexion au relais perdue : {e:#}");
                    }
                    self.peda.lock().unwrap().set_connected(false, now_ms());
                    self.stop_capture();
                }
                Err(e) => {
                    log::info!("relais non joint : {e:#}");
                    let deadline = self.deadline();
                    tokio::select! {
                        _ = tokio::time::sleep(backoff) => {}
                        _ = sleep_until(deadline) => self.leave_session(),
                    }
                    backoff = (backoff * 2).min(MAX_BACKOFF);
                }
            }
        }
    }

    fn deadline(&self) -> Option<Instant> {
        self.session.as_ref().map(|s| s.deadline)
    }

    /// Chaîne de découverte (DECOUV-1) : dernier relais joint, adresses du
    /// paquet, puis recherche sur le réseau local. Un relais trouvé n'est
    /// suivi que s'il s'authentifie au nom de l'établissement (DECOUV-2).
    async fn connect(&self) -> Result<ClientWs> {
        let dir = &self.setup.dir;
        let mut tried = Vec::new();
        let known = setup::last_relay(dir).into_iter().chain(self.setup.package.relays.iter().cloned());
        for addr in known {
            if tried.contains(&addr) {
                continue;
            }
            if let Some(ws) = self.try_relay(&addr).await? {
                return Ok(ws);
            }
            tried.push(addr);
        }
        let found = discovery::search(self.trust.etab_id(), SEARCH_WAIT, self.setup.discovery_port).await?;
        for addr in found.iter().map(|a| a.to_string()) {
            if !tried.contains(&addr) {
                if let Some(ws) = self.try_relay(&addr).await? {
                    return Ok(ws);
                }
            }
        }
        Err(anyhow!("aucun relais de l'établissement trouvé"))
    }

    async fn try_relay(&self, addr: &str) -> Result<Option<ClientWs>> {
        match handshake::connect(addr, &self.setup.identity, &self.trust, Some(&self.setup.info)).await {
            Ok((ws, _)) => {
                log::info!("connecté au relais {addr}");
                setup::save_last_relay(&self.setup.dir, addr);
                Ok(Some(ws))
            }
            Err(e) => {
                if let Some(r) = e.downcast_ref::<Refused>() {
                    log::warn!("relais {addr} : {}", r.0);
                } else {
                    log::debug!("relais {addr} : {e:#}");
                }
                Ok(None)
            }
        }
    }

    async fn serve(&mut self, ws: ClientWs) -> Result<()> {
        let (mut sink, mut stream) = ws.split();
        if self.session.is_some() && self.screens {
            self.start_capture();
        }
        // Ce qui attendait pendant la coupure est reconstitué à neuf.
        while self.peda_rx.try_recv().is_ok() {}
        send(&mut sink, &self.status()).await?;
        let resume = self.peda.lock().unwrap().set_connected(true, now_ms());
        for m in resume {
            send(&mut sink, &m).await?;
        }
        loop {
            let deadline = self.deadline();
            tokio::select! {
                next = tokio::time::timeout(IDLE_TIMEOUT, stream.next()) => {
                    let msg = match next {
                        Ok(Some(Ok(m))) => m,
                        Ok(Some(Err(e))) => return Err(e.into()),
                        Ok(None) => return Err(anyhow!("connexion fermée par le relais")),
                        Err(_) => return Err(anyhow!("plus de nouvelles du relais")),
                    };
                    let t = match msg {
                        WsMessage::Text(t) => t,
                        WsMessage::Binary(b) => {
                            // Morceau d'un fichier distribué (DISTRIB-4).
                            if let Some((BinKind::Chunk, payload)) = decode_agent_frame(&b) {
                                if let Some((file, offset, data)) = decode_chunk(payload) {
                                    self.peda.lock().unwrap().chunk(file, offset, data, now_ms());
                                }
                            }
                            continue;
                        }
                        _ => continue,
                    };
                    let Ok(ToAgent::Command { id, cmd, .. }) = serde_json::from_str::<ToAgent>(t.as_str()) else {
                        log::debug!("message illisible : {t}");
                        continue;
                    };
                    let before = self.status();
                    match self.handle(id, cmd) {
                        Reply::Msg(m) => send(&mut sink, &m).await?,
                        Reply::Chunk(c) => sink.send(WsMessage::Binary(encode_agent_frame(BinKind::Chunk, &c).into())).await?,
                    }
                    let after = self.status();
                    if after != before {
                        send(&mut sink, &after).await?;
                    }
                }
                Some((generation, ev)) = self.frames_rx.recv() => {
                    if generation != self.generation || self.capture.is_none() {
                        continue;
                    }
                    match ev {
                        CaptureEvent::Frame(jpeg) => {
                            if self.screen != ScreenState::On {
                                self.screen = ScreenState::On;
                                send(&mut sink, &self.status()).await?;
                            }
                            sink.send(WsMessage::Binary(encode_agent_frame(BinKind::Thumbnail, &jpeg).into())).await?;
                        }
                        CaptureEvent::Failed(reason) => {
                            self.capture = None;
                            self.screen = ScreenState::Unavailable { reason };
                            send(&mut sink, &self.status()).await?;
                        }
                    }
                }
                _ = sleep_until(deadline) => {
                    self.leave_session();
                    send(&mut sink, &self.status()).await?;
                }
                Some(()) = self.changes_rx.recv() => {
                    send(&mut sink, &self.status()).await?;
                }
                Some(m) = self.peda_rx.recv() => {
                    send(&mut sink, &m).await?;
                }
            }
        }
    }

    fn status(&self) -> AgentMsg {
        let p = self.peda.lock().unwrap();
        AgentMsg::Status {
            session: self.session.as_ref().map(|s| s.info.id.clone()),
            screen: self.screen.clone(),
            web: self.restrictor.lock().unwrap().web_state(),
            tp: p.progress(now_ms()),
            files: p.received_ids(),
        }
    }

    fn in_session(&self, session: &str) -> bool {
        self.session.as_ref().is_some_and(|s| s.info.id == session)
    }

    /// Exécute une commande ; rend la réponse, en général l'acquittement
    /// (RETOUR-1).
    fn handle(&mut self, id: u64, cmd: Command) -> Reply {
        let (ok, detail) = match cmd {
            Command::ListApps => return Reply::Msg(AgentMsg::Apps { id, apps: self.restrictor.lock().unwrap().list_apps() }),
            Command::Fetch { file, offset, len } => match self.peda.lock().unwrap().rendu_chunk(&file, offset, len, now_ms()) {
                Some(c) => return Reply::Chunk(c),
                None => (false, Some("rendu inconnu".into())),
            },
            cmd => self.execute(cmd),
        };
        Reply::Msg(AgentMsg::Ack { id, ok, detail })
    }

    fn execute(&mut self, cmd: Command) -> (bool, Option<String>) {
        match cmd {
            Command::Session { session } => {
                self.join_session(session);
                (true, None)
            }
            Command::EndSession { session } => {
                if self.session.as_ref().is_some_and(|s| s.info.id == session) {
                    self.leave_session();
                }
                (true, None)
            }
            Command::Screens { on } => {
                if self.session.is_none() {
                    return (false, Some("hors séance".into()));
                }
                self.screens = on;
                if on && self.capture.is_none() {
                    self.start_capture();
                } else if !on {
                    self.stop_capture();
                }
                (true, None)
            }
            Command::Identify { number, seconds } => {
                self.ui.identify(&number, seconds.clamp(1, 30));
                (true, None)
            }
            Command::Restrict { session, restrictions } => {
                if self.session.as_ref().is_none_or(|s| s.info.id != session) {
                    return (false, Some("hors séance".into()));
                }
                // Sur un poste personnel, Surveo observe et signale
                // seulement (PERSO-3, EXAM-5).
                if self.setup.info.kind == PosteKind::Personnel {
                    let any = restrictions != Restrictions::default();
                    return (!any, any.then(|| "poste personnel : aucune restriction ne s'y applique".into()));
                }
                let errors = self.restrictor.lock().unwrap().apply(restrictions);
                (errors.is_empty(), (!errors.is_empty()).then(|| errors.join(" · ")))
            }
            Command::CloseApp { app } => {
                if self.session.is_none() {
                    return (false, Some("hors séance".into()));
                }
                match self.restrictor.lock().unwrap().close_app(&app) {
                    Ok(()) => (true, None),
                    Err(e) => (false, Some(e)),
                }
            }
            Command::Tp { session, tp } if self.in_session(&session) => {
                self.peda.lock().unwrap().set_tp(tp, now_ms());
                (true, None)
            }
            Command::Files { session, files } if self.in_session(&session) => {
                self.peda.lock().unwrap().set_files(files, now_ms());
                (true, None)
            }
            Command::Poll { session, poll } if self.in_session(&session) => {
                self.peda.lock().unwrap().set_poll(poll, now_ms());
                (true, None)
            }
            Command::Tp { .. } | Command::Files { .. } | Command::Poll { .. } => (false, Some("hors séance".into())),
            // Un rendu reçu après la séance est accusé quand même (RENDU-3).
            Command::Receipt { rendu, error } => {
                self.peda.lock().unwrap().receipt(&rendu, error, now_ms());
                (true, None)
            }
            Command::ListApps | Command::Fetch { .. } => unreachable!("traitée par handle"),
        }
    }

    /// Entrée dans la séance (REJ-1), ou mise à jour de son heure de fin
    /// (SEANCE-3) et resynchronisation après une coupure (RESTR-2).
    fn join_session(&mut self, info: SessionInfo) {
        if self.session.as_ref().is_some_and(|s| s.info.id != info.id) {
            self.leave_session();
        }
        let remaining = Duration::from_millis(info.remaining_ms);
        let view = SessionView { teacher: info.teacher.clone(), ends_at_ms: now_ms() + info.remaining_ms };
        let deadline = Instant::now() + remaining + Duration::from_millis(info.margin_ms);
        let changed = self.session.as_ref().is_none_or(|s| s.view.teacher != view.teacher || s.view.ends_at_ms.abs_diff(view.ends_at_ms) > 30_000);
        if self.session.is_none() {
            log::info!("séance {} rejointe, fin prévue dans {} min", info.id, remaining.as_secs() / 60);
        }
        let view = match &self.session {
            // Même heure de fin à quelques secondes près : l'affichage ne bouge pas.
            Some(s) if !changed => s.view.clone(),
            _ => view,
        };
        if changed {
            self.ui.session(Some(&view));
        }
        self.restrictor.lock().unwrap().begin(&info.teacher);
        self.peda.lock().unwrap().begin(&info, now_ms());
        self.session = Some(Active { info, deadline, view });
    }

    /// Fin de la séance, sur ordre de la console (SEANCE-5) ou à
    /// l'échéance (SEANCE-6) : toutes les restrictions sont levées et le
    /// poste revient à son état d'origine.
    fn leave_session(&mut self) {
        self.restrictor.lock().unwrap().clear();
        self.peda.lock().unwrap().end(now_ms());
        if let Some(s) = self.session.take() {
            log::info!("séance {} quittée : poste dans son état d'origine", s.info.id);
        }
        self.screens = false;
        self.stop_capture();
        self.ui.session(None);
    }

    fn start_capture(&mut self) {
        self.generation += 1;
        let (tx, generation) = (self.frames_tx.clone(), self.generation);
        let mut opts = CaptureOptions::thumbnails(self.setup.dir.join(setup::TOKEN_FILE));
        opts.synthetic = self.setup.synthetic_screen;
        self.screen = ScreenState::Starting;
        self.capture = Some(capture::start(
            opts,
            Arc::new(move |ev| {
                // Une image en retard est perdue plutôt que d'encombrer la file.
                let _ = tx.try_send((generation, ev));
            }),
        ));
    }

    fn stop_capture(&mut self) {
        self.capture = None;
        self.screen = ScreenState::Off;
    }
}

async fn send<S>(sink: &mut S, m: &AgentMsg) -> Result<()>
where
    S: futures_util::Sink<WsMessage> + Unpin,
    S::Error: std::error::Error + Send + Sync + 'static,
{
    sink.send(WsMessage::Text(serde_json::to_string(m)?.into())).await?;
    Ok(())
}

async fn sleep_until(deadline: Option<Instant>) {
    match deadline {
        Some(d) => tokio::time::sleep_until(d).await,
        None => std::future::pending().await,
    }
}
