//! Pédagogie côté élève (SPEC.md, chap. 3 ; IMPL.md, 3.1) : TP, fichiers
//! reçus, rendus, sondage, présentés par le panneau élève (AGENT-3).
//!
//! L'agent garde sur le poste (`peda.json`) de quoi tenir une coupure ou
//! un redémarrage : son TP et les étapes cochées (TP-3), les rendus
//! déposés et pas encore reçus par l'enseignant (RENDU-3), et la liste des
//! fichiers distribués qu'il a écrits, pour les retirer en fin de séance
//! (DISTRIB-6, DISTRIB-7).
//!
//! Les fichiers vont par morceaux, tirés par le destinataire : l'agent
//! tire les fichiers distribués ([`AgentMsg::Fetch`]), la console tire les
//! rendus ([`Command::Fetch`]). Un morceau perdu est redemandé.

use crate::core::Ui;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use surveo_identity::{load_json, random_id, save_json};
use surveo_platform::files::{self, size_text, Hasher};
use surveo_protocol::*;
use tokio::sync::mpsc;
use ts_rs::TS;

const STATE_FILE: &str = "peda.json";
/// Un fichier dont aucun morceau n'arrive est redemandé au bout de ce
/// délai ; un rendu sans nouvelles de la console est proposé de nouveau.
const FETCH_TIMEOUT_MS: u64 = 10_000;
const OFFER_TIMEOUT_MS: u64 = 30_000;
/// L'élève est prévenu de ce délai avant la fin de la séance que les
/// fichiers reçus seront retirés (DISTRIB-8).
const END_WARNING_MS: u64 = 5 * 60_000;

#[derive(Serialize, Deserialize, Clone, Debug)]
struct SavedSession {
    id: SessionId,
    teacher: String,
    /// Fin prévue, et fin d'elle-même (marge comprise), sur l'horloge du
    /// poste.
    ends_at_ms: u64,
    deadline_ms: u64,
    /// Dossier des fichiers reçus pendant cette séance (DISTRIB-2).
    folder: Option<PathBuf>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct SavedTp {
    info: TpInfo,
    done: BTreeSet<u16>,
    /// Depuis quand l'étape en cours l'est (TP-5, TP-7).
    since_ms: u64,
    /// Étape dont l'action de dépassement a déjà eu lieu.
    fired: Option<u16>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct SavedRendu {
    offer: RenduOffer,
    deposited_ms: u64,
    received_ms: Option<u64>,
    error: Option<String>,
}

/// Fichier distribué écrit dans le dossier de l'élève : noté avant
/// d'être écrit, pour être retiré en fin de séance.
#[derive(Serialize, Deserialize, Clone, Debug)]
struct ReceivedFile {
    id: String,
    name: String,
    path: PathBuf,
}

#[derive(Serialize, Deserialize, Default)]
struct Saved {
    session: Option<SavedSession>,
    tp: Option<SavedTp>,
    rendus: Vec<SavedRendu>,
    received: Vec<ReceivedFile>,
}

struct Download {
    offer: FileOffer,
    part: PathBuf,
    received: u64,
    hasher: Hasher,
    asked_ms: u64,
}

struct PollState {
    info: PollInfo,
    answer: Option<Vec<u16>>,
    closes_at_ms: Option<u64>,
    /// L'élève a remis sa réponse à plus tard : la question reste dans le
    /// panneau.
    dismissed: bool,
}

/// Fin du TP, annoncée dans le panneau (TP-9).
#[derive(Clone, Debug, PartialEq, Eq)]
enum Ended {
    Session,
    Stopped(String),
}

// --- Ce que le panneau élève affiche (types TypeScript engendrés) -------

#[derive(Serialize, Clone, Debug, PartialEq, Eq, TS)]
#[ts(export)]
pub struct PanelView {
    pub teacher: String,
    /// Fin prévue de la séance ; aucune : hors séance.
    #[ts(type = "number | null")]
    pub ends_at_ms: Option<u64>,
    pub connected: bool,
    pub tp: Option<PanelTp>,
    pub ended: Option<PanelEnded>,
    pub rendus: Vec<PanelRendu>,
    /// La zone de dépôt est ouverte (RENDU-8).
    pub can_submit: bool,
    /// « 50 Mo » (RENDU-7).
    pub limit: String,
    /// Dernier dépôt refusé, expliqué à l'élève.
    pub error: Option<String>,
    /// Fichiers distribués hors TP (DISTRIB-3).
    pub files: Vec<PanelFile>,
    /// Les fichiers reçus seront retirés en fin de séance (DISTRIB-6).
    pub removed_at_end: bool,
    pub poll: Option<PanelPoll>,
}

#[derive(Serialize, Clone, Debug, PartialEq, Eq, TS)]
#[ts(export)]
pub struct PanelTp {
    pub title: String,
    pub steps: Vec<PanelStep>,
    /// Étape en cours : la première non terminée (TP-6).
    pub current: Option<u32>,
    #[ts(type = "number")]
    pub current_since_ms: u64,
    pub late: bool,
    pub expected: u32,
    /// Rendus reçus par l'enseignant, un même nom comptant une fois.
    pub received: u32,
    pub files: Vec<PanelFile>,
}

#[derive(Serialize, Clone, Debug, PartialEq, Eq, TS)]
#[ts(export)]
pub struct PanelStep {
    pub objective: String,
    pub instructions: String,
    pub minutes: u32,
    pub done: bool,
}

#[derive(Serialize, Clone, Debug, PartialEq, Eq, TS)]
#[ts(export)]
pub struct PanelFile {
    pub id: String,
    pub name: String,
    pub size: String,
    /// Reçu et rangé dans le dossier de l'élève.
    pub received: bool,
}

#[derive(Serialize, Clone, Debug, PartialEq, Eq, TS)]
#[ts(export)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum PanelEnded {
    /// Fin de la séance ; `pending` rendus attendent encore la connexion.
    Session { pending: u32 },
    /// TP arrêté par l'enseignant.
    Stopped { title: String, pending: u32 },
}

#[derive(Serialize, Clone, Debug, PartialEq, Eq, TS)]
#[ts(export)]
pub struct PanelRendu {
    pub id: String,
    pub name: String,
    pub state: RenduState,
}

#[derive(Serialize, Clone, Debug, PartialEq, Eq, TS)]
#[ts(export)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum RenduState {
    /// Connexion interrompue : il partira à son retour (RENDU-3).
    Waiting,
    Sending,
    /// Reçu par l'enseignant (RENDU-2).
    Received {
        #[ts(type = "number")]
        at_ms: u64,
    },
    Refused { reason: String },
}

