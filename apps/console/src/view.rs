//! Ce que l'interface de la console affiche. Les types TypeScript sont
//! engendrés dans `ui/shared/bindings/` par
//! `cargo test -p surveo-console export_bindings`.

use crate::store::Pos;
use serde::Serialize;
use surveo_protocol::ScreenState;
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
    pub limits: Limits,
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
}

/// Miniature de l'écran d'un poste, en JPEG codé en base 64 (AFF-4).
#[derive(Serialize, Clone, Debug, TS)]
#[ts(export)]
pub struct Thumb {
    pub member: String,
    pub data: String,
}
