//! Bibliothèque de l'enseignant : modèles de TP et sondages (SPEC.md,
//! MODELE-1 à MODELE-4, SONDAGE-6).
//!
//! Les fichiers joints sont rangés dans le magasin de la console par leur
//! empreinte ([`crate::store::Store::put_blob`]) ; un modèle ne garde que
//! leur nom, leur taille et leur empreinte. L'export met modèles,
//! sondages et fichiers joints dans un seul fichier (MODELE-4).

use anyhow::{anyhow, bail, ensure, Context, Result};
use base64::Engine as _;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use surveo_platform::files::sha256_hex;
use surveo_protocol::Step;
use ts_rs::TS;

/// Format du fichier d'export (MODELE-4).
pub const EXPORT_FORMAT: &str = "surveo-bibliotheque";
pub const EXPORT_EXTENSION: &str = "surveo";

/// Fichier joint à un modèle de TP.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, TS)]
#[ts(export)]
pub struct Attachment {
    pub name: String,
    #[ts(type = "number")]
    pub size: u64,
    pub sha256: String,
}

/// Modèle de TP (MODELE-1, MODELE-2).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, TS)]
#[ts(export)]
pub struct Model {
    /// Vide pour un modèle pas encore enregistré.
    pub id: String,
    pub title: String,
    pub steps: Vec<Step>,
    /// Nombre de rendus attendus ; zéro si le TP n'en attend aucun.
    pub expected: u32,
    pub files: Vec<Attachment>,
}

/// Sondage de la bibliothèque (SONDAGE-1, SONDAGE-6).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, TS)]
#[ts(export)]
pub struct PollModel {
    pub id: String,
    pub question: String,
    pub options: Vec<String>,
    /// Choix multiple ; sinon, une seule réponse.
    pub multiple: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq, TS)]
#[ts(export)]
pub struct Library {
    pub models: Vec<Model>,
    pub polls: Vec<PollModel>,
}

/// Fichier d'export : modèles et sondages, fichiers joints compris, en
/// base 64 par empreinte.
#[derive(Serialize, Deserialize)]
pub struct Export {
    pub format: String,
    pub version: u32,
    #[serde(default)]
    pub models: Vec<Model>,
    #[serde(default)]
    pub polls: Vec<PollModel>,
    #[serde(default)]
    pub files: BTreeMap<String, String>,
}

/// Bornes d'un modèle : de quoi tenir une séance, pas davantage.
const MAX_STEPS: usize = 30;
const MAX_MINUTES: u32 = 240;
const MAX_EXPECTED: u32 = 20;
const MAX_OPTIONS: usize = 10;

/// Vérifie et nettoie un modèle avant de l'enregistrer.
pub fn check_model(mut m: Model) -> Result<Model> {
    m.title = m.title.trim().to_string();
    ensure!(!m.title.is_empty(), "Donnez un titre au modèle.");
    ensure!(!m.steps.is_empty(), "Un modèle compte au moins une étape.");
    ensure!(m.steps.len() <= MAX_STEPS, "Un modèle compte au plus {MAX_STEPS} étapes.");
    ensure!(m.expected <= MAX_EXPECTED, "Au plus {MAX_EXPECTED} rendus attendus.");
    for (i, s) in m.steps.iter_mut().enumerate() {
        s.objective = s.objective.trim().to_string();
        ensure!(!s.objective.is_empty(), "L'étape {} n'a pas d'objectif.", i + 1);
        ensure!(s.minutes <= MAX_MINUTES, "La durée d'une étape est d'au plus {MAX_MINUTES} minutes (étape {}).", i + 1);
        s.instructions = s.instructions.trim_end().to_string();
    }
    Ok(m)
}

pub fn check_poll(mut p: PollModel) -> Result<PollModel> {
    p.question = p.question.trim().to_string();
    ensure!(!p.question.is_empty(), "Écrivez la question du sondage.");
    p.options = p.options.iter().map(|o| o.trim().to_string()).filter(|o| !o.is_empty()).collect();
    ensure!(p.options.len() >= 2, "Proposez au moins deux réponses.");
    ensure!(p.options.len() <= MAX_OPTIONS, "Au plus {MAX_OPTIONS} réponses proposées.");
    Ok(p)
}