#[derive(Serialize, Clone, Debug, PartialEq, Eq, TS)]
#[ts(export)]
pub struct PanelPoll {
    pub id: String,
    pub question: String,
    pub options: Vec<String>,
    pub multiple: bool,
    pub open: bool,
    #[ts(type = "number | null")]
    pub closes_at_ms: Option<u64>,
    pub answer: Option<Vec<u16>>,
    /// Résultats de la classe, sans nom (SONDAGE-5).
    pub results: Option<Vec<u32>>,
    pub dismissed: bool,
}

/// Pédagogie de l'agent, partagée entre la connexion et les fenêtres.
pub struct Peda {
    ui: Arc<dyn Ui>,
    dir: PathBuf,
    /// Dossier Surveo de l'élève (DISTRIB-2).
    files_root: PathBuf,
    kind: PosteKind,
    saved: Saved,
    /// Fichiers distribués hors TP, proposés par la console.
    offers: Vec<FileOffer>,
    downloads: HashMap<String, Download>,
    poll: Option<PollState>,
    ended: Option<Ended>,
    error: Option<String>,
    connected: bool,
    /// Messages à envoyer, tant que la connexion tient.
    out: mpsc::UnboundedSender<AgentMsg>,
    /// L'état annoncé à la console a changé.
    changes: mpsc::Sender<()>,
    /// Dernière nouvelle de chaque rendu en attente.
    offered: HashMap<String, u64>,
    /// Fin de séance pour laquelle l'élève a été prévenu (DISTRIB-8).
    warned_for: Option<u64>,
}

fn local_hhmm(ms: u64) -> String {
    use chrono::TimeZone;
    chrono::Local.timestamp_millis_opt(ms as i64).single().map(|t| t.format("%H:%M").to_string()).unwrap_or_default()
}

fn local_stamp(ms: u64) -> String {
    use chrono::TimeZone;
    chrono::Local.timestamp_millis_opt(ms as i64).single().map(|t| t.format("%Y-%m-%d %Hh%M").to_string()).unwrap_or_default()
}

impl Peda {
    /// Reprend l'état gardé sur le poste. Les fichiers d'une séance dont
    /// l'échéance est passée sont retirés aussitôt (DISTRIB-6) ; ceux de
    /// la séance encore en cours attendent de savoir si le poste la
    /// rejoint.
    pub fn new(
        ui: Arc<dyn Ui>,
        dir: PathBuf,
        files_root: PathBuf,
        kind: PosteKind,
        out: mpsc::UnboundedSender<AgentMsg>,
        changes: mpsc::Sender<()>,
        now: u64,
    ) -> Self {
        let saved: Saved = load_json(&dir.join(STATE_FILE)).unwrap_or_default();
        let mut p = Self {
            ui,
            dir,
            files_root,
            kind,
            saved,
            offers: vec![],
            downloads: HashMap::new(),
            poll: None,
            ended: None,
            error: None,
            connected: false,
            out,
            changes,
            offered: HashMap::new(),
            warned_for: None,
        };
        let _ = std::fs::remove_dir_all(p.dir.join("reception"));
        if p.saved.session.as_ref().is_none_or(|s| now >= s.deadline_ms) {
            p.forget_session();
        }
        p
    }

    fn save(&self) {
        if let Err(e) = save_json(&self.dir.join(STATE_FILE), &self.saved, false) {
            log::error!("état du TP impossible à enregistrer : {e:#}");
        }
    }

    fn send(&self, m: AgentMsg) {
        if self.connected {
            let _ = self.out.send(m);
        }
    }

    fn changed(&self) {
        let _ = self.changes.try_send(());
    }

    fn publish(&self, now: u64) {
        self.ui.panel(&self.view(now));
    }

    fn session_id(&self) -> Option<&str> {
        self.saved.session.as_ref().map(|s| s.id.as_str())
    }

    // --- Séance --------------------------------------------------------

    /// Le poste entre dans une séance, ou en apprend la nouvelle heure de
    /// fin.
    pub fn begin(&mut self, info: &SessionInfo, now: u64) {
        if self.session_id() != Some(info.id.as_str()) {
            self.forget_session();
            self.ended = None;
        }
        let folder = self.saved.session.as_ref().and_then(|s| s.folder.clone());
        self.saved.session = Some(SavedSession {
            id: info.id.clone(),
            teacher: info.teacher.clone(),
            ends_at_ms: now + info.remaining_ms,
            deadline_ms: now + info.remaining_ms + info.margin_ms,
            folder,
        });
        self.save();
        self.publish(now);
    }

    /// Fin de la séance (TP-9) : la zone de dépôt se ferme (RENDU-8), les
    /// fichiers distribués sont retirés d'un poste de salle (DISTRIB-6) ;
    /// les rendus en attente partiront au retour de la connexion.
    pub fn end(&mut self, now: u64) {
        if self.saved.session.is_none() {
            return;
        }
        let had_tp = self.saved.tp.is_some();
        self.forget_session();
        self.poll = None;
        if had_tp {
            self.ended = Some(Ended::Session);
            self.ui.show_panel();
        }
        self.publish(now);
        self.changed();
    }

