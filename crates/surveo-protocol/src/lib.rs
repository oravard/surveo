//! Messages échangés entre agents, consoles et relais (IMPL.md,
//! « Protocole »).
//!
//! Deux sortes de trames circulent sur une connexion authentifiée :
//! - des **messages de contrôle** en JSON (trames texte) ;
//! - des **trames binaires** pour le volumineux (miniatures, morceaux de
//!   fichier), préfixées par leur nature et, entre consoles et relais, par
//!   le poste d'origine ou de destination.
//!
//! Le relais connaît le rôle de chaque pair depuis l'authentification :
//! un agent envoie des [`AgentMsg`], une console des [`FromConsole`] ;
//! le relais remet aux agents des [`ToAgent`] et aux consoles des
//! [`ToConsole`].

use serde::{Deserialize, Serialize};

/// Version du protocole, échangée à l'authentification. Un pair d'une
/// autre version est refusé avec un motif explicite.
pub const PROTOCOL_VERSION: u16 = 3;

/// Taille d'un morceau de fichier. Le destinataire tire les morceaux un
/// par un et redemande celui qui se perd (IMPL.md, 3.1).
pub const CHUNK_SIZE: u32 = 256 * 1024;

/// Taille maximale d'un rendu (SPEC.md, RENDU-7 ; proposition d'IMPL.md,
/// 3.1).
pub const MAX_RENDU_BYTES: u64 = 50_000_000;

/// Taille maximale d'un fichier distribué ou joint à un TP.
pub const MAX_FILE_BYTES: u64 = 200_000_000;

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

/// Filtrage web d'un poste (SPEC.md, WEB-1).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum WebPolicy {
    /// Seuls ces sites sont accessibles (liste d'autorisation).
    Allow { sites: Vec<String> },
    /// Ces sites sont bloqués, le reste est accessible (liste de blocage).
    Block { sites: Vec<String> },
    /// Aucun site accessible.
    All,
}

impl WebPolicy {
    /// Le site `host` est-il accessible ? Un site de la liste couvre aussi
    /// ses sous-domaines : `wikipedia.org` couvre `fr.wikipedia.org`.
    pub fn allows(&self, host: &str) -> bool {
        let host = host.trim_end_matches('.').to_ascii_lowercase();
        let listed = |l: &[String]| l.iter().any(|d| host == *d || host.strip_suffix(d.as_str()).is_some_and(|p| p.ends_with('.')));
        match self {
            WebPolicy::Allow { sites } => listed(sites),
            WebPolicy::Block { sites } => !listed(sites),
            WebPolicy::All => false,
        }
    }
}

/// Ramène ce que l'enseignant écrit (« https://www.Wikipedia.org/wiki »)
/// au nom du site (« wikipedia.org ») ; rien si ce n'est pas un nom de
/// site.
pub fn normalize_site(s: &str) -> Option<String> {
    let s = s.trim().to_ascii_lowercase();
    let s = s.split_once("://").map_or(s.as_str(), |(_, r)| r);
    let s = s.split(['/', '?', '#']).next().unwrap_or("");
    let s = s.rsplit_once('@').map_or(s, |(_, h)| h);
    let s = s.split(':').next().unwrap_or("");
    let s = s.trim_start_matches("*.").trim_start_matches("www.").trim_end_matches('.');
    let ok = !s.is_empty() && s.contains('.') && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.');
    ok.then(|| s.to_string())
}

/// Mode examen (SPEC.md, EXAM-1 à EXAM-3) : il a sa propre liste
/// d'autorisation, qui remplace le filtrage web du poste, et sa propre
/// échéance, en durée restante comme la séance.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Exam {
    pub remaining_ms: u64,
    pub sites: Vec<String>,
}

/// État complet des restrictions demandées à un poste (SPEC.md, chap. 2).
/// La console le renvoie à chaque changement et à chaque reconnexion ;
/// le poste applique la différence (IMPL.md, 2.2).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Default)]
pub struct Restrictions {
    /// Écran en pause, avec le message affiché (ECRAN-1, ECRAN-2).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pause: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub web: Option<WebPolicy>,
    /// Applications interdites, nommées par l'enseignant (APP-2, APP-3).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub banned_apps: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exam: Option<Exam>,
}

/// Une application ouverte sur un poste (APP-1). `id` la désigne pour la
/// fermer ; `name` est ce que l'enseignant et l'élève lisent.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct OpenApp {
    pub id: String,
    pub name: String,
}

/// Un fichier proposé à un pair, qui le tire morceau par morceau
/// ([`Command::Fetch`], [`AgentMsg::Fetch`], [`BinKind::Chunk`]).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct FileOffer {
    /// Identifiant de cet envoi : un même contenu envoyé deux fois donne
    /// deux fichiers (DISTRIB-2).
    pub id: String,
    pub name: String,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub size: u64,
    /// Empreinte SHA-256 du contenu, en hexadécimal, vérifiée à l'arrivée.
    pub sha256: String,
}