impl Library {
    pub fn model(&self, id: &str) -> Option<&Model> {
        self.models.iter().find(|m| m.id == id)
    }

    pub fn poll(&self, id: &str) -> Option<&PollModel> {
        self.polls.iter().find(|p| p.id == id)
    }

    /// Enregistre un modèle, nouveau (sans identifiant) ou modifié ; rend
    /// son identifiant.
    pub fn put_model(&mut self, m: Model) -> String {
        let mut m = m;
        if m.id.is_empty() {
            m.id = surveo_identity::random_id();
        }
        let id = m.id.clone();
        match self.models.iter_mut().find(|x| x.id == id) {
            Some(x) => *x = m,
            None => self.models.push(m),
        }
        self.models.sort_by_key(|m| m.title.to_lowercase());
        id
    }

    pub fn put_poll(&mut self, p: PollModel) -> String {
        let mut p = p;
        if p.id.is_empty() {
            p.id = surveo_identity::random_id();
        }
        let id = p.id.clone();
        match self.polls.iter_mut().find(|x| x.id == id) {
            Some(x) => *x = p,
            None => self.polls.push(p),
        }
        id
    }

    /// Copie d'un modèle, « (copie) » ajouté au titre (MODELE-1).
    pub fn duplicate_model(&mut self, id: &str) -> Result<String> {
        let mut m = self.model(id).cloned().ok_or_else(|| anyhow!("Modèle introuvable."))?;
        m.id = String::new();
        m.title = format!("{} (copie)", m.title);
        Ok(self.put_model(m))
    }

    /// Empreintes de tous les fichiers joints, pour le ménage du magasin.
    pub fn blobs(&self) -> impl Iterator<Item = &str> {
        self.models.iter().flat_map(|m| m.files.iter().map(|f| f.sha256.as_str()))
    }
}

/// Fichier d'export d'un modèle ou d'un sondage, avec le contenu des
/// fichiers joints lu par `blob`.
pub fn export(models: Vec<Model>, polls: Vec<PollModel>, blob: impl Fn(&str) -> Result<Vec<u8>>) -> Result<Export> {
    let mut files = BTreeMap::new();
    for f in models.iter().flat_map(|m| &m.files) {
        if !files.contains_key(&f.sha256) {
            let data = blob(&f.sha256).with_context(|| format!("fichier joint « {} » introuvable", f.name))?;
            files.insert(f.sha256.clone(), base64::engine::general_purpose::STANDARD.encode(data));
        }
    }
    Ok(Export { format: EXPORT_FORMAT.into(), version: 1, models, polls, files })
}

/// Lit un fichier d'export : rend les modèles et sondages, avec des
/// identifiants neufs, et le contenu vérifié de chaque fichier joint.
pub fn import(text: &str) -> Result<(Vec<Model>, Vec<PollModel>, Vec<Vec<u8>>)> {
    let e: Export = serde_json::from_str(text).map_err(|_| anyhow!("Ce fichier n'est pas un export de bibliothèque Surveo."))?;
    ensure!(e.format == EXPORT_FORMAT, "Ce fichier n'est pas un export de bibliothèque Surveo.");
    ensure!(e.version == 1, "Ce fichier vient d'une version plus récente de Surveo.");
    let mut blobs = Vec::new();
    for (sha, b64) in &e.files {
        let data = base64::engine::general_purpose::STANDARD.decode(b64).map_err(|_| anyhow!("Fichier joint illisible dans l'export."))?;
        if sha256_hex(&data) != *sha {
            bail!("Un fichier joint de l'export est abîmé.");
        }
        blobs.push(data);
    }
    let mut models = Vec::new();
    for m in e.models {
        if let Some(f) = m.files.iter().find(|f| !e.files.contains_key(&f.sha256)) {
            bail!("Le fichier joint « {} » manque dans l'export.", f.name);
        }
        models.push(Model { id: String::new(), ..check_model(m)? });
    }
    let polls = e.polls.into_iter().map(|p| check_poll(p).map(|p| PollModel { id: String::new(), ..p })).collect::<Result<_>>()?;
    Ok((models, polls, blobs))
}