    /// Oublie la séance : TP, téléchargements et fichiers reçus, retirés
    /// sur un poste de salle (DISTRIB-6, DISTRIB-9).
    fn forget_session(&mut self) {
        if self.kind == PosteKind::Salle {
            self.remove_received();
        }
        self.saved.received.clear();
        for (_, d) in self.downloads.drain() {
            let _ = std::fs::remove_file(d.part);
        }
        self.offers.clear();
        self.saved.tp = None;
        self.saved.session = None;
        self.warned_for = None;
        let current_tp: Option<String> = None;
        self.prune_rendus(current_tp.as_deref());
        self.save();
    }

    /// Retire les fichiers distribués, et seulement eux (DISTRIB-7), puis
    /// le dossier de la séance s'il est vide.
    fn remove_received(&mut self) {
        for f in &self.saved.received {
            match std::fs::remove_file(&f.path) {
                Ok(()) => log::info!("fichier distribué retiré : {}", f.path.display()),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => log::warn!("fichier distribué impossible à retirer ({}) : {e}", f.path.display()),
            }
        }
        if let Some(folder) = self.saved.session.as_ref().and_then(|s| s.folder.as_ref()) {
            // Un dossier où l'élève a mis ses propres fichiers reste.
            let _ = std::fs::remove_dir(folder);
        }
    }

    /// Les rendus déjà reçus d'un autre TP ne s'affichent plus ; ceux en
    /// attente restent jusqu'à leur envoi.
    fn prune_rendus(&mut self, tp: Option<&str>) {
        self.saved.rendus.retain(|r| r.received_ms.is_none() && r.error.is_none() || Some(r.offer.tp.as_str()) == tp);
    }

    // --- TP -------------------------------------------------------------

    /// TP du poste (TP-1, TP-4, TP-9). Le même TP, renvoyé après une
    /// coupure ou un redémarrage, garde les étapes cochées (TP-3).
    pub fn set_tp(&mut self, tp: Option<TpInfo>, now: u64) {
        match (tp, self.saved.tp.take()) {
            (Some(info), Some(mut cur)) if cur.info.id == info.id => {
                cur.info = info;
                self.saved.tp = Some(cur);
            }
            (Some(info), _) => {
                log::info!("TP « {} » lancé", info.title);
                self.saved.tp = Some(SavedTp { info, done: BTreeSet::new(), since_ms: now, fired: None });
                self.ended = None;
                self.error = None;
                self.ui.show_panel();
            }
            (None, Some(cur)) => {
                log::info!("TP « {} » arrêté", cur.info.title);
                self.ended = Some(Ended::Stopped(cur.info.title));
            }
            (None, None) => {}
        }
        let tp = self.saved.tp.as_ref().map(|t| t.info.id.clone());
        self.prune_rendus(tp.as_deref());
        self.save();
        self.ensure_downloads(now);
        self.publish(now);
        self.changed();
    }

    fn current_step(t: &SavedTp) -> Option<u16> {
        (0..t.info.steps.len() as u16).find(|i| !t.done.contains(i))
    }

    fn is_late(t: &SavedTp, now: u64) -> bool {
        Self::current_step(t).is_some_and(|i| {
            let m = t.info.steps[i as usize].minutes as u64;
            m > 0 && now >= t.since_ms + m * 60_000
        })
    }

    /// L'élève coche ou décoche une étape (TP-6).
    pub fn set_step(&mut self, step: u16, done: bool, now: u64) {
        let Some(t) = self.saved.tp.as_mut() else { return };
        if step as usize >= t.info.steps.len() {
            return;
        }
        let before = Self::current_step(t);
        if done {
            t.done.insert(step);
        } else {
            t.done.remove(&step);
        }
        if Self::current_step(t) != before {
            t.since_ms = now;
        }
        self.save();
        self.publish(now);
        self.changed();
    }

    /// Avancement annoncé à la console (TP-8).
    pub fn progress(&self, now: u64) -> Option<TpProgress> {
        let t = self.saved.tp.as_ref()?;
        Some(TpProgress { tp: t.info.id.clone(), done: t.done.iter().copied().collect(), late: Self::is_late(t, now) })
    }

    /// Fichiers distribués entièrement reçus (DISTRIB-4).
    pub fn received_ids(&self) -> Vec<String> {
        self.saved.received.iter().map(|f| f.id.clone()).collect()
    }

    // --- Fichiers distribués -------------------------------------------

    /// Fichiers distribués au poste dans la séance (DISTRIB-1).
    pub fn set_files(&mut self, files: Vec<FileOffer>, now: u64) {
        self.offers = files;
        self.ensure_downloads(now);
        self.publish(now);
    }

    fn wanted_files(&self) -> Vec<FileOffer> {
        let tp = self.saved.tp.iter().flat_map(|t| t.info.files.iter().cloned());
        self.offers.iter().cloned().chain(tp).collect()
    }

    /// Tire les fichiers qui manquent ; abandonne ceux qui ne sont plus
    /// proposés.
    fn ensure_downloads(&mut self, now: u64) {
        let wanted = self.wanted_files();
        self.downloads.retain(|id, d| {
            let keep = wanted.iter().any(|f| &f.id == id);
            if !keep {
                let _ = std::fs::remove_file(&d.part);
            }
            keep
        });
        if self.saved.session.is_none() {
            return;
        }
        for f in wanted {
            if self.downloads.contains_key(&f.id) || self.saved.received.iter().any(|r| r.id == f.id) {
                continue;
            }
            let part = self.dir.join("reception").join(format!("{}.part", f.id));
            if let Err(e) = std::fs::create_dir_all(part.parent().unwrap()).and_then(|_| std::fs::File::create(&part).map(|_| ())) {
                log::error!("réception de « {} » impossible : {e}", f.name);
                continue;
            }
            let id = f.id.clone();
            self.downloads.insert(id.clone(), Download { offer: f, part, received: 0, hasher: Hasher::default(), asked_ms: now });
            self.fetch(&id, now);
        }
    }