/// Ce qui se passe quand une étape dépasse sa durée (SPEC.md, MODELE-2).
/// L'aide de l'IA viendra avec le chapitre 5.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "snake_case")]
pub enum Overrun {
    #[default]
    Nothing,
    /// Le panneau élève s'ouvre sur les consignes de l'étape.
    ShowInstructions,
}

/// Étape d'un TP (MODELE-2).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct Step {
    pub objective: String,
    /// Consignes en Markdown (titres, gras, listes, code).
    pub instructions: String,
    /// Durée indicative, en minutes ; zéro : sans durée.
    pub minutes: u32,
    pub overrun: Overrun,
}

/// TP lancé sur un poste (TP-1) : une copie du modèle, avec ses fichiers
/// joints (DISTRIB-5).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct TpInfo {
    pub id: String,
    pub title: String,
    pub steps: Vec<Step>,
    /// Nombre de rendus attendus, un repère (RENDU-4).
    pub expected: u32,
    pub files: Vec<FileOffer>,
}

/// Sondage tel qu'un poste le reçoit (SONDAGE-1 à SONDAGE-5).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct PollInfo {
    pub id: String,
    pub question: String,
    pub options: Vec<String>,
    pub multiple: bool,
    /// Ouvert : l'élève répond et peut changer sa réponse ; clos : seuls
    /// les résultats montrés restent.
    pub open: bool,
    /// Temps restant avant la clôture, si une durée a été fixée.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub closes_in_ms: Option<u64>,
    /// Réponse de ce poste connue de la console, pour un poste redémarré.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub answer: Option<Vec<u16>>,
    /// Résultats montrés aux élèves : un compte par réponse, jamais de
    /// nom (SONDAGE-5).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub results: Option<Vec<u32>>,
}

/// Avancement d'un poste dans son TP (TP-6 à TP-8), annoncé par le poste.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct TpProgress {
    pub tp: String,
    /// Étapes cochées par l'élève.
    pub done: Vec<u16>,
    /// L'étape en cours dépasse sa durée (TP-7).
    pub late: bool,
}

/// Rendu déposé par l'élève, que la console tire du poste (RENDU-1).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct RenduOffer {
    pub session: SessionId,
    pub tp: String,
    pub file: FileOffer,
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
    /// Restrictions du poste dans cette séance (chap. 2).
    Restrict { session: SessionId, restrictions: Restrictions },
    /// Dire quelles applications sont ouvertes (APP-1) : le poste répond
    /// par [`AgentMsg::Apps`].
    ListApps,
    /// Fermer une application ouverte (APP-1).
    CloseApp { app: String },
    /// TP du poste dans cette séance, ou aucun (TP-1, TP-4, TP-9).
    /// Renvoyé à chaque reconnexion, comme les restrictions (TP-2, TP-3).
    Tp { session: SessionId, tp: Option<TpInfo> },
    /// Fichiers distribués au poste dans cette séance, tous ceux reçus ou
    /// à recevoir (DISTRIB-1, DISTRIB-4) ; le poste tire ceux qui lui
    /// manquent.
    Files { session: SessionId, files: Vec<FileOffer> },
    /// Envoyer ce morceau d'un rendu : le poste répond par une trame
    /// [`BinKind::Chunk`].
    Fetch { file: String, offset: u64, len: u32 },
    /// Le rendu est rangé côté enseignant (RENDU-2), ou refusé avec le
    /// motif (RENDU-7).
    Receipt {
        rendu: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },
    /// Sondage du poste, ou aucun (SONDAGE-2, SONDAGE-4).
    Poll { session: SessionId, poll: Option<PollInfo> },
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

/// État du filtrage web sur un poste.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Default)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum WebState {
    #[default]
    Off,
    On,
    /// Le réglage posé par l'agent a été changé sur le poste : l'élève
    /// est sorti de la restriction (MODE-3).
    Escaped,
    /// Filtrage impossible sur ce poste, avec le motif.
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
    Status {
        session: Option<SessionId>,
        screen: ScreenState,
        #[serde(default)]
        web: WebState,
        /// Avancement dans le TP en cours (TP-8).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        tp: Option<TpProgress>,
        /// Fichiers distribués entièrement reçus (DISTRIB-4).
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        files: Vec<String>,
    },
    /// Applications ouvertes, en réponse à [`Command::ListApps`] `id`.
    Apps { id: u64, apps: Vec<OpenApp> },
    /// Envoyer ce morceau d'un fichier distribué : la console répond par
    /// une trame [`BinKind::Chunk`].
    Fetch { file: String, offset: u64, len: u32 },
    /// Un rendu attend d'être tiré par la console (RENDU-1, RENDU-3).
    Submit { rendu: RenduOffer },
    /// Réponse de l'élève à un sondage, qu'il peut changer tant que le
    /// sondage est ouvert (SONDAGE-2).
    Answer { poll: String, choices: Vec<u16> },
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
    /// Miniature JPEG de l'écran (chap. 1, AFF). Le relais n'en garde que
    /// la plus récente de chaque poste.
    Thumbnail = 1,
    /// Morceau de fichier ([`encode_chunk`]), remis dans l'ordre et sans
    /// perte tant que les files du relais ne débordent pas.
    Chunk = 2,
}

