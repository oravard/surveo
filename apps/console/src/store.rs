//! Fichiers de la console, dans [`surveo_platform::data_dir`] :
//!
//! | Fichier | Contenu |
//! |---|---|
//! | `etablissement.json` | clé d'établissement (secret, CONF-1) |
//! | `console.json` | enseignant, identités de la console et du relais intégré (secret) |
//! | `salles.json` | salles, postes et plans (SALLE-2, SALLE-3) |
//! | `seance.json` | séance ouverte, pour la reprendre si la console redémarre |

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::PathBuf;
use surveo_identity::{load_json, save_json, EstablishmentFile, IdentityFile};
use surveo_protocol::{MemberId, RoomId, SessionId};

/// Ce que la console sait d'elle-même.
#[derive(Serialize, Deserialize, Clone)]
pub struct Profile {
    /// Nom affiché aux élèves par l'indicateur de séance (REJ-4).
    pub teacher: String,
    pub console: IdentityFile,
    pub relay: IdentityFile,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, ts_rs::TS)]
#[ts(export)]
pub struct Pos {
    pub col: u8,
    pub row: u8,
}

/// Un poste connu d'une salle. Son numéro est celui de sa tuile, affiché
/// aussi sur le poste par « identifier » (SALLE-4).
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Station {
    pub member: MemberId,
    pub number: u16,
    pub label: String,
    /// Place sur le plan ; aucune : « poste à placer » (SALLE-3).
    pub pos: Option<Pos>,
}

/// Une salle et son plan (SALLE-2) : une grille de `cols` × `rows` places.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Room {
    pub id: RoomId,
    pub name: String,
    pub cols: u8,
    pub rows: u8,
    pub stations: Vec<Station>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Session {
    pub id: SessionId,
    pub room: RoomId,
    pub teacher: String,
    pub opened_at_ms: u64,
    /// Heure de fin prévue (SEANCE-1, SEANCE-3).
    pub end_ms: u64,
    /// L'enseignant affiche les écrans (AFF-4).
    pub screens: bool,
    /// Postes entrés dans la séance, pour le récapitulatif de clôture
    /// (RESTR-5).
    pub members: BTreeSet<MemberId>,
}

pub struct Store {
    pub dir: PathBuf,
}

impl Store {
    pub fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    pub fn establishment(&self) -> Option<EstablishmentFile> {
        load_json(&self.dir.join("etablissement.json")).ok()
    }

    pub fn save_establishment(&self, e: &EstablishmentFile) -> Result<()> {
        save_json(&self.dir.join("etablissement.json"), e, true)
    }

    pub fn profile(&self) -> Option<Profile> {
        load_json(&self.dir.join("console.json")).ok()
    }

    pub fn save_profile(&self, p: &Profile) -> Result<()> {
        save_json(&self.dir.join("console.json"), p, true)
    }

    pub fn rooms(&self) -> Vec<Room> {
        load_json(&self.dir.join("salles.json")).unwrap_or_default()
    }

    pub fn save_rooms(&self, r: &[Room]) -> Result<()> {
        save_json(&self.dir.join("salles.json"), &r, false)
    }

    pub fn session(&self) -> Option<Session> {
        load_json(&self.dir.join("seance.json")).ok()
    }

    pub fn save_session(&self, s: Option<&Session>) -> Result<()> {
        let path = self.dir.join("seance.json");
        match s {
            Some(s) => save_json(&path, s, false),
            None => {
                let _ = std::fs::remove_file(path);
                Ok(())
            }
        }
    }
}
