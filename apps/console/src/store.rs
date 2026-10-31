//! Fichiers de la console, dans [`surveo_platform::data_dir`] :
//!
//! | Fichier | Contenu |
//! |---|---|
//! | `etablissement.json` | clé d'établissement (secret, CONF-1) |
//! | `console.json` | enseignant, identités de la console et du relais intégré (secret) |
//! | `salles.json` | salles, postes et plans (SALLE-2, SALLE-3) |
//! | `seance.json` | séance ouverte et ses restrictions, pour la reprendre si la console redémarre |
//! | `listes.json` | listes de sites et d'applications de l'enseignant (WEB-4, APP-4) |
//! | `journal.jsonl` | restrictions appliquées, une par ligne (MAIT-4) |
//! | `bibliotheque.json` | modèles de TP et sondages de l'enseignant (MODELE-3, SONDAGE-6) |
//! | `fichiers/` | fichiers joints et distribués, nommés par leur empreinte SHA-256 |
//! | `rendus.json` | rendus reçus par séance, et où ils sont rangés (RENDU-6) |
//! | `reception/` | rendus en cours de réception |
//!
//! Les rendus eux-mêmes sont rangés dans le dossier des rendus
//! ([`Store::rendus`]), que l'enseignant ouvre depuis la console.

use crate::library::Library;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use surveo_identity::{load_json, save_json, EstablishmentFile, IdentityFile};
use surveo_protocol::{FileOffer, MemberId, RoomId, SessionId, Step, WebPolicy};

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

/// Valeur d'une restriction pour toute la salle, et pour les postes d'une
/// sélection (IMPL.md, 2.2) : un poste qui rejoint la séance plus tard
/// reçoit la valeur de la salle.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct Layer<T> {
    pub all: T,
    #[serde(default)]
    pub some: BTreeMap<MemberId, T>,
}

impl<T: Clone + PartialEq> Layer<T> {
    pub fn get(&self, member: &str) -> &T {
        self.some.get(member).unwrap_or(&self.all)
    }

    /// Sans poste désigné, toute la salle.
    pub fn set(&mut self, members: &[MemberId], v: T) {
        if members.is_empty() {
            self.all = v;
            self.some.clear();
            return;
        }
        for m in members {
            if v == self.all {
                self.some.remove(m);
            } else {
                self.some.insert(m.clone(), v.clone());
            }
        }
    }
}

/// Mode examen de la séance (SPEC.md, EXAM-1 à EXAM-3).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct ExamState {
    pub end_ms: u64,
    /// Liste d'autorisation de l'examen, vide par défaut (EXAM-2).
    pub sites: Vec<String>,
}

/// Restrictions de la séance (SPEC.md, chap. 2).
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct SessionRestrictions {
    /// Message de l'écran en pause (ECRAN-2).
    pub pause: Layer<Option<String>>,
    pub web: Layer<Option<WebPolicy>>,
    /// Applications interdites (APP-2).
    pub apps: Layer<Vec<String>>,
    pub exam: Option<ExamState>,
}

/// Liste de sites réutilisable (WEB-4).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, ts_rs::TS)]
#[ts(export)]
pub struct SiteList {
    pub name: String,
    pub sites: Vec<String>,
}

/// Liste d'applications interdites réutilisable (APP-4).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, ts_rs::TS)]
#[ts(export)]
pub struct AppList {
    pub name: String,
    pub apps: Vec<String>,
}

/// Listes de l'enseignant, qui le suivent d'une salle à l'autre (WEB-4,
/// APP-4).
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq, ts_rs::TS)]
#[ts(export)]
pub struct Lists {
    pub sites: Vec<SiteList>,
    pub apps: Vec<AppList>,
}

/// Une restriction consignée (MAIT-4) : qui, quoi, quand, où.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct JournalEntry {
    pub at_ms: u64,
    pub teacher: String,
    pub room: String,
    pub session: SessionId,
    /// Toute la salle, ou les postes visés.
    pub target: String,
    pub action: String,
}

/// TP lancé dans la séance : une copie du modèle (TP-1).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct TpRun {
    pub id: String,
    /// Modèle d'origine, pour dire lequel est en cours.
    pub model: String,
    pub title: String,
    pub steps: Vec<Step>,
    pub expected: u32,
    /// Fichiers joints, distribués avec le TP (DISTRIB-5).
    pub files: Vec<FileOffer>,
    pub launched_at_ms: u64,
}

/// Fichier distribué dans la séance (DISTRIB-1).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Distribution {
    pub file: FileOffer,
    /// Postes visés ; aucun : toute la séance, y compris les postes qui la
    /// rejoignent plus tard (DISTRIB-4).
    pub to: Option<BTreeSet<MemberId>>,
    pub at_ms: u64,
}