impl BinKind {
    fn from_byte(b: u8) -> Option<Self> {
        match b {
            1 => Some(Self::Thumbnail),
            2 => Some(Self::Chunk),
            _ => None,
        }
    }
}

/// Contenu d'une trame [`BinKind::Chunk`] :
/// `[longueur de l'identifiant][identifiant du fichier][position, 8 octets][données]`.
pub fn encode_chunk(file: &str, offset: u64, data: &[u8]) -> Vec<u8> {
    let id = file.as_bytes();
    let len = u8::try_from(id.len()).expect("identifiant de fichier trop long");
    let mut out = Vec::with_capacity(9 + id.len() + data.len());
    out.push(len);
    out.extend_from_slice(id);
    out.extend_from_slice(&offset.to_be_bytes());
    out.extend_from_slice(data);
    out
}

pub fn decode_chunk(payload: &[u8]) -> Option<(&str, u64, &[u8])> {
    let (&len, rest) = payload.split_first()?;
    let len = len as usize;
    if rest.len() < len + 8 {
        return None;
    }
    let file = std::str::from_utf8(&rest[..len]).ok()?;
    let offset = u64::from_be_bytes(rest[len..len + 8].try_into().ok()?);
    Some((file, offset, &rest[len + 8..]))
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
    fn file_chunks_round_trip() {
        let c = encode_chunk("f1", 3 * CHUNK_SIZE as u64, b"data");
        assert_eq!(decode_chunk(&c), Some(("f1", 3 * CHUNK_SIZE as u64, &b"data"[..])));
        let empty = encode_chunk("f1", 0, b"");
        assert_eq!(decode_chunk(&empty), Some(("f1", 0, &b""[..])));
        assert_eq!(decode_chunk(&[2, b'f', b'1', 0, 0]), None);
        let framed = encode_console_frame(BinKind::Chunk, "poste-1", &c);
        assert_eq!(decode_console_frame(&framed), Some((BinKind::Chunk, "poste-1", &c[..])));
    }

    #[test]
    fn control_messages_are_tagged_json() {
        let m = FromConsole::Send { id: 3, to: Target::Members(vec!["p".into()]), cmd: Command::Screens { on: true } };
        let s = serde_json::to_string(&m).unwrap();
        assert_eq!(s, r#"{"t":"send","id":3,"to":{"members":["p"]},"cmd":{"t":"screens","on":true}}"#);
        assert_eq!(serde_json::from_str::<FromConsole>(&s).unwrap(), m);
        let all = serde_json::to_string(&Target::All).unwrap();
        assert_eq!(all, r#""all""#);
        // Un poste de la tranche 1 n'envoie pas l'état du filtrage.
        let old: AgentMsg = serde_json::from_str(r#"{"t":"status","session":null,"screen":{"t":"off"}}"#).unwrap();
        assert_eq!(old, AgentMsg::Status { session: None, screen: ScreenState::Off, web: WebState::Off, tp: None, files: vec![] });
        let none = serde_json::to_string(&Restrictions::default()).unwrap();
        assert_eq!(none, "{}");
    }

    #[test]
    fn web_policy_covers_subdomains_only() {
        let allow = WebPolicy::Allow { sites: vec!["wikipedia.org".into()] };
        assert!(allow.allows("wikipedia.org") && allow.allows("FR.Wikipedia.org."));
        assert!(!allow.allows("notwikipedia.org") && !allow.allows("wikipedia.org.evil.fr"));
        let block = WebPolicy::Block { sites: vec!["youtube.com".into()] };
        assert!(!block.allows("www.youtube.com") && block.allows("education.gouv.fr"));
        assert!(!WebPolicy::All.allows("education.gouv.fr"));
    }

    #[test]
    fn sites_are_normalized() {
        assert_eq!(normalize_site(" https://www.Wikipedia.org/wiki/Réseau ").as_deref(), Some("wikipedia.org"));
        assert_eq!(normalize_site("*.netacad.com:443").as_deref(), Some("netacad.com"));
        assert_eq!(normalize_site("http://user@cisco.com").as_deref(), Some("cisco.com"));
        assert_eq!(normalize_site("localhost"), None);
        assert_eq!(normalize_site("pas un site"), None);
    }
}
