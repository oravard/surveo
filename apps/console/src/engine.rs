//! Salles, séance et retours poste par poste (SPEC.md, chap. 1, 9.5).
//!
//! Le moteur est synchrone et ne connaît ni l'interface ni le réseau :
//! il reçoit les messages du relais ([`Engine::handle`]), les gestes de
//! l'enseignant, et l'heure courante en paramètre, et envoie ses
//! commandes par un [`Link`]. Il est appelé toutes les secondes par
//! [`Engine::tick`] pour les échéances.

use crate::store::{AppList, ExamState, JournalEntry, Lists, Pos, Profile, Room, Session, SessionRestrictions, SiteList, Station, Store};
use crate::view::*;
use crate::*;
use anyhow::{anyhow, bail, ensure, Result};
use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};
use surveo_identity::{new_member_id, random_id, Establishment, Identity, Package, Role, Trust};
use surveo_protocol::*;

/// Envoi des commandes vers les postes, par le relais.
pub trait Link: Send {
    fn send(&self, m: &FromConsole);
}

impl Link for surveo_relay_core::LocalSender {
    fn send(&self, m: &FromConsole) {
        surveo_relay_core::LocalSender::send(self, m)
    }
}

/// Un poste connecté au relais.
struct Online {
    presence: AgentPresence,
    session: Option<SessionId>,
    screen: ScreenState,
    web: WebState,
}

/// Action envoyée à plusieurs postes, en attente de leurs réponses
/// (RETOUR-1).
struct Pending {
    label: String,
    /// Commande → poste.
    ids: BTreeMap<u64, MemberId>,
    acked: BTreeSet<MemberId>,
    refused: Vec<(MemberId, String)>,
    /// Postes absents au moment de l'envoi.
    offline: Vec<MemberId>,
    /// Les postes absents recevront l'action à leur retour (RETOUR-2).
    deferred: bool,
    /// Retour quand tous les postes ont répondu, à la place du décompte.
    done_text: Option<String>,
    /// L'action peut être annulée (RETOUR-3).
    undo: Option<u64>,
    deadline_ms: u64,
}