    fn fetch(&mut self, id: &str, now: u64) {
        let Some(d) = self.downloads.get_mut(id) else { return };
        d.asked_ms = now;
        if d.received >= d.offer.size {
            self.finish_download(id, now);
            return;
        }
        let len = (d.offer.size - d.received).min(CHUNK_SIZE as u64) as u32;
        let m = AgentMsg::Fetch { file: id.into(), offset: d.received, len };
        self.send(m);
    }

    /// Morceau d'un fichier distribué ([`BinKind::Chunk`]).
    pub fn chunk(&mut self, file: &str, offset: u64, data: &[u8], now: u64) {
        let Some(d) = self.downloads.get_mut(file) else { return };
        if offset != d.received || data.is_empty() || d.received + data.len() as u64 > d.offer.size {
            return;
        }
        let written = std::fs::OpenOptions::new().append(true).open(&d.part).and_then(|mut f| std::io::Write::write_all(&mut f, data));
        if let Err(e) = written {
            log::error!("« {} » : écriture impossible ({e})", d.offer.name);
            return;
        }
        d.hasher.update(data);
        d.received += data.len() as u64;
        self.fetch(file, now);
    }

    /// Fichier complet : il rejoint le dossier de l'élève sous un nom
    /// libre (DISTRIB-2), noté avant d'être écrit (DISTRIB-6).
    fn finish_download(&mut self, id: &str, now: u64) {
        let Some(d) = self.downloads.remove(id) else { return };
        if d.hasher.clone().finish() != d.offer.sha256 {
            log::warn!("« {} » arrivé abîmé : nouvel envoi", d.offer.name);
            let _ = std::fs::remove_file(&d.part);
            self.ensure_downloads(now);
            return;
        }
        let Some(s) = self.saved.session.as_mut() else { return };
        let folder = s.folder.get_or_insert_with(|| self.files_root.join(format!("Séance {}", local_stamp(now)))).clone();
        let teacher = s.teacher.clone();
        if let Err(e) = std::fs::create_dir_all(&folder) {
            log::error!("dossier {} impossible à créer : {e}", folder.display());
            return;
        }
        let path = files::unique_path(&folder, &d.offer.name);
        self.saved.received.push(ReceivedFile { id: id.into(), name: d.offer.name.clone(), path: path.clone() });
        self.save();
        if let Err(e) = files::move_file(&d.part, &path) {
            log::error!("« {} » impossible à ranger : {e:#}", d.offer.name);
            self.saved.received.retain(|r| r.id != id);
            self.save();
            return;
        }
        log::info!("fichier reçu : {}", path.display());
        // Les fichiers joints d'un TP s'affichent avec le TP.
        if self.offers.iter().any(|f| f.id == id) {
            self.ui.notify(&format!("{teacher} vous a envoyé « {} ».", d.offer.name));
        }
        self.publish(now);
        self.changed();
    }

    /// Chemin d'un fichier reçu, pour l'ouvrir (TP-5, DISTRIB-3).
    pub fn file_path(&self, id: &str) -> Option<PathBuf> {
        self.saved.received.iter().find(|f| f.id == id).map(|f| f.path.clone())
    }

    pub fn folder(&self) -> Option<PathBuf> {
        self.saved.session.as_ref().and_then(|s| s.folder.clone()).filter(|f| f.is_dir())
    }

    // --- Rendus ----------------------------------------------------------

    /// Où le dépôt d'un rendu irait : séance et TP en cours, sinon le
    /// motif du refus (RENDU-8).
    fn deposit_target(&self) -> Result<(SessionId, String), String> {
        match (&self.saved.session, &self.saved.tp) {
            (Some(s), Some(t)) => Ok((s.id.clone(), t.info.id.clone())),
            (None, _) => Err("La séance est terminée : la zone de dépôt est fermée.".into()),
            (_, None) => Err("Aucun TP en cours : rien à rendre pour l'instant.".into()),
        }
    }

    fn deposit_error(&mut self, e: String, now: u64) -> String {
        self.error = Some(e.clone());
        self.publish(now);
        e
    }

    /// Enregistre un rendu déjà copié dans le dossier de l'agent.
    fn register_rendu(&mut self, session: SessionId, tp: String, file: FileOffer, now: u64) {
        log::info!("rendu déposé : {}", file.name);
        let offer = RenduOffer { session, tp, file };
        self.saved.rendus.push(SavedRendu { offer: offer.clone(), deposited_ms: now, received_ms: None, error: None });
        self.error = None;
        self.save();
        self.offered.insert(offer.file.id.clone(), now);
        self.send(AgentMsg::Submit { rendu: offer });
        self.publish(now);
    }

    fn rendu_copy(&self, id: &str) -> PathBuf {
        self.dir.join("rendus").join(id)
    }

    /// Morceau d'un rendu demandé par la console ; rien s'il n'est plus
    /// en attente.
    pub fn rendu_chunk(&mut self, id: &str, offset: u64, len: u32, now: u64) -> Option<Vec<u8>> {
        let r = self.saved.rendus.iter().find(|r| r.offer.file.id == id && r.received_ms.is_none())?;
        let data = files::read_at(&self.rendu_copy(&r.offer.file.id), offset, len.min(CHUNK_SIZE) as usize).ok()?;
        self.offered.insert(id.into(), now);
        Some(encode_chunk(id, offset, &data))
    }

    /// Accusé de réception de l'enseignant (RENDU-2), ou refus (RENDU-7).
    pub fn receipt(&mut self, id: &str, error: Option<String>, now: u64) {
        let Some(r) = self.saved.rendus.iter_mut().find(|r| r.offer.file.id == id) else { return };
        match error {
            None => r.received_ms = Some(now),
            Some(e) => r.error = Some(e),
        }
        let _ = std::fs::remove_file(self.rendu_copy(id));
        self.offered.remove(id);
        let tp = self.saved.tp.as_ref().map(|t| t.info.id.clone());
        self.prune_rendus(tp.as_deref());
        self.save();
        self.publish(now);
    }

    fn pending_rendus(&self) -> impl Iterator<Item = &SavedRendu> {
        self.saved.rendus.iter().filter(|r| r.received_ms.is_none() && r.error.is_none())
    }

