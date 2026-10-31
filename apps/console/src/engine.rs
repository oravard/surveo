//! Salles, séance et retours poste par poste (SPEC.md, chap. 1, 9.5).
//!
//! Le moteur est synchrone et ne connaît ni l'interface ni le réseau :
//! il reçoit les messages du relais ([`Engine::handle`]), les gestes de
//! l'enseignant, et l'heure courante en paramètre, et envoie ses
//! commandes par un [`Link`]. Il est appelé toutes les secondes par
//! [`Engine::tick`] pour les échéances.

use crate::store::{Pos, Profile, Room, Session, Station, Store};
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
    deadline_ms: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Restore {
    Pending,
    Restored,
    Unreachable,
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
    link: Option<Box<dyn Link>>,
    next_id: u64,
    /// État du relais intégré, tenu par l'application.
    pub relay: RelayView,
    notices: Vec<Notice>,
}

fn plural(n: usize, word: &str) -> String {
    format!("{n} {word}{}", if n > 1 { "s" } else { "" })
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
        self.notices.push(Notice { text: text.into(), detail, warning });
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
            ToConsole::FromAgent { from, msg: AgentMsg::Status { session, screen } } => self.agent_status(&from, session, screen),
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
        self.online.insert(member.clone(), Online { presence: a, session: None, screen: ScreenState::Off });
        if in_room {
            let to = Target::Members(vec![member]);
            self.send_session(to.clone(), now);
            if self.session.as_ref().is_some_and(|s| s.screens) {
                self.send(to, Command::Screens { on: true });
            }
        }
    }

    fn agent_status(&mut self, from: &str, session: Option<SessionId>, screen: ScreenState) {
        let Some(o) = self.online.get_mut(from) else { return };
        o.session = session.clone();
        o.screen = screen;
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
        self.session = Some(Session { id: random_id(), room: room.clone(), teacher, opened_at_ms: now, end_ms, screens: false, members: BTreeSet::new() });
        self.closure = None;
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
        let room = s.room.clone();
        self.save_session();
        self.send_session(Target::Room(room), now);
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
        let mut p = Pending {
            label: "Identifier".into(),
            ids: BTreeMap::new(),
            acked: BTreeSet::new(),
            refused: vec![],
            offline: vec![],
            deadline_ms: now + ACK_TIMEOUT_MS,
        };
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
        let mut text = format!("{} : envoyé à {}", p.label, plural(p.acked.len(), "poste"));
        if !unreachable.is_empty() {
            let names: Vec<String> = unreachable.iter().map(|m| self.station_name(m)).collect();
            text += &format!(" · {} ({})", plural(unreachable.len(), "injoignable"), names.join(", "));
        }
        let detail = (!p.refused.is_empty()).then(|| {
            p.refused.iter().map(|(m, why)| format!("{} : {why}", self.station_name(m))).collect::<Vec<_>>().join(" · ")
        });
        Notice { text, detail, warning: !unreachable.is_empty() || !p.refused.is_empty() }
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
                    StationView {
                        member: s.member.clone(),
                        number: format!("{:02}", s.number),
                        label: s.label.clone(),
                        pos: s.pos,
                        state,
                        screen: o.map(|o| o.screen.clone()).unwrap_or_default(),
                        os: o.map(|o| o.presence.info.os.clone()),
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
        let msg = AgentMsg::Status { session: session.map(Into::into), screen: ScreenState::Off };
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
}
