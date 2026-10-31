//! Cœur de la console de l'enseignant (IMPL.md, « Architecture »),
//! indépendant de l'interface :
//! - [`store`] : fichiers de la console (établissement, salles, séance,
//!   bibliothèque, rendus) ;
//! - [`library`] : modèles de TP et sondages, export et import ;
//! - [`engine`] : salles et plans, séance, restrictions, pédagogie,
//!   retours poste par poste ;
//! - [`view`] : ce que l'interface affiche, avec ses types TypeScript.
//!
//! La console qui a ouvert la séance en tient l'état de référence ; un
//! poste qui se reconnecte se resynchronise depuis elle (IMPL.md, « État
//! et stockage »).

pub mod engine;
pub mod library;
pub mod store;
pub mod view;

use std::time::{SystemTime, UNIX_EPOCH};

/// Valeurs par défaut des durées (SPEC.md, 1.11, proposition) : séance
/// d'une heure, quatre heures au plus prolongations comprises (SEANCE-4),
/// marge de dix minutes après l'heure de fin (SEANCE-6).
pub const DEFAULT_DURATION_MS: u64 = 60 * 60_000;
pub const MAX_DURATION_MS: u64 = 4 * 60 * 60_000;
pub const MARGIN_MS: u64 = 10 * 60_000;
/// Pas d'une prolongation (SEANCE-3).
pub const EXTEND_STEP_MS: u64 = 15 * 60_000;
/// Délai au-delà duquel un poste qui n'a pas répondu est déclaré
/// injoignable (RETOUR-1, RESTR-5).
pub const ACK_TIMEOUT_MS: u64 = 5_000;
/// Durée d'affichage du numéro d'un poste (SALLE-4).
pub const IDENTIFY_SECONDS: u32 = 6;
/// Sans morceau reçu pendant ce délai, un rendu en cours de réception est
/// redemandé là où il en était (IMPL.md, 3.1).
pub const FETCH_TIMEOUT_MS: u64 = 10_000;

pub fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}
