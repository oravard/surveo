//! Ce que l'interface de la console affiche. Les types TypeScript sont
//! engendrés dans `ui/shared/bindings/` par
//! `cargo test -p surveo-console export_bindings`.

use crate::library::Library;
use crate::store::{Lists, Pos};
use serde::Serialize;
use surveo_protocol::{DeployMode, OpenApp, PosteKind, ScreenState, WebPolicy, WebState};
use ts_rs::TS;

#[derive(Serialize, Clone, Debug, TS)]
#[ts(export)]
pub struct ConsoleView {
    /// Absent tant qu'aucun établissement n'est créé (CONF-1).
    pub setup: Option<SetupView>,
    pub relay: RelayView,
    pub rooms: Vec<RoomSummary>,
    /// Salle choisie (SALLE-1).
    pub room: Option<RoomView>,
    pub session: Option<SessionView>,
    /// Récapitulatif de la dernière clôture (RESTR-5).
    pub closure: Option<ClosureView>,
    /// Dernières connexions refusées par le relais (CONF-3).
    pub refused: Vec<String>,
    /// Sorties de restriction de la séance, la plus récente d'abord
    /// (MODE-3, MODE-5).
    pub alerts: Vec<Alert>,
    /// Listes de sites et d'applications de l'enseignant (WEB-4, APP-4).
    pub lists: Lists,
    /// Modèles de TP et sondages de l'enseignant (MODELE-3, SONDAGE-6).
    pub library: Library,
    /// Dossier où sont rangés les rendus (RENDU-6).
    pub rendus_dir: String,
    pub limits: Limits,
}

#[derive(Serialize, Clone, Debug, PartialEq, Eq, TS)]
#[ts(export)]
pub struct Alert {
    #[ts(type = "number")]
    pub at_ms: u64,
    /// Fin de la sortie (retour du poste), si elle est connue.
    #[ts(type = "number | null")]
    pub until_ms: Option<u64>,
    pub text: String,
}

#[derive(Serialize, Clone, Debug, TS)]
#[ts(export)]
pub struct SetupView {
    pub etab_name: String,
    pub teacher: String,
}

#[derive(Serialize, Clone, Debug, Default, TS)]
#[ts(export)]
pub struct RelayView {
    pub running: bool,
    pub port: u16,
    /// Adresses de ce poste sur le réseau local.
    pub addresses: Vec<String>,
    /// Le relais ne répond pas aux recherches des postes.
    pub discovery_error: Option<String>,
    pub error: Option<String>,
}

#[derive(Serialize, Clone, Debug, TS)]
#[ts(export)]
pub struct RoomSummary {
    pub id: String,
    pub name: String,
    pub stations: u32,
}

#[derive(Serialize, Clone, Debug, TS)]
#[ts(export)]
pub struct RoomView {
    pub id: String,
    pub name: String,
    pub cols: u8,
    pub rows: u8,
    pub stations: Vec<StationView>,
}

/// État d'un poste (SALLE-1).
#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq, TS)]
#[ts(export)]
#[serde(rename_all = "snake_case")]
pub enum StationState {
    /// Connu mais absent.
    Off,
    /// Allumé, hors de la séance.
    On,
    InSession,
}

#[derive(Serialize, Clone, Debug, TS)]
#[ts(export)]
pub struct StationView {
    pub member: String,
    /// Numéro de la tuile, sur deux chiffres (« 07 »).
    pub number: String,
    pub label: String,
    pub pos: Option<Pos>,
    pub state: StationState,
    pub screen: ScreenState,
    pub os: Option<String>,
    /// Type et mode de déploiement, connus quand le poste est allumé
    /// (MODE-2).
    pub kind: Option<PosteKind>,
    pub mode: Option<DeployMode>,
    /// Restrictions du poste dans la séance (AFF-2).
    pub pause: Option<String>,
    pub web: Option<WebPolicy>,
    pub banned_apps: Vec<String>,
    /// État du filtrage annoncé par le poste (MODE-3).
    pub web_state: WebState,
    /// Applications ouvertes, si l'enseignant les a demandées (APP-1).
    pub apps: Option<Vec<OpenApp>>,
    /// TP du poste et son avancement (TP-8).
    pub tp: Option<StationTp>,
    /// Fichiers distribués au poste, TP compris, et combien il en a reçus
    /// (DISTRIB-4).
    pub files_total: u32,
    pub files_received: u32,
    /// Le poste a répondu au sondage ouvert ; aucun : pas interrogé.
    pub answered: Option<bool>,
}

/// TP d'un poste (TP-8).
#[derive(Serialize, Clone, Debug, PartialEq, Eq, TS)]
#[ts(export)]
pub struct StationTp {
    pub run: String,
    pub title: String,
    /// Étape en cours (la première non terminée) ; aucune : toutes faites.
    pub step: Option<u32>,
    pub steps: u32,
    pub objective: Option<String>,
    /// L'étape en cours dépasse sa durée (TP-7).
    pub late: bool,
    /// Rendus reçus, sur le nombre attendu (RENDU-4).
    pub rendus: u32,
    pub expected: u32,
    /// Le poste a déjà annoncé son avancement.
    pub known: bool,
}