impl Pending {
    fn new(label: impl Into<String>, deadline_ms: u64) -> Self {
        Self {
            label: label.into(),
            ids: BTreeMap::new(),
            acked: BTreeSet::new(),
            refused: vec![],
            offline: vec![],
            deferred: false,
            done_text: None,
            undo: None,
            deadline_ms,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Restore {
    Pending,
    Restored,
    Unreachable,
}

/// Sortie de restriction signalée à l'enseignant (MODE-3, MODE-5).
struct Exit {
    at_ms: u64,
    member: MemberId,
    /// Connexion perdue, et l'heure du retour du poste ; sinon, filtrage
    /// coupé sur le poste.
    lost: Option<Option<u64>>,
}

/// Remise en état des postes après la clôture (RESTR-5).
struct Closure {
    session: SessionId,
    ended_at_ms: u64,
    auto: bool,
    restore_by_ms: u64,
    deadline_ms: u64,
    entries: BTreeMap<MemberId, Restore>,
}

pub struct Engine {
    store: Store,
    etab: Option<Establishment>,
    profile: Option<Profile>,
    rooms: Vec<Room>,
    room: Option<RoomId>,
    session: Option<Session>,
    online: HashMap<MemberId, Online>,
    pending: Vec<Pending>,
    closure: Option<Closure>,
    refused: VecDeque<String>,
    /// Sorties de restriction et pertes de connexion d'un poste restreint
    /// (MODE-3, MODE-5).
    alerts: VecDeque<Exit>,
    /// Restrictions d'avant la dernière action, pour « Annuler » (RETOUR-3).
    undo: Option<(u64, SessionRestrictions)>,
    lists: Lists,
    /// Dernière liste des applications ouvertes de chaque poste (APP-1).
    apps: HashMap<MemberId, Vec<OpenApp>>,
    link: Option<Box<dyn Link>>,
    next_id: u64,
    /// État du relais intégré, tenu par l'application.
    pub relay: RelayView,
    notices: Vec<Notice>,
}

fn plural(n: usize, word: &str) -> String {
    format!("{n} {word}{}", if n > 1 { "s" } else { "" })
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or_default()
}

fn dedup(v: Vec<String>) -> Vec<String> {
    let mut seen = BTreeSet::new();
    v.into_iter().filter(|x| seen.insert(x.to_lowercase())).collect()
}

/// Sites d'une liste, ramenés à leur nom (WEB-1) ; au moins un.
fn sites_list(sites: &[String]) -> Result<Vec<String>> {
    let mut out = Vec::new();
    for x in sites.iter().filter(|x| !x.trim().is_empty()) {
        out.push(normalize_site(x).ok_or_else(|| anyhow!("« {} » n'est pas un nom de site.", x.trim()))?);
    }
    ensure!(!out.is_empty(), "Indiquez au moins un site.");
    Ok(dedup(out))
}

/// Noms d'applications tels que l'enseignant les écrit, sans doublon.
fn names_list(names: &[String]) -> Vec<String> {
    dedup(names.iter().map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect())
}

/// « wikipedia.org et cisco.com », « 4 sites (wikipedia.org…) ».
fn sites_text(sites: &[String]) -> String {
    match sites {
        [a] => a.clone(),
        [a, b] => format!("{a} et {b}"),
        [a, ..] => format!("{} sites ({a}…)", sites.len()),
        [] => "aucun site".into(),
    }
}

impl Engine {
    pub fn new(store: Store, now: u64) -> Result<Self> {
        let etab = store.establishment().map(|f| Establishment::from_file(&f)).transpose()?;
        let profile = store.profile();
        let rooms = store.rooms();
        // Une séance dont l'échéance est passée pendant que la console était
        // fermée est terminée : les postes l'ont quittée d'eux-mêmes.
        let session = store.session().filter(|s| now < s.end_ms + MARGIN_MS);
        if session.is_none() {
            store.save_session(None)?;
        }
        let room = session.as_ref().map(|s| s.room.clone()).or_else(|| rooms.first().map(|r| r.id.clone()));
        let lists = store.lists();
        Ok(Self {
            store,
            etab,
            profile,
            rooms,
            room,
            session,
            online: HashMap::new(),
            pending: Vec::new(),
            closure: None,
            refused: VecDeque::new(),
            alerts: VecDeque::new(),
            undo: None,
            lists,
            apps: HashMap::new(),
            link: None,
            next_id: 1,
            relay: RelayView::default(),
            notices: Vec::new(),
        })
    }

    pub fn is_set_up(&self) -> bool {
        self.etab.is_some() && self.profile.is_some()
    }

    /// Création d'un établissement, localement et sans réseau (CONF-1,
    /// AUTO-1) : la console en devient le premier membre, avec le relais
    /// qu'elle embarque.
    pub fn create_establishment(&mut self, etab_name: &str, teacher: &str) -> Result<()> {
        ensure!(!self.is_set_up(), "Cette console fait déjà partie d'un établissement.");
        let (etab_name, teacher) = (etab_name.trim(), teacher.trim());
        ensure!(!etab_name.is_empty(), "Indiquez le nom de l'établissement.");
        ensure!(!teacher.is_empty(), "Indiquez votre nom, tel que les élèves le verront.");
        let etab = Establishment::create(etab_name);
        let profile = Profile {
            teacher: teacher.into(),
            console: etab.issue(&new_member_id("console"), Role::Console).to_file(),
            relay: etab.issue(&new_member_id("relais"), Role::Relay).to_file(),
        };
        self.store.save_establishment(&etab.to_file())?;
        self.store.save_profile(&profile)?;
        self.etab = Some(etab);
        self.profile = Some(profile);
        Ok(())
    }

    /// Identités du relais intégré et de la console, et confiance commune.
    pub fn relay_identity(&self) -> Option<(Identity, Trust, MemberId)> {
        let (etab, p) = (self.etab.as_ref()?, self.profile.as_ref()?);
        let relay = Identity::from_file(&p.relay).ok()?;
        Some((relay, etab.trust(), p.console.credential.cert.member.clone()))
    }

    pub fn attach(&mut self, link: Box<dyn Link>) {
        self.link = Some(link);
    }

    pub fn take_notices(&mut self) -> Vec<Notice> {
        std::mem::take(&mut self.notices)
    }

    fn notice(&mut self, text: impl Into<String>, detail: Option<String>, warning: bool) {
        self.notices.push(Notice { text: text.into(), detail, warning, undo: None });
    }

    fn alert(&mut self, now: u64, member: &str, lost: bool) {
        self.alerts.push_front(Exit { at_ms: now, member: member.into(), lost: lost.then_some(None) });
        self.alerts.truncate(20);
    }

    fn send(&mut self, to: Target, cmd: Command) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        if let Some(link) = &self.link {
            link.send(&FromConsole::Send { id, to, cmd });
        }
        id
    }

    // --- Messages du relais ---------------------------------------------

    pub fn handle(&mut self, msg: ToConsole, now: u64) {
        match msg {
            ToConsole::Roster { agents } => {
                self.online.clear();
                for a in agents {
                    self.agent_online(a, now);
                }
            }
            ToConsole::Online { agent } => self.agent_online(agent, now),
            ToConsole::Offline { member } => {
                self.online.remove(&member);
                self.apps.remove(&member);
                // Sans droits d'administration, un agent arrêté ne restreint
                // plus rien : la console le signale (MODE-5).
                let restricted = self.session.as_ref().is_some_and(|s| s.members.contains(&member) && Self::restrictions_for(&s.restrictions, &member, now) != Restrictions::default());
                if restricted {
                    self.alert(now, &member, true);
                }
            }
            ToConsole::FromAgent { from, msg: AgentMsg::Ack { id, ok, detail } } => {
                for p in &mut self.pending {
                    if p.ids.get(&id) == Some(&from) {
                        if ok {
                            p.acked.insert(from.clone());
                        } else {
                            p.refused.push((from.clone(), detail.clone().unwrap_or_default()));
                        }
                    }
                }
            }
            ToConsole::FromAgent { from, msg: AgentMsg::Status { session, screen, web } } => self.agent_status(&from, session, screen, web, now),
            ToConsole::FromAgent { from, msg: AgentMsg::Apps { apps, .. } } => {
                self.apps.insert(from, apps);
            }
            ToConsole::Refused { peer, reason } => {
                self.refused.push_front(format!("{peer} : {reason}"));
                self.refused.truncate(10);
                self.notice("Connexion d'un poste refusée", Some(format!("{peer} : {reason}")), true);
            }
        }
    }

    /// Un poste se connecte : il est ajouté aux postes de sa salle s'il est
    /// nouveau (SALLE-3), et relié à la séance ouverte dans sa salle
    /// (REJ-1, RESTR-2).
    fn agent_online(&mut self, a: AgentPresence, now: u64) {
        if let Some(room) = a.room.as_ref().and_then(|r| self.rooms.iter_mut().find(|x| &x.id == r)) {
            let mut changed = false;
            match room.stations.iter_mut().find(|s| s.member == a.member) {
                Some(s) if s.label != a.info.label => {
                    s.label = a.info.label.clone();
                    changed = true;
                }
                Some(_) => {}
                None => {
                    let number = room.stations.iter().map(|s| s.number).max().unwrap_or(0) + 1;
                    room.stations.push(Station { member: a.member.clone(), number, label: a.info.label.clone(), pos: None });
                    changed = true;
                }
            }
            if changed {
                self.save_rooms();
            }
        }
        let member = a.member.clone();
        let in_room = self.session.as_ref().is_some_and(|s| a.room.as_ref() == Some(&s.room));
        // Retour d'un poste qui avait perdu la connexion pendant la séance.
        if let Some(e) = self.alerts.iter_mut().find(|e| e.member == member && e.lost == Some(None)) {
            e.lost = Some(Some(now));
        }
        self.online.insert(member.clone(), Online { presence: a, session: None, screen: ScreenState::Off, web: WebState::Off });
        if in_room {
            let to = Target::Members(vec![member.clone()]);
            self.send_session(to.clone(), now);
            if self.session.as_ref().is_some_and(|s| s.screens) {
                self.send(to.clone(), Command::Screens { on: true });
            }
            // Le poste réapplique les restrictions de la séance (RESTR-3).
            if let Some(cmd) = self.restrict_cmd(&member, now) {
                self.send(to, cmd);
            }
        }
    }

    fn agent_status(&mut self, from: &str, session: Option<SessionId>, screen: ScreenState, web: WebState, now: u64) {
        let Some(o) = self.online.get_mut(from) else { return };
        o.session = session.clone();
        o.screen = screen;
        let escaped = web == WebState::Escaped && o.web != WebState::Escaped;
        o.web = web;
        // L'élève a coupé le filtrage de sa session : la console le dit
        // plutôt que de le masquer (MODE-3).
        if escaped {
            let name = self.station_name(from);
            self.alert(now, from, false);
            self.notice(
                format!("{} : sorti du filtrage web", capitalize(&name)),
                Some("Le réglage posé par Surveo a été modifié sur le poste. Réappliquez le filtrage pour le rétablir.".into()),
                true,
            );
        }
        let current = self.session.as_ref().map(|s| s.id.clone());
        match &session {
            Some(sid) if Some(sid) == current.as_ref() => {
                let s = self.session.as_mut().expect("séance ouverte");
                if s.members.insert(from.to_string()) {
                    self.save_session();
                }
            }
            // Séance terminée pendant que le poste était injoignable : il y
            // revient maintenant à son état d'origine.
            Some(sid) => {
                let sid = sid.clone();
                self.send(Target::Members(vec![from.to_string()]), Command::EndSession { session: sid });
            }
            None => {}
        }
        if let Some(c) = &mut self.closure {
            if session.as_ref() != Some(&c.session) {
                if let Some(e) = c.entries.get_mut(from) {
                    *e = Restore::Restored;
                }
            }
        }
    }

    // --- Salles et plans (SALLE-1 à SALLE-3) ----------------------------

    pub fn create_room(&mut self, name: &str, cols: u8, rows: u8) -> Result<RoomId> {
        let name = name.trim();
        ensure!(!name.is_empty(), "Indiquez le nom de la salle.");
        ensure!(!self.rooms.iter().any(|r| r.name.eq_ignore_ascii_case(name)), "Une salle porte déjà ce nom.");
        ensure!((1..=12).contains(&cols) && (1..=10).contains(&rows), "Le plan compte de 1 à 12 colonnes et de 1 à 10 rangées.");
        let id = random_id();
        self.rooms.push(Room { id: id.clone(), name: name.into(), cols, rows, stations: vec![] });
        self.room = Some(id.clone());
        self.save_rooms();
        Ok(id)
    }

    pub fn select_room(&mut self, id: &str) -> Result<()> {
        ensure!(self.rooms.iter().any(|r| r.id == id), "Salle inconnue.");
        self.room = Some(id.into());
        Ok(())
    }

    /// Change la taille du plan ; les postes qui n'y tiennent plus
    /// retournent dans « postes à placer ».
    pub fn resize_room(&mut self, cols: u8, rows: u8) -> Result<()> {
        ensure!((1..=12).contains(&cols) && (1..=10).contains(&rows), "Le plan compte de 1 à 12 colonnes et de 1 à 10 rangées.");
        let room = self.current_room_mut()?;
        room.cols = cols;
        room.rows = rows;
        for s in &mut room.stations {
            if s.pos.is_some_and(|p| p.col >= cols || p.row >= rows) {
                s.pos = None;
            }
        }
        self.save_rooms();
        Ok(())
    }

    /// Place un poste sur le plan (SALLE-3). Si la place est prise, les
    /// deux postes échangent leurs places.
    pub fn place(&mut self, member: &str, pos: Pos) -> Result<()> {
        let room = self.current_room_mut()?;
        ensure!(pos.col < room.cols && pos.row < room.rows, "Place hors du plan.");
        let old = room.stations.iter().find(|s| s.member == member).ok_or_else(|| anyhow!("Poste inconnu dans cette salle."))?.pos;
        for s in &mut room.stations {
            if s.member == member {
                s.pos = Some(pos);
            } else if s.pos == Some(pos) {
                s.pos = old;
            }
        }
        self.save_rooms();
        Ok(())
    }

    pub fn unplace(&mut self, member: &str) -> Result<()> {
        let room = self.current_room_mut()?;
        let s = room.stations.iter_mut().find(|s| s.member == member).ok_or_else(|| anyhow!("Poste inconnu dans cette salle."))?;
        s.pos = None;
        self.save_rooms();
        Ok(())
    }

    fn current_room_mut(&mut self) -> Result<&mut Room> {
        let id = self.room.clone().ok_or_else(|| anyhow!("Choisissez d'abord une salle."))?;
        self.rooms.iter_mut().find(|r| r.id == id).ok_or_else(|| anyhow!("Salle inconnue."))
    }

    fn save_rooms(&mut self) {
        if let Err(e) = self.store.save_rooms(&self.rooms) {
            log::error!("enregistrement des salles impossible : {e:#}");
            self.notice("Le plan n'a pas pu être enregistré", Some(format!("{e:#}")), true);
        }
    }

    // --- Séance (SEANCE-1 à SEANCE-6) -----------------------------------

    /// Ouvre une séance dans la salle choisie, jusqu'à `end_ms` (SEANCE-1).
    pub fn open_session(&mut self, end_ms: u64, now: u64) -> Result<()> {
        let teacher = self.profile.as_ref().map(|p| p.teacher.clone()).ok_or_else(|| anyhow!("Console non configurée."))?;
        if let Some(s) = &self.session {
            bail!("Une séance est déjà ouverte en {}.", self.room_name(&s.room));
        }
        let room = self.room.clone().ok_or_else(|| anyhow!("Choisissez d'abord une salle."))?;
        ensure!(end_ms > now + 60_000, "L'heure de fin doit être dans le futur.");
        ensure!(
            end_ms <= now + MAX_DURATION_MS,
            "Une séance ne peut pas dépasser {} h, prolongations comprises.",
            MAX_DURATION_MS / 3_600_000
        );
        self.session = Some(Session {
            id: random_id(),
            room: room.clone(),
            teacher,
            opened_at_ms: now,
            end_ms,
            screens: false,
            members: BTreeSet::new(),
            restrictions: SessionRestrictions::default(),
        });
        self.closure = None;
        self.alerts.clear();
        self.undo = None;
        self.save_session();
        self.send_session(Target::Room(room), now);
        Ok(())
    }

    /// Nouvelle heure de fin : prolongation (SEANCE-3) ou retour en arrière
    /// par « Annuler » (RETOUR-3), dans la limite de la durée maximale
    /// (SEANCE-4).
    pub fn set_end(&mut self, end_ms: u64, now: u64) -> Result<()> {
        let s = self.session.as_mut().ok_or_else(|| anyhow!("Aucune séance ouverte."))?;
        ensure!(end_ms > now, "L'heure de fin doit être dans le futur.");
        ensure!(
            end_ms <= s.opened_at_ms + MAX_DURATION_MS,
            "Une séance ne peut pas dépasser {} h, prolongations comprises.",
            MAX_DURATION_MS / 3_600_000
        );
        s.end_ms = end_ms;
        // Le mode examen s'arrête au plus tard à la fin de la séance (EXAM-3).
        let clamped = match &mut s.restrictions.exam {
            Some(e) if e.end_ms > end_ms => {
                e.end_ms = end_ms;
                true
            }
            _ => false,
        };
        let room = s.room.clone();
        self.save_session();
        self.send_session(Target::Room(room), now);
        if clamped {
            self.resend_restrictions(now);
        }
        Ok(())
    }

    /// Clôture (SEANCE-5) ou fin d'elle-même (SEANCE-6) : les postes
    /// reviennent à leur état d'origine ; le récapitulatif dit lesquels
    /// l'ont confirmé et lesquels étaient injoignables (RESTR-5).
    pub fn close_session(&mut self, now: u64, auto: bool) -> Result<()> {
        let s = self.session.take().ok_or_else(|| anyhow!("Aucune séance ouverte."))?;
        self.save_session();
        let mut members = s.members.clone();
        members.extend(self.online.iter().filter(|(_, o)| o.session.as_ref() == Some(&s.id)).map(|(m, _)| m.clone()));
        let entries = members
            .into_iter()
            .map(|m| {
                let state = if self.online.contains_key(&m) { Restore::Pending } else { Restore::Unreachable };
                (m, state)
            })
            .collect();
        self.send(Target::Room(s.room.clone()), Command::EndSession { session: s.id.clone() });
        self.closure = Some(Closure {
            session: s.id,
            ended_at_ms: now,
            auto,
            restore_by_ms: s.end_ms + MARGIN_MS,
            deadline_ms: now + ACK_TIMEOUT_MS,
            entries,
        });
        if auto {
            self.notice("La séance s'est terminée d'elle-même", Some("L'heure de fin prévue et sa marge sont passées.".into()), false);
        }
        Ok(())
    }

    /// Afficher ou masquer les écrans (AFF-4).
    pub fn set_screens(&mut self, on: bool) -> Result<()> {
        let s = self.session.as_mut().ok_or_else(|| anyhow!("Les écrans ne s'affichent que pendant une séance."))?;
        s.screens = on;
        let room = s.room.clone();
        self.save_session();
        self.send(Target::Room(room), Command::Screens { on });
        Ok(())
    }

    fn send_session(&mut self, to: Target, now: u64) {
        let Some(s) = &self.session else { return };
        let info = SessionInfo {
            id: s.id.clone(),
            room: Some(s.room.clone()),
            teacher: s.teacher.clone(),
            remaining_ms: s.end_ms.saturating_sub(now),
            margin_ms: MARGIN_MS,
        };
        self.send(to, Command::Session { session: info });
    }

    fn save_session(&mut self) {
        if let Err(e) = self.store.save_session(self.session.as_ref()) {
            log::error!("enregistrement de la séance impossible : {e:#}");
        }
    }

    // --- Restrictions (SPEC.md, chap. 2) -----------------------------------

    /// Restrictions d'un poste dans la séance, telles que le poste les
    /// reçoit : la valeur de sa sélection, sinon celle de la salle.
    fn restrictions_for(r: &SessionRestrictions, member: &str, now: u64) -> Restrictions {
        Restrictions {
            pause: r.pause.get(member).clone(),
            web: r.web.get(member).clone(),
            banned_apps: r.apps.get(member).clone(),
            exam: r.exam.as_ref().filter(|e| e.end_ms > now).map(|e| Exam { remaining_ms: e.end_ms - now, sites: e.sites.clone() }),
        }
    }

    fn restrict_cmd(&self, member: &str, now: u64) -> Option<Command> {
        let s = self.session.as_ref()?;
        Some(Command::Restrict { session: s.id.clone(), restrictions: Self::restrictions_for(&s.restrictions, member, now) })
    }

    /// Postes visés par une action de la séance : la sélection, ou sans
    /// sélection les postes de la salle présents ou entrés dans la séance
    /// (SEL-2).
    fn session_targets(&self, members: &[MemberId]) -> Vec<MemberId> {
        let Some(s) = &self.session else { return vec![] };
        if !members.is_empty() {
            return members.to_vec();
        }
        let mut all: BTreeSet<MemberId> = s.members.clone();
        all.extend(self.online.iter().filter(|(_, o)| o.presence.room.as_ref() == Some(&s.room)).map(|(m, _)| m.clone()));
        all.into_iter().collect()
    }

    /// Applique un changement des restrictions de la séance, l'envoie aux
    /// postes concernés et le consigne (MAIT-2, MAIT-4) ; l'action peut
    /// être annulée (RETOUR-3).
    fn restrict(&mut self, label: String, members: &[MemberId], now: u64, change: impl FnOnce(&mut SessionRestrictions)) -> Result<()> {
        let s = self.session.as_mut().ok_or_else(|| anyhow!("Les restrictions ne s'appliquent que pendant une séance."))?;
        let before = s.restrictions.clone();
        change(&mut s.restrictions);
        let undo = self.next_id;
        self.next_id += 1;
        self.undo = Some((undo, before));
        self.save_session();
        self.journal(&label, members, now);
        let targets = self.session_targets(members);
        self.send_restrictions(label, targets, now, Some(undo));
        Ok(())
    }

    fn send_restrictions(&mut self, label: String, targets: Vec<MemberId>, now: u64, undo: Option<u64>) {
        let mut p = Pending::new(label, now + ACK_TIMEOUT_MS);
        p.deferred = true;
        p.undo = undo;
        for m in targets {
            match self.restrict_cmd(&m, now) {
                Some(cmd) if self.online.contains_key(&m) => {
                    let id = self.send(Target::Members(vec![m.clone()]), cmd);
                    p.ids.insert(id, m);
                }
                _ => p.offline.push(m),
            }
        }
        self.pending.push(p);
    }

    /// Renvoie leurs restrictions à tous les postes de la séance, sans
    /// retour à l'enseignant.
    fn resend_restrictions(&mut self, now: u64) {
        for m in self.session_targets(&[]) {
            if self.online.contains_key(&m) {
                if let Some(cmd) = self.restrict_cmd(&m, now) {
                    self.send(Target::Members(vec![m]), cmd);
                }
            }
        }
    }

    fn journal(&mut self, action: &str, members: &[MemberId], now: u64) {
        let Some(s) = &self.session else { return };
        let target = if members.is_empty() {
            "toute la salle".to_string()
        } else {
            members.iter().map(|m| self.station_name(m)).collect::<Vec<_>>().join(", ")
        };
        let e = JournalEntry { at_ms: now, teacher: s.teacher.clone(), room: self.room_name(&s.room), session: s.id.clone(), target, action: action.into() };
        if let Err(err) = self.store.append_journal(&e) {
            log::error!("journal des restrictions impossible à écrire : {err:#}");
        }
    }

    /// Met en pause l'écran des postes, avec un message, ou la lève
    /// (`None`) (ECRAN-1 à ECRAN-3). Un message vide donne le message par
    /// défaut (ECRAN-2).
    pub fn set_pause(&mut self, members: &[MemberId], message: Option<String>, now: u64) -> Result<()> {
        let teacher = self.session.as_ref().map(|s| s.teacher.clone()).unwrap_or_default();
        let message = message.map(|m| m.trim().to_string()).map(|m| if m.is_empty() { format!("Écran mis en pause par {teacher}.") } else { m });
        let label = if message.is_some() { "Écran mis en pause" } else { "Écran repris" };
        self.restrict(label.into(), members, now, |r| r.pause.set(members, message))
    }

    /// Filtrage web des postes, ou sa levée (`None`) (WEB-1, WEB-3).
    pub fn set_web(&mut self, members: &[MemberId], policy: Option<WebPolicy>, now: u64) -> Result<()> {
        let policy = match policy {
            Some(WebPolicy::Allow { sites }) => Some(WebPolicy::Allow { sites: sites_list(&sites)? }),
            Some(WebPolicy::Block { sites }) => Some(WebPolicy::Block { sites: sites_list(&sites)? }),
            other => other,
        };
        let label = match &policy {
            None => "Filtrage web levé".to_string(),
            Some(WebPolicy::Allow { sites }) => format!("Filtrage web : liste d'autorisation ({})", sites_text(sites)),
            Some(WebPolicy::Block { sites }) => format!("Filtrage web : liste de blocage ({})", sites_text(sites)),
            Some(WebPolicy::All) => "Filtrage web : tout bloquer".to_string(),
        };
        self.restrict(label, members, now, |r| r.web.set(members, policy))
    }

    /// Applications interdites des postes ; une liste vide lève
    /// l'interdiction (APP-2, APP-5).
    pub fn set_banned_apps(&mut self, members: &[MemberId], apps: Vec<String>, now: u64) -> Result<()> {
        let apps = names_list(&apps);
        let label = if apps.is_empty() { "Applications interdites levées".to_string() } else { format!("Applications interdites : {}", apps.join(", ")) };
        self.restrict(label, members, now, |r| r.apps.set(members, apps))
    }

    /// Passe la séance en mode examen jusqu'à `end_ms`, avec sa liste
    /// d'autorisation, ou en sort (`None`) (EXAM-1 à EXAM-3).
    pub fn set_exam(&mut self, exam: Option<(u64, Vec<String>)>, now: u64) -> Result<()> {
        let s = self.session.as_ref().ok_or_else(|| anyhow!("Le mode examen n'existe que pendant une séance."))?;
        let exam = match exam {
            Some((end_ms, sites)) => {
                ensure!(end_ms > now + 60_000, "L'heure de fin de l'examen doit être dans le futur.");
                ensure!(end_ms <= s.end_ms, "Le mode examen s'arrête au plus tard à la fin de la séance.");
                let sites: Vec<String> = sites.iter().map(|x| normalize_site(x).ok_or_else(|| anyhow!("« {} » n'est pas un nom de site.", x.trim()))).collect::<Result<_>>()?;
                Some(ExamState { end_ms, sites: dedup(sites) })
            }
            None => None,
        };
        let label = match (&exam, s.restrictions.exam.is_some()) {
            (None, _) => "Mode examen désactivé",
            (Some(_), false) => "Mode examen activé",
            (Some(_), true) => "Mode examen modifié",
        };
        self.restrict(label.into(), &[], now, |r| r.exam = exam)
    }

    /// « Annuler » la dernière restriction (RETOUR-3).
    pub fn undo(&mut self, id: u64, now: u64) -> Result<()> {
        let (last, before) = self.undo.take().ok_or_else(|| anyhow!("Il n'y a plus rien à annuler."))?;
        if last != id {
            self.undo = Some((last, before));
            bail!("Une autre action a eu lieu depuis : celle-ci ne peut plus être annulée.");
        }
        let s = self.session.as_mut().ok_or_else(|| anyhow!("Aucune séance ouverte."))?;
        s.restrictions = before;
        self.save_session();
        self.journal("Annulation de la dernière restriction", &[], now);
        let targets = self.session_targets(&[]);
        self.send_restrictions("Action annulée".into(), targets, now, None);
        Ok(())
    }

    /// Demande à un poste ses applications ouvertes (APP-1).
    pub fn list_apps(&mut self, member: &str) -> Result<()> {
        ensure!(self.online.get(member).is_some_and(|o| o.session.is_some()), "Ce poste n'est pas dans la séance.");
        self.send(Target::Members(vec![member.into()]), Command::ListApps);
        Ok(())
    }

    /// Ferme une application d'un poste ; l'élève en est prévenu (APP-1).
    pub fn close_app(&mut self, member: &str, app: &str, now: u64) -> Result<()> {
        ensure!(self.online.get(member).is_some_and(|o| o.session.is_some()), "Ce poste n'est pas dans la séance.");
        let name = self.apps.get(member).and_then(|l| l.iter().find(|a| a.id == app)).map(|a| a.name.clone()).unwrap_or_else(|| app.into());
        let station = self.station_name(member);
        self.journal(&format!("Application fermée : {name}"), &[member.to_string()], now);
        let mut p = Pending::new(format!("Fermer {name}"), now + ACK_TIMEOUT_MS);
        p.done_text = Some(format!("{name} fermée sur le {station}"));
        let id = self.send(Target::Members(vec![member.into()]), Command::CloseApp { app: app.into() });
        p.ids.insert(id, member.into());
        self.pending.push(p);
        if let Some(l) = self.apps.get_mut(member) {
            l.retain(|a| a.id != app);
        }
        Ok(())
    }

    // --- Listes de l'enseignant (WEB-4, APP-4) -----------------------------

    /// Enregistre une liste de sites ; une liste du même nom est remplacée.
    pub fn save_site_list(&mut self, name: &str, sites: &[String]) -> Result<()> {
        let name = name.trim();
        ensure!(!name.is_empty(), "Donnez un nom à la liste.");
        let sites = sites_list(sites)?;
        self.lists.sites.retain(|l| l.name.to_lowercase() != name.to_lowercase());
        self.lists.sites.push(SiteList { name: name.into(), sites });
        self.lists.sites.sort_by_key(|l| l.name.to_lowercase());
        self.save_lists();
        Ok(())
    }

    pub fn delete_site_list(&mut self, name: &str) {
        self.lists.sites.retain(|l| l.name != name);
        self.save_lists();
    }

    pub fn save_app_list(&mut self, name: &str, apps: &[String]) -> Result<()> {
        let name = name.trim();
        ensure!(!name.is_empty(), "Donnez un nom à la liste.");
        let apps = names_list(apps);
        ensure!(!apps.is_empty(), "La liste ne nomme aucune application.");
        self.lists.apps.retain(|l| l.name.to_lowercase() != name.to_lowercase());
        self.lists.apps.push(AppList { name: name.into(), apps });
        self.lists.apps.sort_by_key(|l| l.name.to_lowercase());
        self.save_lists();
        Ok(())
    }

    pub fn delete_app_list(&mut self, name: &str) {
        self.lists.apps.retain(|l| l.name != name);
        self.save_lists();
    }

    fn save_lists(&mut self) {
        if let Err(e) = self.store.save_lists(&self.lists) {
            log::error!("enregistrement des listes impossible : {e:#}");
            self.notice("Les listes n'ont pas pu être enregistrées", Some(format!("{e:#}")), true);
        }
    }

    // --- Actions sur les postes ------------------------------------------

    /// « Identifier » (SALLE-4) : chaque poste affiche le numéro de sa
    /// tuile. Sans sélection, toute la salle.
    pub fn identify(&mut self, members: &[MemberId], now: u64) -> Result<()> {
        let room = self.current_room_mut()?;
        let targets: Vec<(MemberId, u16)> = room
            .stations
            .iter()
            .filter(|s| members.is_empty() || members.contains(&s.member))
            .map(|s| (s.member.clone(), s.number))
            .collect();
        ensure!(!targets.is_empty(), "Aucun poste dans cette salle.");
        let mut p = Pending::new("Identifier", now + ACK_TIMEOUT_MS);
        for (member, number) in targets {
            if self.online.contains_key(&member) {
                let id = self.send(
                    Target::Members(vec![member.clone()]),
                    Command::Identify { number: format!("{number:02}"), seconds: IDENTIFY_SECONDS },
                );
                p.ids.insert(id, member);
            } else {
                p.offline.push(member);
            }
        }
        self.pending.push(p);
        Ok(())
    }

    /// Échéances : fin de la séance d'elle-même (SEANCE-6), retours
    /// d'action (RETOUR-1), postes injoignables à la clôture (RESTR-5).
    /// Rend `true` si l'affichage a changé.
    pub fn tick(&mut self, now: u64) -> bool {
        let mut changed = false;
        if self.session.as_ref().is_some_and(|s| now >= s.end_ms + MARGIN_MS) {
            let _ = self.close_session(now, true);
            changed = true;
        }
        // Fin du mode examen à son heure (EXAM-3) ; les postes la
        // connaissent aussi et n'attendent pas la console.
        if let Some(s) = self.session.as_mut() {
            if s.restrictions.exam.as_ref().is_some_and(|e| now >= e.end_ms) {
                s.restrictions.exam = None;
                self.save_session();
                self.resend_restrictions(now);
                self.notice("Le mode examen est terminé", Some("Son heure de fin est atteinte.".into()), false);
                changed = true;
            }
        }
        let (done, waiting): (Vec<_>, Vec<_>) = std::mem::take(&mut self.pending)
            .into_iter()
            .partition(|p| now >= p.deadline_ms || p.ids.values().all(|m| p.acked.contains(m) || p.refused.iter().any(|(r, _)| r == m)));
        self.pending = waiting;
        for p in done {
            let n = self.report(&p);
            self.notices.push(n);
            changed = true;
        }
        if let Some(c) = &mut self.closure {
            if now >= c.deadline_ms && c.entries.values().any(|e| *e == Restore::Pending) {
                for e in c.entries.values_mut() {
                    if *e == Restore::Pending {
                        *e = Restore::Unreachable;
                    }
                }
                changed = true;
            }
        }
        changed
    }

    /// Retour poste par poste d'une action (RETOUR-1) :
    /// « Envoyé à 23 postes · 1 injoignable (poste 07) ».
    fn report(&self, p: &Pending) -> Notice {
        let unreachable: Vec<&MemberId> = p
            .offline
            .iter()
            .chain(p.ids.values().filter(|m| !p.acked.contains(*m) && !p.refused.iter().any(|(r, _)| r == *m)))
            .collect();
        let detail = (!p.refused.is_empty()).then(|| {
            p.refused.iter().map(|(m, why)| format!("{} : {why}", self.station_name(m))).collect::<Vec<_>>().join(" · ")
        });
        let warning = !unreachable.is_empty() || !p.refused.is_empty();
        if let (Some(done), false) = (&p.done_text, warning) {
            return Notice { text: done.clone(), detail: None, warning, undo: p.undo };
        }
        let mut text = format!("{} : envoyé à {}", p.label, plural(p.acked.len(), "poste"));
        if !unreachable.is_empty() {
            let mut names: Vec<String> = unreachable.iter().map(|m| self.station_name(m)).collect();
            names.sort();
            if p.deferred {
                text += &format!(" · {} en attente ({}), appliqué à son retour", unreachable.len(), names.join(", "));
            } else {
                text += &format!(" · {} ({})", plural(unreachable.len(), "injoignable"), names.join(", "));
            }
        }
        Notice { text, detail, warning, undo: p.undo }
    }

    /// « poste 07 », ou le libellé d'un poste hors des salles connues.
    fn station_name(&self, member: &str) -> String {
        self.rooms
            .iter()
            .flat_map(|r| &r.stations)
            .find(|s| s.member == member)
            .map(|s| format!("poste {:02}", s.number))
            .unwrap_or_else(|| member.to_string())
    }

    fn room_name(&self, id: &str) -> String {
        self.rooms.iter().find(|r| r.id == id).map(|r| r.name.clone()).unwrap_or_else(|| id.into())
    }

    /// Paquet de rattachement d'une salle (INST-3), avec les adresses
    /// actuelles du relais intégré.
    pub fn package(&self, room_id: &str) -> Result<Package> {
        let etab = self.etab.as_ref().ok_or_else(|| anyhow!("Console non configurée."))?;
        let room = self.rooms.iter().find(|r| r.id == room_id).ok_or_else(|| anyhow!("Salle inconnue."))?;
        let relays = if self.relay.running {
            self.relay.addresses.iter().map(|a| format!("{a}:{}", self.relay.port)).collect()
        } else {
            vec![]
        };
        Ok(etab.package(&room.id, &room.name, PosteKind::Salle, relays))
    }

    pub fn room_name_of(&self, room_id: &str) -> Option<String> {
        self.rooms.iter().find(|r| r.id == room_id).map(|r| r.name.clone())
    }

    // --- Vue -------------------------------------------------------------

    pub fn view(&self) -> ConsoleView {
        let setup = match (&self.etab, &self.profile) {
            (Some(e), Some(p)) => Some(SetupView { etab_name: e.public.name.clone(), teacher: p.teacher.clone() }),
            _ => None,
        };
        let session_id = self.session.as_ref().map(|s| s.id.clone());
        let room = self.room.as_ref().and_then(|id| self.rooms.iter().find(|r| &r.id == id)).map(|r| RoomView {
            id: r.id.clone(),
            name: r.name.clone(),
            cols: r.cols,
            rows: r.rows,
            stations: r
                .stations
                .iter()
                .map(|s| {
                    let o = self.online.get(&s.member);
                    let state = match o {
                        None => StationState::Off,
                        Some(o) if o.session.is_some() && o.session == session_id => StationState::InSession,
                        Some(_) => StationState::On,
                    };
                    let r = self.session.as_ref().filter(|x| x.room == r.id).map(|x| &x.restrictions);
                    StationView {
                        member: s.member.clone(),
                        number: format!("{:02}", s.number),
                        label: s.label.clone(),
                        pos: s.pos,
                        state,
                        screen: o.map(|o| o.screen.clone()).unwrap_or_default(),
                        os: o.map(|o| o.presence.info.os.clone()),
                        kind: o.map(|o| o.presence.info.kind),
                        mode: o.map(|o| o.presence.info.mode),
                        pause: r.and_then(|r| r.pause.get(&s.member).clone()),
                        web: r.and_then(|r| r.web.get(&s.member).clone()),
                        banned_apps: r.map(|r| r.apps.get(&s.member).clone()).unwrap_or_default(),
                        web_state: o.map(|o| o.web.clone()).unwrap_or_default(),
                        apps: self.apps.get(&s.member).cloned(),
                    }
                })
                .collect(),
        });
        let session = self.session.as_ref().map(|s| SessionView {
            id: s.id.clone(),
            room_id: s.room.clone(),
            room_name: self.room_name(&s.room),
            teacher: s.teacher.clone(),
            opened_at_ms: s.opened_at_ms,
            end_ms: s.end_ms,
            max_end_ms: s.opened_at_ms + MAX_DURATION_MS,
            margin_ms: MARGIN_MS,
            screens_on: s.screens,
            pause_all: s.restrictions.pause.all.clone(),
            web_all: s.restrictions.web.all.clone(),
            apps_all: s.restrictions.apps.all.clone(),
            exam: s.restrictions.exam.as_ref().map(|e| ExamView { end_ms: e.end_ms, sites: e.sites.clone() }),
        });
        let closure = self.closure.as_ref().map(|c| {
            let names = |st: Restore| -> Vec<String> {
                c.entries.iter().filter(|(_, e)| **e == st).map(|(m, _)| self.station_name(m)).collect()
            };
            ClosureView {
                ended_at_ms: c.ended_at_ms,
                auto: c.auto,
                restore_by_ms: c.restore_by_ms,
                restored: names(Restore::Restored),
                pending: names(Restore::Pending),
                unreachable: names(Restore::Unreachable),
            }
        });
        ConsoleView {
            setup,
            relay: self.relay.clone(),
            rooms: self
                .rooms
                .iter()
                .map(|r| RoomSummary { id: r.id.clone(), name: r.name.clone(), stations: r.stations.len() as u32 })
                .collect(),
            room,
            session,
            closure,
            refused: self.refused.iter().cloned().collect(),
            alerts: self
                .alerts
                .iter()
                .map(|e| {
                    let name = self.station_name(&e.member);
                    let (until_ms, text) = match e.lost {
                        None => (None, format!("{name} : le filtrage web a été coupé sur le poste.")),
                        Some(None) => (None, format!("{name} : connexion perdue. Si l'agent a été arrêté, ses restrictions ne s'appliquent plus.")),
                        Some(Some(t)) => (
                            Some(t),
                            format!("{name} : connexion perdue, puis revenu. Si l'agent avait été arrêté, ses restrictions ne s'appliquaient plus entre-temps."),
                        ),
                    };
                    Alert { at_ms: e.at_ms, until_ms, text }
                })
                .collect(),
            lists: self.lists.clone(),
            limits: Limits {
                default_duration_ms: DEFAULT_DURATION_MS,
                max_duration_ms: MAX_DURATION_MS,
                margin_ms: MARGIN_MS,
                extend_step_ms: EXTEND_STEP_MS,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[derive(Clone, Default)]
    struct Recorder(Arc<Mutex<Vec<FromConsole>>>);

    impl Link for Recorder {
        fn send(&self, m: &FromConsole) {
            self.0.lock().unwrap().push(m.clone());
        }
    }

    impl Recorder {
        fn take(&self) -> Vec<(Target, Command)> {
            self.0.lock().unwrap().drain(..).map(|FromConsole::Send { to, cmd, .. }| (to, cmd)).collect()
        }
        fn ids(&self) -> Vec<(u64, Command)> {
            self.0.lock().unwrap().drain(..).map(|FromConsole::Send { id, cmd, .. }| (id, cmd)).collect()
        }
    }

    const T0: u64 = 1_800_000_000_000;

    fn engine() -> (Engine, Recorder, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!("surveo-console-test-{}", random_id()));
        let mut e = Engine::new(Store::new(dir.clone()), T0).unwrap();
        e.create_establishment("Lycée test", "M. Durand").unwrap();
        e.create_room("B204", 6, 5).unwrap();
        let rec = Recorder::default();
        e.attach(Box::new(rec.clone()));
        (e, rec, dir)
    }

    fn presence(e: &Engine, member: &str) -> AgentPresence {
        AgentPresence {
            member: member.into(),
            room: e.room.clone(),
            info: AgentInfo { label: format!("PC-{member}"), os: "Linux".into(), version: "0".into(), kind: PosteKind::Salle, mode: DeployMode::SansAdmin },
        }
    }

    fn status(e: &mut Engine, member: &str, session: Option<&str>) {
        let msg = AgentMsg::Status { session: session.map(Into::into), screen: ScreenState::Off, web: WebState::Off };
        e.handle(ToConsole::FromAgent { from: member.into(), msg }, T0);
    }

    #[test]
    fn new_stations_are_numbered_and_wait_to_be_placed() {
        let (mut e, _, _) = engine();
        for m in ["a", "b"] {
            let p = presence(&e, m);
            e.handle(ToConsole::Online { agent: p }, T0);
        }
        let v = e.view().room.unwrap();
        assert_eq!(v.stations.iter().map(|s| (s.number.as_str(), s.pos, s.state)).collect::<Vec<_>>(), vec![
            ("01", None, StationState::On),
            ("02", None, StationState::On)
        ]);
        e.place("a", Pos { col: 0, row: 0 }).unwrap();
        e.place("b", Pos { col: 0, row: 0 }).unwrap();
        let v = e.view().room.unwrap();
        assert_eq!((v.stations[0].pos, v.stations[1].pos), (None, Some(Pos { col: 0, row: 0 })));

        // Le plan est enregistré avec la salle (SALLE-2) ; un poste déconnecté
        // reste connu, éteint (SALLE-1).
        e.handle(ToConsole::Offline { member: "b".into() }, T0);
        let again = Engine::new(Store::new(e.store.dir.clone()), T0).unwrap();
        let v = again.view().room.unwrap();
        assert_eq!((v.stations[1].pos, v.stations[1].state), (Some(Pos { col: 0, row: 0 }), StationState::Off));
    }

    #[test]
    fn session_reaches_present_and_returning_stations() {
        let (mut e, rec, _) = engine();
        let p = presence(&e, "a");
        e.handle(ToConsole::Online { agent: p }, T0);
        e.open_session(T0 + 30 * 60_000, T0).unwrap();
        assert!(e.open_session(T0 + 60 * 60_000, T0).is_err(), "une seule séance à la fois");
        match &rec.take()[..] {
            [(Target::Room(_), Command::Session { session })] => {
                assert_eq!((session.remaining_ms, session.margin_ms, session.teacher.as_str()), (30 * 60_000, MARGIN_MS, "M. Durand"));
            }
            other => panic!("{other:?}"),
        }
        let sid = e.session.clone().unwrap().id;
        status(&mut e, "a", Some(&sid));
        assert_eq!(e.view().room.unwrap().stations[0].state, StationState::InSession);

        // Un poste qui arrive pendant la séance la rejoint (REJ-1), avec les
        // écrans s'ils sont affichés.
        e.set_screens(true).unwrap();
        rec.take();
        let p = presence(&e, "b");
        e.handle(ToConsole::Online { agent: p }, T0 + 60_000);
        let sent = rec.take();
        assert!(matches!(&sent[0], (Target::Members(m), Command::Session { session }) if m == &["b".to_string()] && session.remaining_ms == 29 * 60_000));
        assert!(matches!(&sent[1], (_, Command::Screens { on: true })));
    }

    #[test]
    fn extension_is_capped_by_the_maximum_duration() {
        let (mut e, _, _) = engine();
        e.open_session(T0 + DEFAULT_DURATION_MS, T0).unwrap();
        e.set_end(T0 + 2 * DEFAULT_DURATION_MS, T0 + 1000).unwrap();
        let err = e.set_end(T0 + MAX_DURATION_MS + 1, T0 + 1000).unwrap_err().to_string();
        assert!(err.contains("4 h"), "{err}");
        assert_eq!(e.view().session.unwrap().end_ms, T0 + 2 * DEFAULT_DURATION_MS);
    }

    #[test]
    fn closing_reports_restored_and_unreachable_stations() {
        let (mut e, rec, _) = engine();
        for m in ["a", "b"] {
            let p = presence(&e, m);
            e.handle(ToConsole::Online { agent: p }, T0);
        }
        e.open_session(T0 + DEFAULT_DURATION_MS, T0).unwrap();
        let sid = e.session.clone().unwrap().id;
        status(&mut e, "a", Some(&sid));
        status(&mut e, "b", Some(&sid));
        e.handle(ToConsole::Offline { member: "b".into() }, T0);
        rec.take();

        e.close_session(T0 + 1000, false).unwrap();
        assert!(matches!(&rec.take()[..], [(Target::Room(_), Command::EndSession { session })] if *session == sid));
        let c = e.view().closure.unwrap();
        assert_eq!((c.pending, c.unreachable.clone()), (vec!["poste 01".to_string()], vec!["poste 02".to_string()]));
        assert_eq!(c.restore_by_ms, T0 + DEFAULT_DURATION_MS + MARGIN_MS);

        status(&mut e, "a", None);
        assert_eq!(e.view().closure.unwrap().restored, vec!["poste 01"]);

        // Le poste injoignable revient encore dans la séance close : il est
        // remis en état, et le récapitulatif le note.
        let p = presence(&e, "b");
        e.handle(ToConsole::Online { agent: p }, T0 + 5000);
        status(&mut e, "b", Some(&sid));
        assert!(matches!(&rec.take()[..], [(Target::Members(_), Command::EndSession { .. })]));
        status(&mut e, "b", None);
        assert_eq!(e.view().closure.unwrap().restored, vec!["poste 01", "poste 02"]);
    }

    #[test]
    fn session_ends_by_itself_after_the_margin() {
        let (mut e, rec, _) = engine();
        e.open_session(T0 + DEFAULT_DURATION_MS, T0).unwrap();
        rec.take();
        assert!(!e.tick(T0 + DEFAULT_DURATION_MS + MARGIN_MS - 1));
        assert!(e.view().session.is_some());
        assert!(e.tick(T0 + DEFAULT_DURATION_MS + MARGIN_MS));
        assert!(e.view().session.is_none());
        assert!(e.view().closure.unwrap().auto);
        assert!(matches!(&rec.take()[..], [(_, Command::EndSession { .. })]));
        // Une console relancée après l'échéance ne reprend pas la séance.
        let again = Engine::new(Store::new(e.store.dir.clone()), T0 + DEFAULT_DURATION_MS + MARGIN_MS).unwrap();
        assert!(again.view().session.is_none());
    }

    #[test]
    fn identify_reports_each_station() {
        let (mut e, rec, _) = engine();
        for m in ["a", "b"] {
            let p = presence(&e, m);
            e.handle(ToConsole::Online { agent: p }, T0);
        }
        e.handle(ToConsole::Offline { member: "b".into() }, T0);
        e.identify(&[], T0).unwrap();
        let sent = rec.ids();
        assert_eq!(sent.len(), 1);
        let (id, cmd) = &sent[0];
        assert_eq!(cmd, &Command::Identify { number: "01".into(), seconds: IDENTIFY_SECONDS });
        e.handle(ToConsole::FromAgent { from: "a".into(), msg: AgentMsg::Ack { id: *id, ok: true, detail: None } }, T0);
        assert!(e.tick(T0 + 10));
        let n = e.take_notices();
        assert_eq!(n[0].text, "Identifier : envoyé à 1 poste · 1 injoignable (poste 02)");
        assert!(n[0].warning);
    }

    /// Restrictions envoyées à chaque poste, dans l'ordre.
    fn restricts(rec: &Recorder) -> Vec<(MemberId, Restrictions)> {
        rec.take()
            .into_iter()
            .filter_map(|(to, cmd)| match (to, cmd) {
                (Target::Members(m), Command::Restrict { restrictions, .. }) => Some((m[0].clone(), restrictions)),
                _ => None,
            })
            .collect()
    }

    fn in_session(e: &mut Engine, members: &[&str]) -> SessionId {
        for m in members {
            let p = presence(e, m);
            e.handle(ToConsole::Online { agent: p }, T0);
        }
        if e.session.is_none() {
            e.open_session(T0 + DEFAULT_DURATION_MS, T0).unwrap();
        }
        let sid = e.session.clone().unwrap().id;
        for m in members {
            status(e, m, Some(&sid));
        }
        sid
    }

    #[test]
    fn pause_reaches_the_room_then_newcomers_and_can_be_undone() {
        let (mut e, rec, dir) = engine();
        in_session(&mut e, &["a", "b"]);
        rec.take();

        // Toute la salle, message par défaut (ECRAN-2).
        e.set_pause(&[], Some("  ".into()), T0).unwrap();
        let sent = restricts(&rec);
        assert_eq!(sent.len(), 2);
        assert!(sent.iter().all(|(_, r)| r.pause.as_deref() == Some("Écran mis en pause par M. Durand.")));

        // Un poste qui rejoint la séance reçoit la pause de la salle.
        let p = presence(&e, "c");
        e.handle(ToConsole::Online { agent: p }, T0);
        let late = restricts(&rec);
        assert_eq!(late, vec![("c".to_string(), Restrictions { pause: Some("Écran mis en pause par M. Durand.".into()), ..Default::default() })]);

        // Reprise d'un seul poste : les autres restent en pause.
        e.set_pause(&["b".into()], None, T0).unwrap();
        assert_eq!(restricts(&rec), vec![("b".to_string(), Restrictions::default())]);
        let v = e.view().room.unwrap();
        let paused: Vec<bool> = v.stations.iter().map(|s| s.pause.is_some()).collect();
        assert_eq!(paused, vec![true, false, true]);

        // Retour poste par poste, avec « Annuler » (RETOUR-1, RETOUR-3).
        let ids: Vec<u64> = e.pending.last().unwrap().ids.keys().copied().collect();
        e.handle(ToConsole::FromAgent { from: "b".into(), msg: AgentMsg::Ack { id: ids[0], ok: true, detail: None } }, T0);
        e.tick(T0 + 10);
        let n = e.take_notices().pop().unwrap();
        assert_eq!(n.text, "Écran repris : envoyé à 1 poste");
        e.undo(n.undo.unwrap(), T0).unwrap();
        assert!(e.view().room.unwrap().stations.iter().all(|s| s.pause.is_some()));
        assert!(e.undo(n.undo.unwrap(), T0).is_err(), "une seule annulation");

        // Chaque restriction est consignée (MAIT-4).
        let journal = std::fs::read_to_string(dir.join("journal.jsonl")).unwrap();
        assert_eq!(journal.lines().count(), 3);
        assert!(journal.contains(r#""target":"poste 02","action":"Écran repris""#), "{journal}");
    }

    #[test]
    fn absent_stations_get_restrictions_on_return() {
        let (mut e, rec, _) = engine();
        in_session(&mut e, &["a", "b"]);
        e.handle(ToConsole::Offline { member: "b".into() }, T0);
        rec.take();
        e.set_web(&[], Some(WebPolicy::Allow { sites: vec!["https://www.Wikipedia.org/wiki".into()] }), T0).unwrap();
        let sent = restricts(&rec);
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].1.web, Some(WebPolicy::Allow { sites: vec!["wikipedia.org".into()] }));
        e.tick(T0 + ACK_TIMEOUT_MS);
        let n = e.take_notices().pop().unwrap();
        assert_eq!(n.text, "Filtrage web : liste d'autorisation (wikipedia.org) : envoyé à 0 poste · 2 en attente (poste 01, poste 02), appliqué à son retour");

        // Au retour, le poste reçoit séance puis restrictions (RESTR-2).
        let p = presence(&e, "b");
        e.handle(ToConsole::Online { agent: p }, T0 + 1000);
        assert_eq!(restricts(&rec)[0].1.web, Some(WebPolicy::Allow { sites: vec!["wikipedia.org".into()] }));
        assert!(e.set_web(&[], Some(WebPolicy::Block { sites: vec!["pas un site".into()] }), T0).is_err());
    }

    #[test]
    fn exam_ends_on_time_and_never_after_the_session() {
        let (mut e, rec, _) = engine();
        in_session(&mut e, &["a"]);
        e.set_web(&[], Some(WebPolicy::All), T0).unwrap();
        rec.take();
        let end = T0 + 30 * 60_000;
        assert!(e.set_exam(Some((T0 + DEFAULT_DURATION_MS + 1, vec![])), T0).is_err());
        e.set_exam(Some((end, vec!["moodle.lycee.fr".into()])), T0).unwrap();
        let r = &restricts(&rec)[0].1;
        assert_eq!(r.exam, Some(Exam { remaining_ms: 30 * 60_000, sites: vec!["moodle.lycee.fr".into()] }));
        assert_eq!(r.web, Some(WebPolicy::All), "le filtrage d'avant revient après l'examen");

        // Une séance raccourcie raccourcit l'examen (EXAM-3).
        e.set_end(T0 + 20 * 60_000, T0).unwrap();
        let r = restricts(&rec).pop().unwrap().1;
        assert_eq!(r.exam.unwrap().remaining_ms, 20 * 60_000);

        e.tick(T0 + 20 * 60_000);
        assert!(e.view().session.unwrap().exam.is_none());
        assert_eq!(restricts(&rec).pop().unwrap().1.exam, None);
    }

    #[test]
    fn leaving_a_restriction_is_reported() {
        let (mut e, _, _) = engine();
        let sid = in_session(&mut e, &["a", "b"]);
        e.set_web(&[], Some(WebPolicy::All), T0).unwrap();
        e.take_notices();
        let msg = AgentMsg::Status { session: Some(sid), screen: ScreenState::Off, web: WebState::Escaped };
        e.handle(ToConsole::FromAgent { from: "a".into(), msg }, T0);
        assert_eq!(e.take_notices()[0].text, "Poste 01 : sorti du filtrage web");
        e.handle(ToConsole::Offline { member: "b".into() }, T0);
        let alerts: Vec<String> = e.view().alerts.into_iter().map(|a| a.text).collect();
        assert_eq!(alerts, vec![
            "poste 02 : connexion perdue. Si l'agent a été arrêté, ses restrictions ne s'appliquent plus.".to_string(),
            "poste 01 : le filtrage web a été coupé sur le poste.".to_string(),
        ]);
        // Le poste revient : l'alerte le dit, sans s'ajouter à une autre.
        let p = presence(&e, "b");
        e.handle(ToConsole::Online { agent: p }, T0 + 60_000);
        let a = &e.view().alerts[0];
        assert_eq!((a.until_ms, e.view().alerts.len()), (Some(T0 + 60_000), 2));
        assert!(a.text.contains("puis revenu"));
    }

    #[test]
    fn lists_follow_the_teacher() {
        let (mut e, _, dir) = engine();
        e.save_site_list("TP réseau", &["cisco.com".into(), "https://netacad.com/".into()]).unwrap();
        e.save_site_list("tp RÉSEAU", &["cisco.com".into()]).unwrap();
        e.save_app_list("Jeux", &["Minecraft".into(), " minecraft ".into(), "Steam".into()]).unwrap();
        let again = Engine::new(Store::new(dir), T0).unwrap();
        let l = again.view().lists;
        assert_eq!(l.sites, vec![SiteList { name: "tp RÉSEAU".into(), sites: vec!["cisco.com".into()] }]);
        assert_eq!(l.apps[0].apps, vec!["Minecraft", "Steam"]);
    }
}