/// Sondage de la séance (SONDAGE-2 à SONDAGE-5) ; un seul à la fois.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct PollRun {
    pub id: String,
    pub question: String,
    pub options: Vec<String>,
    pub multiple: bool,
    pub to: Option<BTreeSet<MemberId>>,
    pub open: bool,
    pub closes_at_ms: Option<u64>,
    /// Résultats montrés aux élèves, sans nom (SONDAGE-5).
    pub shown: bool,
    pub answers: BTreeMap<MemberId, Vec<u16>>,
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
    #[serde(default)]
    pub restrictions: SessionRestrictions,
    /// TP lancés, et celui de chaque poste (TP-1, TP-4).
    #[serde(default)]
    pub tps: Vec<TpRun>,
    #[serde(default)]
    pub tp: Layer<Option<String>>,
    #[serde(default)]
    pub files: Vec<Distribution>,
    #[serde(default)]
    pub poll: Option<PollRun>,
}

/// Rendus reçus, par séance : de quoi ranger un rendu qui arrive après
/// la séance (RENDU-3, RENDU-8), le reconnaître s'il est proposé de
/// nouveau, et compter ceux de chaque poste (TP-8).
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Archive {
    pub sessions: Vec<ArchivedSession>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ArchivedSession {
    pub id: SessionId,
    /// Dossier de la séance dans le dossier des rendus.
    pub folder: String,
    /// TP lancés : titre, et dossier une fois le premier rendu arrivé.
    pub tps: BTreeMap<String, ArchivedTp>,
    /// Dossier de chaque poste, fixé à son premier rendu.
    pub stations: BTreeMap<MemberId, String>,
    pub rendus: Vec<ReceivedRendu>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ArchivedTp {
    pub title: String,
    pub folder: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ReceivedRendu {
    pub id: String,
    pub member: MemberId,
    pub tp: String,
    pub name: String,
    pub at_ms: u64,
}

pub struct Store {
    pub dir: PathBuf,
    /// Dossier des rendus : par séance, puis par TP, puis par poste
    /// (RENDU-6).
    pub rendus: PathBuf,
}

impl Store {
    /// Les rendus sont rangés dans `dir/rendus` ; l'application les met
    /// dans les Documents de l'enseignant ([`Store::with_rendus`]).
    pub fn new(dir: PathBuf) -> Self {
        let rendus = dir.join("rendus");
        Self { dir, rendus }
    }

    pub fn with_rendus(dir: PathBuf, rendus: PathBuf) -> Self {
        Self { dir, rendus }
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

    pub fn lists(&self) -> Lists {
        load_json(&self.dir.join("listes.json")).unwrap_or_default()
    }

    pub fn save_lists(&self, l: &Lists) -> Result<()> {
        save_json(&self.dir.join("listes.json"), l, false)
    }

    /// Ajoute une ligne au journal des restrictions (MAIT-4).
    pub fn append_journal(&self, e: &JournalEntry) -> Result<()> {
        use std::io::Write;
        std::fs::create_dir_all(&self.dir)?;
        let mut f = std::fs::OpenOptions::new().create(true).append(true).open(self.dir.join("journal.jsonl"))?;
        writeln!(f, "{}", serde_json::to_string(e)?)?;
        Ok(())
    }

    pub fn library(&self) -> Library {
        load_json(&self.dir.join("bibliotheque.json")).unwrap_or_default()
    }

    pub fn save_library(&self, l: &Library) -> Result<()> {
        save_json(&self.dir.join("bibliotheque.json"), l, false)
    }

    pub fn archive(&self) -> Archive {
        load_json(&self.dir.join("rendus.json")).unwrap_or_default()
    }

    pub fn save_archive(&self, a: &Archive) -> Result<()> {
        save_json(&self.dir.join("rendus.json"), a, false)
    }

    /// Fichier du magasin, nommé par son empreinte.
    pub fn blob(&self, sha256: &str) -> PathBuf {
        self.dir.join("fichiers").join(sha256)
    }

    /// Range une copie de `src` dans le magasin ; rend sa taille et son
    /// empreinte.
    pub fn put_blob(&self, src: &Path) -> Result<(u64, String)> {
        let tmp = self.dir.join("fichiers").join(format!("ajout-{}", surveo_identity::random_id()));
        let (size, sha) = surveo_platform::files::copy_hashed(src, &tmp)?;
        std::fs::rename(&tmp, self.blob(&sha)).context("rangement du fichier")?;
        Ok((size, sha))
    }

    pub fn put_blob_bytes(&self, data: &[u8]) -> Result<(u64, String)> {
        let sha = surveo_platform::files::sha256_hex(data);
        let path = self.blob(&sha);
        if !path.exists() {
            std::fs::create_dir_all(self.dir.join("fichiers"))?;
            let tmp = path.with_extension("tmp");
            std::fs::write(&tmp, data)?;
            std::fs::rename(&tmp, &path)?;
        }
        Ok((data.len() as u64, sha))
    }

    /// Retire du magasin les fichiers que plus rien ne désigne.
    pub fn sweep_blobs(&self, keep: &BTreeSet<String>) {
        let Ok(entries) = std::fs::read_dir(self.dir.join("fichiers")) else { return };
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if !keep.contains(&name) {
                let _ = std::fs::remove_file(e.path());
            }
        }
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
