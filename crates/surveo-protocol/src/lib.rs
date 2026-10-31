//! Messages échangés entre agents, consoles et relais (IMPL.md,
//! « Protocole »).
//!
//! Deux sortes de trames circulent sur une connexion authentifiée :
//! - des **messages de contrôle** en JSON (trames texte) ;
//! - des **trames binaires** pour le volumineux (miniatures), préfixées
//!   par leur nature et, du relais vers les consoles, par le poste
//!   d'origine.
//!
//! Le relais connaît le rôle de chaque pair depuis l'authentification :
//! un agent envoie des [`AgentMsg`], une console des [`FromConsole`] ;
//! le relais remet aux agents des [`ToAgent`] et aux consoles des
//! [`ToConsole`].

use serde::{Deserialize, Serialize};

/// Version du protocole, échangée à l'authentification. Un pair d'une
/// autre version est refusé avec un motif explicite.
pub const PROTOCOL_VERSION: u16 = 1;

/// Port UDP de la recherche du relais sur le réseau local (SPEC.md,
/// DECOUV-1, niveau 3).
pub const DISCOVERY_PORT: u16 = 47823;

/// Port TCP du relais, quand il est libre.
pub const RELAY_PORT: u16 = 47824;

/// Identifiant stable d'un membre (poste, console, relais), qui ne change
/// pas avec le nom ou l'adresse de l'ordinateur (SPEC.md, 1.1).
pub type MemberId = String;
pub type RoomId = String;
pub type SessionId = String;

/// Type de poste, fixé à l'installation (SPEC.md, INST-2).
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "snake_case")]
pub enum PosteKind {
    Salle,
    Personnel,
}

/// Mode de déploiement de l'agent (SPEC.md, MODE-1).
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "snake_case")]
pub enum DeployMode {
    SansAdmin,
    Etablissement,
}

/// Ce qu'un agent dit de lui à l'authentification. Le nom d'ordinateur
/// n'est qu'un libellé (SPEC.md, 1.1).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct AgentInfo {
    pub label: String,
    pub os: String,
    pub version: String,
    pub kind: PosteKind,
    pub mode: DeployMode,
}

/// Un agent connecté au relais. La salle vient de son certificat, signé
/// par l'établissement, et non de ce que l'agent déclare.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct AgentPresence {
    pub member: MemberId,
    pub room: Option<RoomId>,
    pub info: AgentInfo,
}

/// Séance à laquelle un poste est relié (SPEC.md, 1.4).
///
/// Les échéances sont des durées restantes, et non des heures : le poste
/// calcule son échéance sur sa propre horloge, ce qui le rend
/// insensible à un décalage d'horloge avec la console.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct SessionInfo {
    pub id: SessionId,
    pub room: Option<RoomId>,
    /// Nom de l'enseignant, affiché par l'indicateur (REJ-4).
    pub teacher: String,
    /// Temps restant jusqu'à l'heure de fin prévue.
    pub remaining_ms: u64,
    /// Marge après l'heure de fin, au-delà de laquelle la séance se
    /// termine d'elle-même (SEANCE-6).
    pub margin_ms: u64,
}

/// Demande d'une console à des postes.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum Command {
    /// Le poste est relié à cette séance (REJ-1). Renvoyée à chaque
    /// changement de l'heure de fin (SEANCE-3) et à chaque reconnexion
    /// du poste (RESTR-2).
    Session { session: SessionInfo },
    /// Fin de la séance : le poste revient à son état d'origine
    /// (SEANCE-5).
    EndSession { session: SessionId },
    /// Envoyer ou non une miniature de l'écran (AFF-4).
    Screens { on: bool },
    /// Afficher un grand numéro pendant quelques secondes (SALLE-4).
    Identify { number: String, seconds: u32 },
}

/// À qui une console adresse une commande.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Target {
    All,
    Room(RoomId),
    Members(Vec<MemberId>),
}

/// Console → relais.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum FromConsole {
    Send { id: u64, to: Target, cmd: Command },
}

/// Relais → agent.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum ToAgent {
    /// `from` est la console émettrice, authentifiée par le relais.
    Command { from: MemberId, id: u64, cmd: Command },
}