/// Synthèse d'un TP en cours (TP-8).
#[derive(Serialize, Clone, Debug, TS)]
#[ts(export)]
pub struct TpRunView {
    pub id: String,
    pub model: String,
    pub title: String,
    /// Objectif de chaque étape.
    pub steps: Vec<String>,
    pub expected: u32,
    pub stations: u32,
    /// Nombre de postes à chaque étape.
    pub at_step: Vec<u32>,
    /// Postes qui ont terminé toutes les étapes.
    pub finished: u32,
    pub late: u32,
    /// Postes qui ont rendu au moins le nombre attendu.
    pub complete: u32,
    /// Lancé sur toute la séance (TP-2).
    pub all: bool,
}

/// Fichier distribué dans la séance (DISTRIB-4).
#[derive(Serialize, Clone, Debug, TS)]
#[ts(export)]
pub struct DistributionView {
    pub id: String,
    pub name: String,
    #[ts(type = "number")]
    pub size: u64,
    pub all: bool,
    pub targets: u32,
    pub received: u32,
    #[ts(type = "number")]
    pub at_ms: u64,
}

/// Sondage de la séance, résultats en direct (SONDAGE-3).
#[derive(Serialize, Clone, Debug, TS)]
#[ts(export)]
pub struct PollView {
    pub id: String,
    pub question: String,
    pub options: Vec<String>,
    pub multiple: bool,
    pub open: bool,
    pub shown: bool,
    #[ts(type = "number | null")]
    pub closes_at_ms: Option<u64>,
    /// Nombre de choix de chaque réponse.
    pub counts: Vec<u32>,
    pub answered: u32,
    pub targets: u32,
    pub all: bool,
    /// Réponse de chaque poste, pour l'enseignant seulement.
    pub details: Vec<PollDetail>,
}

#[derive(Serialize, Clone, Debug, TS)]
#[ts(export)]
pub struct PollDetail {
    pub station: String,
    pub choices: Vec<u16>,
}

#[derive(Serialize, Clone, Debug, TS)]
#[ts(export)]
pub struct SessionView {
    pub id: String,
    pub room_id: String,
    pub room_name: String,
    pub teacher: String,
    #[ts(type = "number")]
    pub opened_at_ms: u64,
    #[ts(type = "number")]
    pub end_ms: u64,
    /// Heure de fin au plus tard, prolongations comprises (SEANCE-4).
    #[ts(type = "number")]
    pub max_end_ms: u64,
    #[ts(type = "number")]
    pub margin_ms: u64,
    pub screens_on: bool,
    /// Restrictions posées sur toute la salle (SEL-2).
    pub pause_all: Option<String>,
    pub web_all: Option<WebPolicy>,
    pub apps_all: Vec<String>,
    pub exam: Option<ExamView>,
    /// TP en cours, avec leur synthèse (TP-8).
    pub tps: Vec<TpRunView>,
    /// Fichiers distribués (DISTRIB-4).
    pub files: Vec<DistributionView>,
    pub poll: Option<PollView>,
}

/// Mode examen de la séance (EXAM-1).
#[derive(Serialize, Clone, Debug, TS)]
#[ts(export)]
pub struct ExamView {
    #[ts(type = "number")]
    pub end_ms: u64,
    pub sites: Vec<String>,
}

#[derive(Serialize, Clone, Debug, TS)]
#[ts(export)]
pub struct ClosureView {
    #[ts(type = "number")]
    pub ended_at_ms: u64,
    /// Séance terminée d'elle-même (SEANCE-6).
    pub auto: bool,
    /// Heure à laquelle un poste injoignable sera de toute façon revenu à
    /// son état d'origine (RESTR-1).
    #[ts(type = "number")]
    pub restore_by_ms: u64,
    pub restored: Vec<String>,
    pub pending: Vec<String>,
    pub unreachable: Vec<String>,
}

#[derive(Serialize, Clone, Debug, TS)]
#[ts(export)]
pub struct Limits {
    #[ts(type = "number")]
    pub default_duration_ms: u64,
    #[ts(type = "number")]
    pub max_duration_ms: u64,
    #[ts(type = "number")]
    pub margin_ms: u64,
    #[ts(type = "number")]
    pub extend_step_ms: u64,
}

/// Message passager à l'enseignant : retour d'action (RETOUR-1),
/// avertissement.
#[derive(Serialize, Clone, Debug, PartialEq, Eq, TS)]
#[ts(export)]
pub struct Notice {
    pub text: String,
    pub detail: Option<String>,
    pub warning: bool,
    /// Jeton à rendre à `undo` pour annuler l'action (RETOUR-3).
    #[ts(type = "number | null")]
    pub undo: Option<u64>,
}

/// Miniature de l'écran d'un poste, en JPEG codé en base 64 (AFF-4).
#[derive(Serialize, Clone, Debug, TS)]
#[ts(export)]
pub struct Thumb {
    pub member: String,
    pub data: String,
}