    // --- Sondage ---------------------------------------------------------

    /// Sondage du poste (SONDAGE-2, SONDAGE-4, SONDAGE-5). La question
    /// s'impose au premier plan tant que l'élève n'a pas répondu.
    pub fn set_poll(&mut self, poll: Option<PollInfo>, now: u64) {
        self.poll = match (poll, self.poll.take()) {
            (None, _) => None,
            (Some(info), Some(mut cur)) if cur.info.id == info.id => {
                if info.open && cur.answer.is_some() && cur.answer != info.answer {
                    // Réponse donnée pendant une coupure : la console ne la
                    // connaît pas encore.
                    self.send(AgentMsg::Answer { poll: info.id.clone(), choices: cur.answer.clone().unwrap() });
                }
                cur.closes_at_ms = info.closes_in_ms.map(|ms| now + ms);
                cur.info = info;
                Some(cur)
            }
            (Some(info), _) => Some(PollState { answer: info.answer.clone(), closes_at_ms: info.closes_in_ms.map(|ms| now + ms), dismissed: false, info }),
        };
        self.publish(now);
    }

    /// Réponse de l'élève, qu'il peut changer tant que le sondage est
    /// ouvert (SONDAGE-2).
    pub fn answer(&mut self, choices: Vec<u16>, now: u64) {
        let Some(p) = self.poll.as_mut().filter(|p| p.info.open) else { return };
        let mut choices: Vec<u16> = choices.into_iter().filter(|c| (*c as usize) < p.info.options.len()).collect::<BTreeSet<_>>().into_iter().collect();
        if !p.info.multiple {
            choices.truncate(1);
        }
        if choices.is_empty() {
            return;
        }
        p.answer = Some(choices.clone());
        let id = p.info.id.clone();
        self.send(AgentMsg::Answer { poll: id, choices });
        self.publish(now);
    }

    /// « Plus tard » : la question quitte le premier plan et reste dans le
    /// panneau.
    pub fn dismiss_poll(&mut self, now: u64) {
        if let Some(p) = self.poll.as_mut() {
            p.dismissed = true;
        }
        self.publish(now);
    }

    // --- Connexion et échéances -------------------------------------------

    /// La connexion s'établit ou se perd. À l'établissement, rend ce qui
    /// doit repartir : rendus en attente (RENDU-3), morceaux attendus,
    /// réponse au sondage.
    pub fn set_connected(&mut self, connected: bool, now: u64) -> Vec<AgentMsg> {
        self.connected = connected;
        let mut out = vec![];
        if connected {
            for r in self.pending_rendus() {
                out.push(AgentMsg::Submit { rendu: r.offer.clone() });
            }
            let ids: Vec<String> = self.pending_rendus().map(|r| r.offer.file.id.clone()).collect();
            for id in ids {
                self.offered.insert(id, now);
            }
            for d in self.downloads.values_mut() {
                d.asked_ms = now;
                let len = (d.offer.size - d.received).min(CHUNK_SIZE as u64) as u32;
                out.push(AgentMsg::Fetch { file: d.offer.id.clone(), offset: d.received, len });
            }
        }
        self.publish(now);
        out
    }

    /// Vérifications périodiques : retard sur l'étape en cours et son
    /// action (TP-7), transferts sans nouvelles, avertissement de fin de
    /// séance (DISTRIB-8). Rend `true` si l'état annoncé a changé.
    pub fn tick(&mut self, now: u64) -> bool {
        let mut changed = false;
        if let Some(t) = self.saved.tp.as_mut() {
            let late = Self::is_late(t, now);
            let step = Self::current_step(t);
            if late && t.fired != step {
                t.fired = step;
                changed = true;
                let s = &t.info.steps[step.unwrap() as usize];
                log::info!("étape « {} » en retard", s.objective);
                if s.overrun == Overrun::ShowInstructions {
                    self.ui.show_panel();
                }
                self.save();
                self.publish(now);
            }
        }
        if self.saved.session.as_ref().is_some_and(|s| now >= s.deadline_ms) {
            // Échéance passée sans nouvelles de la séance : le poste l'a
            // quittée (RESTR-1).
            self.end(now);
        }
        if self.connected {
            let stalled: Vec<String> = self.downloads.iter().filter(|(_, d)| now >= d.asked_ms + FETCH_TIMEOUT_MS).map(|(k, _)| k.clone()).collect();
            for id in stalled {
                self.fetch(&id, now);
            }
            let quiet: Vec<RenduOffer> = self
                .pending_rendus()
                .filter(|r| self.offered.get(&r.offer.file.id).is_none_or(|t| now >= t + OFFER_TIMEOUT_MS))
                .map(|r| r.offer.clone())
                .collect();
            for r in quiet {
                self.offered.insert(r.file.id.clone(), now);
                self.send(AgentMsg::Submit { rendu: r });
            }
        }
        if let Some(s) = &self.saved.session {
            let has_files = !self.saved.received.is_empty() || !self.downloads.is_empty();
            let due = now + END_WARNING_MS >= s.ends_at_ms && now < s.ends_at_ms;
            if self.kind == PosteKind::Salle && has_files && due && self.warned_for != Some(s.ends_at_ms) {
                self.warned_for = Some(s.ends_at_ms);
                self.ui.notify(&format!(
                    "La séance se termine à {}. Les fichiers reçus pendant la séance seront alors retirés de ce poste : pensez à rendre votre travail.",
                    local_hhmm(s.ends_at_ms)
                ));
            }
        }
        changed
    }

    // --- Vue ---------------------------------------------------------------

    fn file_view(&self, f: &FileOffer) -> PanelFile {
        PanelFile { id: f.id.clone(), name: f.name.clone(), size: size_text(f.size), received: self.saved.received.iter().any(|r| r.id == f.id) }
    }