/// État de l'image d'écran d'un poste.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Default)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum ScreenState {
    /// Aucune capture (hors séance, ou écrans non affichés).
    #[default]
    Off,
    /// Capture demandée, en attente de la première image (sous Wayland,
    /// le système peut demander l'accord de l'utilisateur).
    Starting,
    On,
    /// Capture impossible : accord refusé, système non pris en charge…
    Unavailable { reason: String },
}

/// Agent → relais → consoles.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum AgentMsg {
    /// Réponse à une commande, pour le retour poste par poste (RETOUR-1).
    Ack {
        id: u64,
        ok: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        detail: Option<String>,
    },
    /// État du poste, envoyé à la connexion et à chaque changement.
    Status { session: Option<SessionId>, screen: ScreenState },
}

/// Relais → console.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum ToConsole {
    /// Postes connectés, à l'arrivée de la console.
    Roster { agents: Vec<AgentPresence> },
    Online { agent: AgentPresence },
    Offline { member: MemberId },
    FromAgent { from: MemberId, msg: AgentMsg },
    /// Connexion refusée par le relais (CONF-3), pour que l'enseignant
    /// comprenne pourquoi un poste n'apparaît pas.
    Refused { peer: String, reason: String },
}

/// Nature d'une trame binaire.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum BinKind {
    /// Miniature JPEG de l'écran (chap. 1, AFF).
    Thumbnail = 1,
}

impl BinKind {
    fn from_byte(b: u8) -> Option<Self> {
        match b {
            1 => Some(Self::Thumbnail),
            _ => None,
        }
    }
}

/// Trame binaire d'un agent : `[nature][contenu]`.
pub fn encode_agent_frame(kind: BinKind, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(1 + payload.len());
    out.push(kind as u8);
    out.extend_from_slice(payload);
    out
}

pub fn decode_agent_frame(frame: &[u8]) -> Option<(BinKind, &[u8])> {
    let (&k, rest) = frame.split_first()?;
    Some((BinKind::from_byte(k)?, rest))
}

/// Trame binaire remise à une console :
/// `[nature][longueur de l'identifiant][identifiant du poste][contenu]`.
pub fn encode_console_frame(kind: BinKind, member: &str, payload: &[u8]) -> Vec<u8> {
    let id = member.as_bytes();
    let len = u8::try_from(id.len()).expect("identifiant de membre trop long");
    let mut out = Vec::with_capacity(2 + id.len() + payload.len());
    out.push(kind as u8);
    out.push(len);
    out.extend_from_slice(id);
    out.extend_from_slice(payload);
    out
}

pub fn decode_console_frame(frame: &[u8]) -> Option<(BinKind, &str, &[u8])> {
    let (&k, rest) = frame.split_first()?;
    let (&len, rest) = rest.split_first()?;
    let len = len as usize;
    if rest.len() < len {
        return None;
    }
    let member = std::str::from_utf8(&rest[..len]).ok()?;
    Some((BinKind::from_byte(k)?, member, &rest[len..]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn binary_frames_round_trip() {
        let a = encode_agent_frame(BinKind::Thumbnail, b"jpeg");
        assert_eq!(decode_agent_frame(&a), Some((BinKind::Thumbnail, &b"jpeg"[..])));
        let c = encode_console_frame(BinKind::Thumbnail, "poste-1", b"jpeg");
        assert_eq!(decode_console_frame(&c), Some((BinKind::Thumbnail, "poste-1", &b"jpeg"[..])));
        assert_eq!(decode_console_frame(&[1, 9, b'x']), None);
        assert_eq!(decode_agent_frame(&[7, 1]), None);
    }

    #[test]
    fn control_messages_are_tagged_json() {
        let m = FromConsole::Send { id: 3, to: Target::Members(vec!["p".into()]), cmd: Command::Screens { on: true } };
        let s = serde_json::to_string(&m).unwrap();
        assert_eq!(s, r#"{"t":"send","id":3,"to":{"members":["p"]},"cmd":{"t":"screens","on":true}}"#);
        assert_eq!(serde_json::from_str::<FromConsole>(&s).unwrap(), m);
        let all = serde_json::to_string(&Target::All).unwrap();
        assert_eq!(all, r#""all""#);
    }
}