    pub fn view(&self, now: u64) -> PanelView {
        let session = self.saved.session.as_ref();
        let tp = self.saved.tp.as_ref().map(|t| {
            let received: BTreeSet<String> = self
                .saved
                .rendus
                .iter()
                .filter(|r| r.offer.tp == t.info.id && r.received_ms.is_some())
                .map(|r| r.offer.file.name.to_lowercase())
                .collect();
            PanelTp {
                title: t.info.title.clone(),
                steps: t
                    .info
                    .steps
                    .iter()
                    .enumerate()
                    .map(|(i, s)| PanelStep { objective: s.objective.clone(), instructions: s.instructions.clone(), minutes: s.minutes, done: t.done.contains(&(i as u16)) })
                    .collect(),
                current: Self::current_step(t).map(u32::from),
                current_since_ms: t.since_ms,
                late: Self::is_late(t, now),
                expected: t.info.expected,
                received: received.len() as u32,
                files: t.info.files.iter().map(|f| self.file_view(f)).collect(),
            }
        });
        let pending = self.pending_rendus().count() as u32;
        PanelView {
            teacher: session.map(|s| s.teacher.clone()).unwrap_or_default(),
            ends_at_ms: session.map(|s| s.ends_at_ms),
            connected: self.connected,
            tp,
            ended: self.ended.as_ref().map(|e| match e {
                Ended::Session => PanelEnded::Session { pending },
                Ended::Stopped(title) => PanelEnded::Stopped { title: title.clone(), pending },
            }),
            rendus: self
                .saved
                .rendus
                .iter()
                .map(|r| PanelRendu {
                    id: r.offer.file.id.clone(),
                    name: r.offer.file.name.clone(),
                    state: match (&r.received_ms, &r.error) {
                        (_, Some(e)) => RenduState::Refused { reason: e.clone() },
                        (Some(t), None) => RenduState::Received { at_ms: *t },
                        (None, None) if self.connected => RenduState::Sending,
                        (None, None) => RenduState::Waiting,
                    },
                })
                .collect(),
            can_submit: self.deposit_target().is_ok(),
            limit: size_text(MAX_RENDU_BYTES),
            error: self.error.clone(),
            files: self.offers.iter().map(|f| self.file_view(f)).collect(),
            removed_at_end: self.kind == PosteKind::Salle,
            poll: self.poll.as_ref().map(|p| PanelPoll {
                id: p.info.id.clone(),
                question: p.info.question.clone(),
                options: p.info.options.clone(),
                multiple: p.info.multiple,
                open: p.info.open,
                closes_at_ms: p.closes_at_ms.filter(|_| p.info.open),
                answer: p.answer.clone(),
                results: p.info.results.clone(),
                dismissed: p.dismissed,
            }),
        }
    }
}

/// Dépose un rendu (RENDU-1) : l'agent en fait aussitôt une copie, qui
/// fige ce qui est rendu à l'heure du dépôt (RENDU-8), puis la propose à
/// la console. Le fichier de l'élève reste en place. Rend le refus,
/// expliqué à l'élève (RENDU-7).
pub fn submit(peda: &Mutex<Peda>, path: &Path, now: impl Fn() -> u64) -> Result<(), String> {
    let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let (session, tp, dest) = {
        let mut p = peda.lock().unwrap();
        let (session, tp) = match p.deposit_target() {
            Ok(t) => t,
            Err(e) => return Err(p.deposit_error(e, now())),
        };
        let meta = match std::fs::metadata(path) {
            Ok(m) if m.is_file() => m,
            Ok(_) => return Err(p.deposit_error(format!("« {name} » est un dossier : rendez un fichier."), now())),
            Err(_) => return Err(p.deposit_error(format!("« {name} » est illisible."), now())),
        };
        if meta.len() > MAX_RENDU_BYTES {
            let e = format!("« {name} » est trop volumineux ({}) : la limite est de {}.", size_text(meta.len()), size_text(MAX_RENDU_BYTES));
            return Err(p.deposit_error(e, now()));
        }
        let id = random_id();
        let dest = p.rendu_copy(&id);
        (session, tp, (id, dest))
    };
    // La copie se fait sans bloquer la connexion.
    let copied = files::copy_hashed(path, &dest.1);
    let mut p = peda.lock().unwrap();
    match copied {
        Ok((size, sha256)) => {
            let file = FileOffer { id: dest.0, name: files::safe_file_name(&name), size, sha256 };
            p.register_rendu(session, tp, file, now());
            Ok(())
        }
        Err(e) => {
            let _ = std::fs::remove_file(&dest.1);
            log::warn!("copie du rendu impossible : {e:#}");
            Err(p.deposit_error(format!("« {name} » n'a pas pu être copié : réessayez."), now()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::SessionView;
    use crate::restrict::RestrictionsView;

    #[derive(Default)]
    struct TestUi {
        panel: Mutex<Option<PanelView>>,
        shown: Mutex<u32>,
        notices: Mutex<Vec<String>>,
    }

    impl Ui for TestUi {
        fn session(&self, _: Option<&SessionView>) {}
        fn identify(&self, _: &str, _: u32) {}
        fn restrictions(&self, _: &RestrictionsView) {}
        fn pause(&self, _: Option<&str>) -> Result<(), String> {
            Ok(())
        }
        fn notify(&self, t: &str) {
            self.notices.lock().unwrap().push(t.into());
        }
        fn panel(&self, v: &PanelView) {
            *self.panel.lock().unwrap() = Some(v.clone());
        }
        fn show_panel(&self) {
            *self.shown.lock().unwrap() += 1;
        }
    }

    const T0: u64 = 1_800_000_000_000;

    struct Rig {
        p: Peda,
        ui: Arc<TestUi>,
        out: mpsc::UnboundedReceiver<AgentMsg>,
        dir: PathBuf,
        root: PathBuf,
    }

    fn rig_in(dir: PathBuf, now: u64) -> Rig {
        let root = dir.join("Documents");
        let ui = Arc::new(TestUi::default());
        let (tx, out) = mpsc::unbounded_channel();
        let (changes, _) = mpsc::channel(4);
        let p = Peda::new(ui.clone(), dir.clone(), root.clone(), PosteKind::Salle, tx, changes, now);
        Rig { p, ui, out, dir, root }
    }

    fn rig() -> Rig {
        rig_in(std::env::temp_dir().join(format!("surveo-peda-{}", random_id())), T0)
    }

    fn session(id: &str) -> SessionInfo {
        SessionInfo { id: id.into(), room: None, teacher: "M. Durand".into(), remaining_ms: 60 * 60_000, margin_ms: 10 * 60_000 }
    }

    fn offer(id: &str, name: &str, data: &[u8]) -> FileOffer {
        FileOffer { id: id.into(), name: name.into(), size: data.len() as u64, sha256: files::sha256_hex(data) }
    }

    fn tp(files: Vec<FileOffer>) -> TpInfo {
        let step = |o: &str, m: u32| Step { objective: o.into(), instructions: String::new(), minutes: m, overrun: Overrun::ShowInstructions };
        TpInfo { id: "tp1".into(), title: "Adressage IPv4".into(), steps: vec![step("Adresser", 10), step("Tester", 0), step("Rédiger", 15)], expected: 2, files }
    }

    fn drain(r: &mut Rig) -> Vec<AgentMsg> {
        std::iter::from_fn(|| r.out.try_recv().ok()).collect()
    }

    /// Répond aux demandes de morceaux avec le contenu de chaque fichier.
    fn serve(r: &mut Rig, contents: &[(&str, &[u8])]) {
        loop {
            let asks = drain(r);
            if asks.is_empty() {
                return;
            }
            for m in asks {
                if let AgentMsg::Fetch { file, offset, len } = m {
                    let data = contents.iter().find(|(id, _)| *id == file).unwrap().1;
                    let end = (offset as usize + len as usize).min(data.len());
                    r.p.chunk(&file, offset, &data[offset as usize..end], T0);
                }
            }
        }
    }

    #[test]
    fn steps_lateness_and_tp_survive_a_restart() {
        let mut r = rig();
        r.p.set_connected(true, T0);
        r.p.begin(&session("s1"), T0);
        r.p.set_tp(Some(tp(vec![])), T0);
        assert_eq!(*r.ui.shown.lock().unwrap(), 1, "le panneau s'ouvre au lancement");
        r.p.set_step(0, true, T0 + 60_000);
        assert_eq!(r.p.progress(T0 + 60_000).unwrap().done, [0]);
        // L'étape 2 n'a pas de durée ; l'étape 3 en a une.
        r.p.set_step(1, true, T0 + 120_000);
        assert!(!r.p.tick(T0 + 120_000 + 14 * 60_000));
        assert!(r.p.tick(T0 + 120_000 + 15 * 60_000), "en retard (TP-7)");
        assert!(r.p.progress(T0 + 120_000 + 15 * 60_000).unwrap().late);
        assert_eq!(*r.ui.shown.lock().unwrap(), 2, "consignes affichées au dépassement");
        assert!(!r.p.tick(T0 + 120_000 + 16 * 60_000), "une seule fois par étape");
        // L'élève revient sur son choix (TP-6).
        r.p.set_step(0, false, T0 + 20 * 60_000);
        assert_eq!(r.p.view(T0 + 20 * 60_000).tp.unwrap().current, Some(0));
        r.p.set_step(0, true, T0 + 20 * 60_000);

        // Redémarrage du poste pendant la séance (TP-3).
        let dir = r.dir.clone();
        drop(r);
        let mut r = rig_in(dir, T0 + 25 * 60_000);
        r.p.begin(&session("s1"), T0 + 25 * 60_000);
        r.p.set_tp(Some(tp(vec![])), T0 + 25 * 60_000);
        assert_eq!(r.p.progress(T0 + 25 * 60_000).unwrap().done, [0, 1]);
        assert_eq!(*r.ui.shown.lock().unwrap(), 0, "même TP : le panneau ne se rouvre pas");

        // Arrêt par l'enseignant, puis fin de la séance (TP-9).
        r.p.set_tp(None, T0 + 26 * 60_000);
        assert_eq!(r.p.view(T0).ended, Some(PanelEnded::Stopped { title: "Adressage IPv4".into(), pending: 0 }));
        assert!(r.p.progress(T0).is_none());
    }

    #[test]
    fn distributed_files_never_overwrite_and_leave_at_the_end() {
        let mut r = rig();
        r.p.set_connected(true, T0);
        r.p.begin(&session("s1"), T0);
        let cours = offer("f1", "cours.pdf", b"le cours");
        r.p.set_files(vec![cours.clone()], T0);
        serve(&mut r, &[("f1", b"le cours")]);
        let folder = r.p.folder().unwrap();
        assert!(folder.starts_with(&r.root));
        assert_eq!(std::fs::read(folder.join("cours.pdf")).unwrap(), b"le cours");
        assert_eq!(r.ui.notices.lock().unwrap().as_slice(), ["M. Durand vous a envoyé « cours.pdf »."]);
        assert_eq!(r.p.received_ids(), ["f1"]);

        // Un fichier de l'élève du même nom reste intact (critère 7).
        std::fs::write(folder.join("tp.odt"), b"travail de l'eleve").unwrap();
        r.p.set_files(vec![cours, offer("f2", "tp.odt", b"sujet")], T0);
        serve(&mut r, &[("f2", b"sujet")]);
        assert_eq!(std::fs::read(folder.join("tp.odt")).unwrap(), b"travail de l'eleve");
        assert_eq!(std::fs::read(folder.join("tp (2).odt")).unwrap(), b"sujet");

        // Prévenu quelques minutes avant la fin (DISTRIB-8).
        r.p.tick(T0 + 56 * 60_000);
        assert!(r.ui.notices.lock().unwrap().last().unwrap().starts_with("La séance se termine à "));

        // Fin de la séance : seuls les fichiers distribués partent (critère 12).
        r.p.end(T0 + 60 * 60_000);
        assert!(!folder.join("cours.pdf").exists() && !folder.join("tp (2).odt").exists());
        assert_eq!(std::fs::read(folder.join("tp.odt")).unwrap(), b"travail de l'eleve");
    }

    #[test]
    fn a_station_off_at_the_end_removes_files_at_next_start() {
        let mut r = rig();
        r.p.set_connected(true, T0);
        r.p.begin(&session("s1"), T0);
        r.p.set_files(vec![offer("f1", "cours.pdf", b"x")], T0);
        serve(&mut r, &[("f1", b"x")]);
        let file = r.p.folder().unwrap().join("cours.pdf");
        let dir = r.dir.clone();
        std::mem::forget(r); // poste éteint brutalement
        // Redémarré pendant la séance : le fichier reste, le poste la rejoint.
        let r2 = rig_in(dir.clone(), T0 + 30 * 60_000);
        assert!(file.exists());
        drop(r2);
        // Redémarré après l'échéance : retiré (DISTRIB-6).
        let _r3 = rig_in(dir, T0 + 71 * 60_000);
        assert!(!file.exists());
    }

    #[test]
    fn rendus_wait_for_the_connection_and_the_receipt() {
        let mut r = rig();
        r.p.begin(&session("s1"), T0);
        r.p.set_tp(Some(tp(vec![])), T0);
        let work = r.dir.join("compte-rendu.odt");
        std::fs::write(&work, b"mon travail").unwrap();
        let peda = Mutex::new(r.p);

        // Coupure : le rendu attend (RENDU-3).
        submit(&peda, &work, || T0).unwrap();
        let mut p = peda.into_inner().unwrap();
        assert!(drain_rx(&mut r.out).is_empty());
        assert_eq!(p.view(T0).rendus[0].state, RenduState::Waiting);
        assert!(work.exists(), "le fichier de l'élève reste en place (RENDU-1)");

        // Fin de la séance pendant la coupure : zone de dépôt fermée, le
        // rendu déposé avant partira (RENDU-8, critère 11).
        p.end(T0 + 1000);
        assert_eq!(p.view(T0).ended, Some(PanelEnded::Session { pending: 1 }));
        let peda = Mutex::new(p);
        assert_eq!(submit(&peda, &work, || T0).unwrap_err(), "La séance est terminée : la zone de dépôt est fermée.");
        let mut p = peda.into_inner().unwrap();

        // Retour de la connexion : le rendu est proposé, tiré, puis reçu.
        let again = p.set_connected(true, T0 + 2000);
        let AgentMsg::Submit { rendu } = &again[0] else { panic!("{again:?}") };
        assert_eq!((rendu.file.name.as_str(), rendu.file.size), ("compte-rendu.odt", 11));
        let chunk = p.rendu_chunk(&rendu.file.id, 0, CHUNK_SIZE, T0).unwrap();
        assert_eq!(decode_chunk(&chunk), Some((rendu.file.id.as_str(), 0, &b"mon travail"[..])));
        assert_eq!(p.view(T0).rendus[0].state, RenduState::Sending, "pas d'accusé avant la réception (RENDU-2)");
        p.receipt(&rendu.file.id, None, T0 + 3000);
        assert_eq!(p.view(T0).ended, Some(PanelEnded::Session { pending: 0 }));
        assert!(p.rendu_chunk(&rendu.file.id, 0, 10, T0).is_none());
    }

    fn drain_rx(rx: &mut mpsc::UnboundedReceiver<AgentMsg>) -> Vec<AgentMsg> {
        std::iter::from_fn(|| rx.try_recv().ok()).collect()
    }

    #[test]
    fn too_big_a_rendu_is_refused_with_its_size() {
        let mut r = rig();
        r.p.begin(&session("s1"), T0);
        let peda = Mutex::new(r.p);
        let work = r.dir.join("x.odt");
        std::fs::write(&work, b"x").unwrap();
        assert_eq!(submit(&peda, &work, || T0).unwrap_err(), "Aucun TP en cours : rien à rendre pour l'instant.");
        peda.lock().unwrap().set_tp(Some(tp(vec![])), T0);
        let big = r.dir.join("film.mp4");
        std::fs::File::create(&big).unwrap().set_len(MAX_RENDU_BYTES + 1).unwrap();
        assert_eq!(submit(&peda, &big, || T0).unwrap_err(), "« film.mp4 » est trop volumineux (50 Mo) : la limite est de 50 Mo.");
        assert_eq!(peda.lock().unwrap().view(T0).error.as_deref(), Some("« film.mp4 » est trop volumineux (50 Mo) : la limite est de 50 Mo."));
    }

    #[test]
    fn poll_comes_forward_until_answered() {
        let mut r = rig();
        r.p.set_connected(true, T0);
        r.p.begin(&session("s1"), T0);
        let poll = PollInfo { id: "p1".into(), question: "Où en êtes-vous ?".into(), options: vec!["Fini".into(), "Bloqué".into()], multiple: false, open: true, closes_in_ms: None, answer: None, results: None };
        r.p.set_poll(Some(poll.clone()), T0);
        let v = r.p.view(T0).poll.unwrap();
        assert!(v.open && v.answer.is_none() && !v.dismissed);
        r.p.answer(vec![1, 0], T0);
        assert_eq!(drain(&mut r), [AgentMsg::Answer { poll: "p1".into(), choices: vec![0] }]);
        // Le même sondage renvoyé sans la réponse (coupure) : elle repart.
        r.p.set_poll(Some(poll.clone()), T0);
        assert_eq!(drain(&mut r), [AgentMsg::Answer { poll: "p1".into(), choices: vec![0] }]);
        // Clos, résultats montrés : plus de réponse possible.
        r.p.set_poll(Some(PollInfo { open: false, results: Some(vec![3, 1]), answer: Some(vec![0]), ..poll }), T0);
        r.p.answer(vec![1], T0);
        assert!(drain(&mut r).is_empty());
        assert_eq!(r.p.view(T0).poll.unwrap().results, Some(vec![3, 1]));
        r.p.end(T0);
        assert!(r.p.view(T0).poll.is_none());
    }
}
