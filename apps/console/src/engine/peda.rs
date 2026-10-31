//! Pédagogie dans la séance (SPEC.md, chap. 3 ; IMPL.md, 3.1) :
//! bibliothèque, TP, distribution de fichiers, rendus, sondages.
//!
//! Comme les restrictions, le TP, les fichiers et le sondage de chaque
//! poste sont des états complets que la console renvoie à chaque
//! changement et à chaque reconnexion ; le poste tire les fichiers qui
//! lui manquent. Les rendus vont dans l'autre sens : le poste les
//! propose, la console les tire morceau par morceau, les range, puis en
//! accuse réception (RENDU-2).

use super::{plural, Engine, Pending};
use crate::library::{self, check_model, check_poll, Attachment, Model, PollModel};
use crate::store::{ArchivedSession, ArchivedTp, Distribution, PollRun, ReceivedRendu, TpRun};
use crate::view::{DistributionView, PollDetail, PollView, StationTp, TpRunView};
use crate::{ACK_TIMEOUT_MS, FETCH_TIMEOUT_MS};
use anyhow::{anyhow, ensure, Result};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use surveo_identity::random_id;
use surveo_platform::files::{self, safe_file_name, size_text, Hasher};
use surveo_protocol::*;

/// Rendu en cours de réception.
pub(super) struct Incoming {
    member: MemberId,
    offer: RenduOffer,
    /// Fichier partiel, dans `reception/`.
    part: PathBuf,
    received: u64,
    hasher: Hasher,
    /// Dernier morceau demandé, et quand.
    fetch_id: u64,
    asked_ms: u64,
}

/// « 2026-10-31 14h05 », à l'heure locale.
fn local_stamp(ms: u64, fmt: &str) -> String {
    use chrono::TimeZone;
    chrono::Local.timestamp_millis_opt(ms as i64).single().map(|t| t.format(fmt).to_string()).unwrap_or_default()
}

/// Nom de dossier libre parmi `taken`.
fn free_name(base: &str, taken: impl Fn(&str) -> bool) -> String {
    let base = safe_file_name(base);
    if !taken(&base) {
        return base;
    }
    (2..).map(|i| format!("{base} ({i})")).find(|n| !taken(n)).expect("nom libre")
}

impl Engine {
    // --- Bibliothèque (MODELE-1 à MODELE-4, SONDAGE-6) -----------------

    /// Enregistre un modèle, nouveau ou modifié ; rend son identifiant.
    pub fn save_model(&mut self, m: Model) -> Result<String> {
        let m = check_model(m)?;
        if let Some(f) = m.files.iter().find(|f| !self.store.blob(&f.sha256).exists()) {
            anyhow::bail!("Le fichier joint « {} » est introuvable : ajoutez-le de nouveau.", f.name);
        }
        let id = self.library.put_model(m);
        self.save_library();
        Ok(id)
    }

    pub fn delete_model(&mut self, id: &str) {
        self.library.models.retain(|m| m.id != id);
        self.save_library();
    }

    pub fn duplicate_model(&mut self, id: &str) -> Result<String> {
        let id = self.library.duplicate_model(id)?;
        self.save_library();
        Ok(id)
    }

    /// Range une copie d'un fichier choisi par l'enseignant, pour le
    /// joindre à un modèle ou le distribuer.
    pub fn add_file(&mut self, path: &Path) -> Result<Attachment> {
        let name = path.file_name().map(|n| n.to_string_lossy().to_string()).ok_or_else(|| anyhow!("Ce n'est pas un fichier."))?;
        let meta = std::fs::metadata(path).map_err(|e| anyhow!("« {name} » est illisible : {e}"))?;
        ensure!(meta.is_file(), "« {name} » n'est pas un fichier.");
        ensure!(meta.len() <= MAX_FILE_BYTES, "« {name} » est trop volumineux ({}) : la limite est de {}.", size_text(meta.len()), size_text(MAX_FILE_BYTES));
        let (size, sha256) = self.store.put_blob(path)?;
        Ok(Attachment { name: safe_file_name(&name), size, sha256 })
    }

    /// Même chose pour un fichier glissé sur la console, reçu en mémoire.
    pub fn add_file_bytes(&mut self, name: &str, data: &[u8]) -> Result<Attachment> {
        ensure!(data.len() as u64 <= MAX_FILE_BYTES, "« {name} » est trop volumineux ({}) : la limite est de {}.", size_text(data.len() as u64), size_text(MAX_FILE_BYTES));
        let (size, sha256) = self.store.put_blob_bytes(data)?;
        Ok(Attachment { name: safe_file_name(name), size, sha256 })
    }

    pub fn save_poll(&mut self, p: PollModel) -> Result<String> {
        let id = self.library.put_poll(check_poll(p)?);
        self.save_library();
        Ok(id)
    }

    pub fn delete_poll(&mut self, id: &str) {
        self.library.polls.retain(|p| p.id != id);
        self.save_library();
    }

    /// Fichier d'export d'un modèle ou d'un sondage, fichiers joints
    /// compris (MODELE-4).
    pub fn export_library(&self, models: &[String], polls: &[String]) -> Result<String> {
        let ms = models.iter().map(|id| self.library.model(id).cloned().ok_or_else(|| anyhow!("Modèle introuvable."))).collect::<Result<Vec<_>>>()?;
        let ps = polls.iter().map(|id| self.library.poll(id).cloned().ok_or_else(|| anyhow!("Sondage introuvable."))).collect::<Result<Vec<_>>>()?;
        let e = library::export(ms, ps, |sha| Ok(std::fs::read(self.store.blob(sha))?))?;
        Ok(serde_json::to_string_pretty(&e)?)
    }

    /// Ajoute à la bibliothèque le contenu d'un fichier d'export ; rend le
    /// nombre de modèles et de sondages ajoutés.
    pub fn import_library(&mut self, text: &str) -> Result<(usize, usize)> {
        let (models, polls, blobs) = library::import(text)?;
        for b in &blobs {
            self.store.put_blob_bytes(b)?;
        }
        let n = (models.len(), polls.len());
        for m in models {
            self.library.put_model(m);
        }
        for p in polls {
            self.library.put_poll(p);
        }
        self.save_library();
        Ok(n)
    }

    fn save_library(&mut self) {
        if let Err(e) = self.store.save_library(&self.library) {
            log::error!("enregistrement de la bibliothèque impossible : {e:#}");
            self.notice("La bibliothèque n'a pas pu être enregistrée", Some(format!("{e:#}")), true);
        }
    }

    /// Retire du magasin les fichiers que ni la bibliothèque ni la séance
    /// ne désignent plus.
    pub(super) fn sweep_blobs(&self) {
        let mut keep: BTreeSet<String> = self.library.blobs().map(Into::into).collect();
        if let Some(s) = &self.session {
            keep.extend(s.files.iter().map(|d| d.file.sha256.clone()));
            keep.extend(s.tps.iter().flat_map(|t| t.files.iter().map(|f| f.sha256.clone())));
        }
        self.store.sweep_blobs(&keep);
    }

    // --- TP (TP-1 à TP-9) ----------------------------------------------

    /// Lance un modèle sur les postes visés, sans sélection toute la
    /// séance (TP-1, TP-2). Le TP est une copie du modèle ; il remplace
    /// celui que les postes avaient (TP-4, confirmé dans l'interface).
    pub fn launch_tp(&mut self, members: &[MemberId], model: &str, now: u64) -> Result<()> {
        let m = self.library.model(model).cloned().ok_or_else(|| anyhow!("Modèle introuvable."))?;
        let s = self.session.as_mut().ok_or_else(|| anyhow!("Un TP se lance pendant une séance."))?;
        let run = TpRun {
            id: random_id(),
            model: m.id.clone(),
            title: m.title.clone(),
            steps: m.steps.clone(),
            expected: m.expected,
            files: m.files.iter().map(|f| FileOffer { id: random_id(), name: f.name.clone(), size: f.size, sha256: f.sha256.clone() }).collect(),
            launched_at_ms: now,
        };
        let sid = s.id.clone();
        s.tp.set(members, Some(run.id.clone()));
        s.tps.push(run.clone());
        self.save_session();
        if let Some(a) = self.archive.sessions.iter_mut().find(|a| a.id == sid) {
            a.tps.insert(run.id.clone(), ArchivedTp { title: run.title.clone(), folder: None });
            self.save_archive();
        }
        let targets = self.session_targets(members);
        self.send_peda(format!("TP « {} » lancé", m.title), targets, now, |e, m| e.tp_cmd(m));
        Ok(())
    }

    /// Arrête le TP des postes visés (TP-9) ; les rendus reçus restent.
    pub fn stop_tp(&mut self, members: &[MemberId], now: u64) -> Result<()> {
        let s = self.session.as_mut().ok_or_else(|| anyhow!("Aucune séance ouverte."))?;
        s.tp.set(members, None);
        self.save_session();
        let targets = self.session_targets(members);
        self.send_peda("TP arrêté".into(), targets, now, |e, m| e.tp_cmd(m));
        Ok(())
    }

    fn tp_run(&self, member: &str) -> Option<&TpRun> {
        let s = self.session.as_ref()?;
        let id = s.tp.get(member).as_ref()?;
        s.tps.iter().find(|t| &t.id == id)
    }

    pub(super) fn tp_cmd(&self, member: &str) -> Option<Command> {
        let s = self.session.as_ref()?;
        let tp = self.tp_run(member).map(|t| TpInfo { id: t.id.clone(), title: t.title.clone(), steps: t.steps.clone(), expected: t.expected, files: t.files.clone() });
        Some(Command::Tp { session: s.id.clone(), tp })
    }

    /// Envoie à chaque poste visé sa commande, avec un retour poste par
    /// poste ; un poste absent la recevra à son retour (RETOUR-2).
    fn send_peda(&mut self, label: String, targets: Vec<MemberId>, now: u64, cmd: impl Fn(&Engine, &str) -> Option<Command>) {
        let mut p = Pending::new(label, now + ACK_TIMEOUT_MS);
        p.deferred = true;
        for m in targets {
            match cmd(self, &m) {
                Some(c) if self.online.contains_key(&m) => {
                    let id = self.send(Target::Members(vec![m.clone()]), c);
                    p.ids.insert(id, m);
                }
                _ => p.offline.push(m),
            }
        }
        self.pending.push(p);
    }

    /// Avancement annoncé par un poste (TP-8) et fichiers reçus
    /// (DISTRIB-4).
    pub(super) fn agent_progress(&mut self, from: &str, tp: Option<TpProgress>, files: Vec<String>) {
        match tp {
            Some(p) => {
                self.progress.insert(from.into(), p);
            }
            None => {
                self.progress.remove(from);
            }
        }
        self.received.insert(from.into(), files.into_iter().collect());
    }

    /// TP d'un poste et son avancement, pour sa tuile (TP-8, AFF-2).
    pub(super) fn station_tp(&self, member: &str) -> Option<StationTp> {
        let t = self.tp_run(member)?;
        let p = self.progress.get(member).filter(|p| p.tp == t.id);
        let done: BTreeSet<u16> = p.map(|p| p.done.iter().copied().collect()).unwrap_or_default();
        // L'étape en cours est la première non terminée (TP-6).
        let step = (0..t.steps.len() as u16).find(|i| !done.contains(i));
        Some(StationTp {
            run: t.id.clone(),
            title: t.title.clone(),
            step: step.map(u32::from),
            steps: t.steps.len() as u32,
            objective: step.map(|i| t.steps[i as usize].objective.clone()),
            late: p.is_some_and(|p| p.late),
            rendus: self.rendus_count(member, &t.id),
            expected: t.expected,
            known: p.is_some(),
        })
    }

    /// Synthèse de chaque TP en cours : postes à chaque étape, retards,
    /// rendus complets (TP-8).
    pub(super) fn tps_view(&self) -> Vec<TpRunView> {
        let Some(s) = &self.session else { return vec![] };
        let members = self.session_targets(&[]);
        let mut out = Vec::new();
        for t in &s.tps {
            let tiles: Vec<StationTp> = members.iter().filter_map(|m| self.station_tp(m)).filter(|x| x.run == t.id).collect();
            if tiles.is_empty() {
                continue;
            }
            let mut at_step = vec![0u32; t.steps.len()];
            for x in &tiles {
                if let Some(i) = x.step {
                    at_step[i as usize] += 1;
                }
            }
            out.push(TpRunView {
                id: t.id.clone(),
                model: t.model.clone(),
                title: t.title.clone(),
                steps: t.steps.iter().map(|x| x.objective.clone()).collect(),
                expected: t.expected,
                stations: tiles.len() as u32,
                at_step,
                finished: tiles.iter().filter(|x| x.known && x.step.is_none()).count() as u32,
                late: tiles.iter().filter(|x| x.late).count() as u32,
                complete: tiles.iter().filter(|x| t.expected > 0 && x.rendus >= t.expected).count() as u32,
                all: s.tp.all.as_ref() == Some(&t.id),
            });
        }
        out
    }

    // --- Distribution (DISTRIB-1 à DISTRIB-5) --------------------------

    /// Distribue des fichiers, rangés au préalable par [`Engine::add_file`],
    /// aux postes visés ; sans sélection à toute la séance (DISTRIB-1).
    pub fn distribute(&mut self, members: &[MemberId], files: Vec<Attachment>, now: u64) -> Result<()> {
        ensure!(!files.is_empty(), "Aucun fichier à distribuer.");
        let s = self.session.as_mut().ok_or_else(|| anyhow!("Les fichiers se distribuent pendant une séance."))?;
        let to = (!members.is_empty()).then(|| members.iter().cloned().collect::<BTreeSet<_>>());
        for f in &files {
            s.files.push(Distribution { file: FileOffer { id: random_id(), name: f.name.clone(), size: f.size, sha256: f.sha256.clone() }, to: to.clone(), at_ms: now });
        }
        self.save_session();
        let label = match &files[..] {
            [f] => format!("« {} » distribué", f.name),
            _ => format!("{} distribués", plural(files.len(), "fichier")),
        };
        let targets = self.session_targets(members);
        self.send_peda(label, targets, now, |e, m| e.files_cmd(m));
        Ok(())
    }

    fn files_for(&self, member: &str) -> Vec<FileOffer> {
        let Some(s) = &self.session else { return vec![] };
        s.files.iter().filter(|d| d.to.as_ref().is_none_or(|t| t.contains(member))).map(|d| d.file.clone()).collect()
    }

    pub(super) fn files_cmd(&self, member: &str) -> Option<Command> {
        let s = self.session.as_ref()?;
        let files = self.files_for(member);
        (!files.is_empty()).then(|| Command::Files { session: s.id.clone(), files })
    }

    /// Un poste tire un morceau d'un fichier distribué ou joint
    /// (DISTRIB-4).
    pub(super) fn serve_fetch(&mut self, from: &str, file: &str, offset: u64, len: u32) {
        let offer = self.files_for(from).into_iter().chain(self.tp_run(from).map(|t| t.files.clone()).unwrap_or_default()).find(|f| f.id == file);
        let Some(offer) = offer else {
            // Fichier qui ne lui est plus destiné : le poste reçoit l'état à
            // jour et oublie ce téléchargement.
            let cmds = [self.tp_cmd(from), self.files_cmd(from)];
            for c in cmds.into_iter().flatten() {
                self.send(Target::Members(vec![from.into()]), c);
            }
            return;
        };
        let len = len.min(CHUNK_SIZE) as usize;
        match files::read_at(&self.store.blob(&offer.sha256), offset, len) {
            Ok(data) => self.send_binary(from, BinKind::Chunk, &encode_chunk(file, offset, &data)),
            Err(e) => log::warn!("fichier « {} » illisible : {e:#}", offer.name),
        }
    }

    pub(super) fn distributions_view(&self) -> Vec<DistributionView> {
        let Some(s) = &self.session else { return vec![] };
        let all = self.session_targets(&[]);
        s.files
            .iter()
            .map(|d| {
                let targets: Vec<&MemberId> = match &d.to {
                    Some(t) => t.iter().collect(),
                    None => all.iter().collect(),
                };
                let received = targets.iter().filter(|m| self.received.get(**m).is_some_and(|r| r.contains(&d.file.id))).count();
                DistributionView {
                    id: d.file.id.clone(),
                    name: d.file.name.clone(),
                    size: d.file.size,
                    all: d.to.is_none(),
                    targets: targets.len() as u32,
                    received: received as u32,
                    at_ms: d.at_ms,
                }
            })
            .collect()
    }

    /// Fichiers distribués à un poste (avec ceux de son TP), et combien il
    /// en a reçus.
    pub(super) fn station_files(&self, member: &str) -> (u32, u32) {
        let all: Vec<FileOffer> = self.files_for(member).into_iter().chain(self.tp_run(member).map(|t| t.files.clone()).unwrap_or_default()).collect();
        let got = self.received.get(member);
        let n = all.iter().filter(|f| got.is_some_and(|g| g.contains(&f.id))).count();
        (all.len() as u32, n as u32)
    }

    // --- Rendus (RENDU-1 à RENDU-8) ------------------------------------

    /// Entrée de la séance dans l'index des rendus, à son ouverture : son
    /// dossier est « 2026-10-31 14h05 B204 » (RENDU-6).
    pub(super) fn archive_session(&mut self, now: u64) {
        let Some(s) = &self.session else { return };
        let base = format!("{} {}", local_stamp(now, "%Y-%m-%d %Hh%M"), self.room_name(&s.room));
        let root = self.store.rendus.clone();
        let folder = free_name(&base, |n| self.archive.sessions.iter().any(|a| a.folder == n) || root.join(n).exists());
        self.archive.sessions.push(ArchivedSession { id: s.id.clone(), folder, tps: Default::default(), stations: Default::default(), rendus: vec![] });
        self.save_archive();
    }

    fn save_archive(&mut self) {
        if let Err(e) = self.store.save_archive(&self.archive) {
            log::error!("index des rendus impossible à enregistrer : {e:#}");
        }
    }

    /// Rendus reçus d'un poste pour un TP de la séance en cours : un même
    /// nom rendu plusieurs fois compte une fois (RENDU-4, RENDU-5).
    fn rendus_count(&self, member: &str, tp: &str) -> u32 {
        let Some(s) = &self.session else { return 0 };
        let Some(a) = self.archive.sessions.iter().find(|a| a.id == s.id) else { return 0 };
        a.rendus.iter().filter(|r| r.member == member && r.tp == tp).map(|r| r.name.to_lowercase()).collect::<BTreeSet<_>>().len() as u32
    }

    /// Un poste propose un rendu : la console le tire, sauf s'il est déjà
    /// rangé (accusé de réception perdu) ou refusé (RENDU-7).
    pub(super) fn offer_rendu(&mut self, from: &str, offer: RenduOffer, now: u64) {
        let id = offer.file.id.clone();
        let known = self.archive.sessions.iter().find(|a| a.id == offer.session);
        if known.is_some_and(|a| a.rendus.iter().any(|r| r.id == id)) {
            self.send(Target::Members(vec![from.into()]), Command::Receipt { rendu: id, error: None });
            return;
        }
        let refuse = if offer.file.size > MAX_RENDU_BYTES {
            Some(format!(
                "« {} » est trop volumineux ({}) : la limite est de {}.",
                offer.file.name,
                size_text(offer.file.size),
                size_text(MAX_RENDU_BYTES)
            ))
        } else if !known.is_some_and(|a| a.tps.contains_key(&offer.tp)) {
            Some("Ce rendu ne correspond à aucun TP connu de l'enseignant.".into())
        } else {
            None
        };
        if let Some(error) = refuse {
            self.send(Target::Members(vec![from.into()]), Command::Receipt { rendu: id, error: Some(error) });
            return;
        }
        // Proposé de nouveau pendant sa réception : la console reprend où
        // elle en était.
        let (offset, fresh) = match self.incoming.get(&id) {
            Some(i) if i.member == from => (i.received, false),
            _ => (0, true),
        };
        if fresh {
            let part = self.store.dir.join("reception").join(format!("{id}.part"));
            if let Err(e) = std::fs::create_dir_all(part.parent().unwrap()).and_then(|_| std::fs::File::create(&part).map(|_| ())) {
                log::error!("réception d'un rendu impossible : {e}");
                return;
            }
            self.incoming.insert(id.clone(), Incoming { member: from.into(), offer, part, received: 0, hasher: Hasher::default(), fetch_id: 0, asked_ms: now });
        }
        self.fetch_next(&id, offset, now);
    }

    fn fetch_next(&mut self, id: &str, offset: u64, now: u64) {
        let Some(i) = self.incoming.get(id) else { return };
        let len = (i.offer.file.size - offset).min(CHUNK_SIZE as u64) as u32;
        let member = i.member.clone();
        let fetch = self.send(Target::Members(vec![member]), Command::Fetch { file: id.into(), offset, len });
        if let Some(i) = self.incoming.get_mut(id) {
            i.fetch_id = fetch;
            i.asked_ms = now;
        }
    }

    /// Le poste ne connaît plus le rendu demandé.
    pub(super) fn fetch_refused(&mut self, from: &str, id: u64, detail: Option<&str>) {
        let gone: Vec<String> = self.incoming.iter().filter(|(_, i)| i.member == from && i.fetch_id == id).map(|(k, _)| k.clone()).collect();
        for k in gone {
            log::warn!("rendu {k} abandonné par le poste : {}", detail.unwrap_or(""));
            if let Some(i) = self.incoming.remove(&k) {
                let _ = std::fs::remove_file(i.part);
            }
        }
    }

    /// Morceau d'un rendu (trame [`BinKind::Chunk`] du poste `from`).
    pub fn handle_chunk(&mut self, from: &str, payload: &[u8], now: u64) {
        let Some((id, offset, data)) = decode_chunk(payload) else { return };
        let id = id.to_string();
        let Some(i) = self.incoming.get_mut(&id).filter(|i| i.member == from) else { return };
        // Un morceau en double ou en retard est ignoré.
        if offset != i.received || data.is_empty() || i.received + data.len() as u64 > i.offer.file.size {
            return;
        }
        let written = std::fs::OpenOptions::new().append(true).open(&i.part).and_then(|mut f| std::io::Write::write_all(&mut f, data));
        if let Err(e) = written {
            log::error!("rendu « {} » : écriture impossible ({e})", i.offer.file.name);
            return;
        }
        i.hasher.update(data);
        i.received += data.len() as u64;
        if i.received < i.offer.file.size {
            let next = i.received;
            self.fetch_next(&id, next, now);
            return;
        }
        let i = self.incoming.remove(&id).expect("rendu en cours");
        let error = match self.store_rendu(&i, now) {
            Ok(path) => {
                log::info!("rendu rangé : {}", path.display());
                None
            }
            Err(e) => {
                let _ = std::fs::remove_file(&i.part);
                log::warn!("rendu « {} » refusé : {e:#}", i.offer.file.name);
                Some(format!("{e:#}"))
            }
        };
        self.send(Target::Members(vec![i.member.clone()]), Command::Receipt { rendu: id, error });
    }

    /// Range un rendu reçu : séance, puis TP, puis poste (RENDU-6). Une
    /// version précédente du même nom est gardée, renommée avec l'heure
    /// de sa réception ; la plus récente garde le nom (RENDU-5).
    fn store_rendu(&mut self, i: &Incoming, now: u64) -> Result<PathBuf> {
        let sha = i.hasher.clone().finish();
        ensure!(sha == i.offer.file.sha256, "« {} » est arrivé abîmé : il sera renvoyé.", i.offer.file.name);
        let station = format!("{} ({})", self.station_name(&i.member), self.station_label(&i.member));
        let root = self.store.rendus.clone();
        let a = self.archive.sessions.iter_mut().find(|a| a.id == i.offer.session).ok_or_else(|| anyhow!("Séance inconnue."))?;
        let session_dir = root.join(&a.folder);
        let taken: BTreeSet<String> = a.tps.values().filter_map(|t| t.folder.clone()).collect();
        let tp = a.tps.get_mut(&i.offer.tp).ok_or_else(|| anyhow!("TP inconnu."))?;
        let tp_folder = match &tp.folder {
            Some(f) => f.clone(),
            None => {
                let f = free_name(&tp.title, |n| taken.contains(n) || session_dir.join(n).exists());
                tp.folder = Some(f.clone());
                f
            }
        };
        let station_folder = a.stations.entry(i.member.clone()).or_insert_with(|| safe_file_name(&station)).clone();
        let dir = session_dir.join(tp_folder).join(station_folder);
        std::fs::create_dir_all(&dir)?;
        let name = safe_file_name(&i.offer.file.name);
        let dest = dir.join(&name);
        if dest.exists() {
            let before = a.rendus.iter().rev().find(|r| r.member == i.member && r.tp == i.offer.tp && r.name == name).map_or(now, |r| r.at_ms);
            let (stem, ext) = files::split_ext(&name);
            let old = files::unique_path(&dir, &format!("{stem} ({}){ext}", local_stamp(before, "%Hh%Mm%S")));
            std::fs::rename(&dest, old)?;
        }
        files::move_file(&i.part, &dest)?;
        a.rendus.push(ReceivedRendu { id: i.offer.file.id.clone(), member: i.member.clone(), tp: i.offer.tp.clone(), name, at_ms: now });
        self.save_archive();
        Ok(dest)
    }

    /// Un rendu dont aucun morceau n'arrive est redemandé ; si le poste
    /// est parti, il le proposera de nouveau à son retour (RENDU-3).
    pub(super) fn rendus_tick(&mut self, now: u64) {
        let stalled: Vec<(String, u64)> = self
            .incoming
            .iter()
            .filter(|(_, i)| now >= i.asked_ms + FETCH_TIMEOUT_MS && self.online.contains_key(&i.member))
            .map(|(k, i)| (k.clone(), i.received))
            .collect();
        for (id, offset) in stalled {
            self.fetch_next(&id, offset, now);
        }
    }

    /// Dossier des rendus de la séance en cours, ou de la dernière, ou
    /// d'un de ses postes (RENDU-6) ; créé s'il n'existe pas encore.
    pub fn rendus_dir(&self, member: Option<&str>) -> Result<PathBuf> {
        let a = match &self.session {
            Some(s) => self.archive.sessions.iter().find(|a| a.id == s.id),
            None => self.archive.sessions.last(),
        };
        let Some(a) = a else {
            std::fs::create_dir_all(&self.store.rendus)?;
            return Ok(self.store.rendus.clone());
        };
        let mut dir = self.store.rendus.join(&a.folder);
        if let Some(m) = member {
            // Le dossier du poste dans le TP le plus récent où il a rendu.
            if let Some(r) = a.rendus.iter().rev().find(|r| r.member == m) {
                if let (Some(Some(tp)), Some(st)) = (a.tps.get(&r.tp).map(|t| t.folder.clone()), a.stations.get(m)) {
                    dir = dir.join(tp).join(st);
                }
            }
        }
        std::fs::create_dir_all(&dir)?;
        Ok(dir)
    }

    pub fn rendus_root(&self) -> &Path {
        &self.store.rendus
    }

    // --- Sondages (SONDAGE-1 à SONDAGE-6) ------------------------------

    /// Lance un sondage, enregistré au passage dans la bibliothèque
    /// (SONDAGE-6), sur les postes visés ; une durée limite le clôt de
    /// lui-même (SONDAGE-4). Un sondage encore ouvert est clos.
    pub fn launch_poll(&mut self, members: &[MemberId], poll: PollModel, minutes: Option<u32>, now: u64) -> Result<()> {
        ensure!(self.session.is_some(), "Un sondage se lance pendant une séance.");
        ensure!(minutes.is_none_or(|m| (1..=120).contains(&m)), "La durée d'un sondage va de 1 à 120 minutes.");
        let id = self.save_poll(poll)?;
        let p = self.library.poll(&id).cloned().expect("sondage enregistré");
        let s = self.session.as_mut().expect("séance ouverte");
        s.poll = Some(PollRun {
            id: random_id(),
            question: p.question.clone(),
            options: p.options,
            multiple: p.multiple,
            to: (!members.is_empty()).then(|| members.iter().cloned().collect()),
            open: true,
            closes_at_ms: minutes.map(|m| now + m as u64 * 60_000),
            shown: false,
            answers: Default::default(),
        });
        self.save_session();
        let targets = self.poll_targets();
        self.send_peda("Sondage lancé".into(), targets, now, |e, m| e.poll_cmd(m, now));
        Ok(())
    }

    /// Clôt le sondage : la question disparaît chez l'élève (SONDAGE-4).
    pub fn close_poll(&mut self, now: u64) -> Result<()> {
        let p = self.session.as_mut().and_then(|s| s.poll.as_mut()).ok_or_else(|| anyhow!("Aucun sondage en cours."))?;
        p.open = false;
        self.save_session();
        self.resend_poll(now);
        Ok(())
    }

    /// Montre ou cache les résultats aux élèves, toujours agrégés et sans
    /// nom (SONDAGE-5).
    pub fn show_poll(&mut self, shown: bool, now: u64) -> Result<()> {
        let p = self.session.as_mut().and_then(|s| s.poll.as_mut()).ok_or_else(|| anyhow!("Aucun sondage en cours."))?;
        p.shown = shown;
        self.save_session();
        self.resend_poll(now);
        Ok(())
    }

    /// Retire le sondage de la séance, chez l'enseignant comme chez
    /// l'élève.
    pub fn clear_poll(&mut self) -> Result<()> {
        let targets = self.poll_targets();
        let s = self.session.as_mut().ok_or_else(|| anyhow!("Aucune séance ouverte."))?;
        s.poll = None;
        let sid = s.id.clone();
        self.save_session();
        let online: Vec<MemberId> = targets.into_iter().filter(|m| self.online.contains_key(m)).collect();
        for m in online {
            self.send(Target::Members(vec![m]), Command::Poll { session: sid.clone(), poll: None });
        }
        Ok(())
    }

    fn poll_targets(&self) -> Vec<MemberId> {
        match self.session.as_ref().and_then(|s| s.poll.as_ref()) {
            Some(PollRun { to: Some(t), .. }) => t.iter().cloned().collect(),
            Some(_) => self.session_targets(&[]),
            None => vec![],
        }
    }

    fn poll_counts(p: &PollRun) -> Vec<u32> {
        let mut counts = vec![0u32; p.options.len()];
        for c in p.answers.values().flatten() {
            if let Some(n) = counts.get_mut(*c as usize) {
                *n += 1;
            }
        }
        counts
    }

    pub(super) fn poll_cmd(&self, member: &str, now: u64) -> Option<Command> {
        let s = self.session.as_ref()?;
        let p = s.poll.as_ref()?;
        if p.to.as_ref().is_some_and(|t| !t.contains(member)) {
            return None;
        }
        // Clos et résultats cachés : plus rien chez l'élève.
        let poll = (p.open || p.shown).then(|| PollInfo {
            id: p.id.clone(),
            question: p.question.clone(),
            options: p.options.clone(),
            multiple: p.multiple,
            open: p.open,
            closes_in_ms: p.closes_at_ms.filter(|_| p.open).map(|t| t.saturating_sub(now)),
            answer: p.answers.get(member).cloned(),
            results: p.shown.then(|| Self::poll_counts(p)),
        });
        Some(Command::Poll { session: s.id.clone(), poll })
    }

    fn resend_poll(&mut self, now: u64) {
        let online: Vec<MemberId> = self.poll_targets().into_iter().filter(|m| self.online.contains_key(m)).collect();
        for m in online {
            if let Some(c) = self.poll_cmd(&m, now) {
                self.send(Target::Members(vec![m]), c);
            }
        }
    }

    /// Réponse d'un élève ; il peut la changer tant que le sondage est
    /// ouvert (SONDAGE-2).
    pub(super) fn answer_poll(&mut self, from: &str, poll: &str, choices: Vec<u16>, now: u64) {
        let targeted = self.poll_targets().iter().any(|m| m == from);
        let Some(p) = self.session.as_mut().and_then(|s| s.poll.as_mut()) else { return };
        let mut choices: Vec<u16> = choices.into_iter().filter(|c| (*c as usize) < p.options.len()).collect::<BTreeSet<_>>().into_iter().collect();
        if p.id != poll || !p.open || !targeted || choices.is_empty() {
            return;
        }
        if !p.multiple {
            choices.truncate(1);
        }
        let shown = p.shown;
        p.answers.insert(from.into(), choices);
        self.save_session();
        // Les résultats montrés suivent les réponses.
        if shown {
            self.resend_poll(now);
        }
    }

    /// Clôture à la durée limite (SONDAGE-4). Rend `true` si l'affichage
    /// a changé.
    pub(super) fn poll_tick(&mut self, now: u64) -> bool {
        let due = self.session.as_ref().and_then(|s| s.poll.as_ref()).is_some_and(|p| p.open && p.closes_at_ms.is_some_and(|t| now >= t));
        if due {
            let _ = self.close_poll(now);
            self.notice("Le sondage est clos", Some("Sa durée limite est atteinte.".into()), false);
        }
        due
    }

    pub(super) fn poll_view(&self) -> Option<PollView> {
        let s = self.session.as_ref()?;
        let p = s.poll.as_ref()?;
        let targets = self.poll_targets();
        let mut details: Vec<PollDetail> =
            p.answers.iter().map(|(m, c)| PollDetail { station: self.station_name(m), choices: c.clone() }).collect();
        details.sort_by(|a, b| a.station.cmp(&b.station));
        Some(PollView {
            id: p.id.clone(),
            question: p.question.clone(),
            options: p.options.clone(),
            multiple: p.multiple,
            open: p.open,
            shown: p.shown,
            closes_at_ms: p.closes_at_ms,
            counts: Self::poll_counts(p),
            answered: p.answers.len() as u32,
            targets: targets.len() as u32,
            all: p.to.is_none(),
            details,
        })
    }

    /// Le poste a-t-il répondu au sondage en cours ? Aucun : il n'est pas
    /// interrogé.
    pub(super) fn station_answered(&self, member: &str) -> Option<bool> {
        let p = self.session.as_ref()?.poll.as_ref()?;
        if !p.open || p.to.as_ref().is_some_and(|t| !t.contains(member)) {
            return None;
        }
        Some(p.answers.contains_key(member))
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::{engine, in_session, presence, Recorder, T0};
    use super::*;
    use crate::store::Store;
    use base64::Engine as _;

    fn model(e: &mut Engine, title: &str, files: Vec<Attachment>) -> String {
        let step = |o: &str| Step { objective: o.into(), instructions: "**Consigne**".into(), minutes: 10, overrun: Overrun::ShowInstructions };
        e.save_model(Model { id: String::new(), title: title.into(), steps: vec![step("Adresser"), step("Tester"), step("Rédiger")], expected: 2, files }).unwrap()
    }

    fn attachment(e: &mut Engine, dir: &Path, name: &str, data: &[u8]) -> Attachment {
        let p = dir.join(name);
        std::fs::write(&p, data).unwrap();
        e.add_file(&p).unwrap()
    }

    /// Commandes `Tp` envoyées, par poste.
    fn tps(rec: &Recorder) -> Vec<(MemberId, Option<TpInfo>)> {
        rec.take()
            .into_iter()
            .filter_map(|(to, cmd)| match (to, cmd) {
                (Target::Members(m), Command::Tp { tp, .. }) => Some((m[0].clone(), tp)),
                _ => None,
            })
            .collect()
    }

    fn progress(e: &mut Engine, member: &str, sid: &str, tp: &str, done: Vec<u16>, late: bool, files: Vec<String>) {
        let msg = AgentMsg::Status { session: Some(sid.into()), screen: ScreenState::Off, web: WebState::Off, tp: Some(TpProgress { tp: tp.into(), done, late }), files };
        e.handle(ToConsole::FromAgent { from: member.into(), msg }, T0);
    }

    #[test]
    fn models_are_exported_and_imported_identically() {
        let (mut e, _, dir) = engine();
        let a = attachment(&mut e, &dir, "consignes.pdf", b"%PDF du TP");
        let id = model(&mut e, "Adressage IPv4", vec![a.clone()]);
        assert!(e.save_model(Model { id: String::new(), title: " ".into(), steps: vec![], expected: 0, files: vec![] }).is_err());
        let copy = e.duplicate_model(&id).unwrap();
        assert_eq!(e.library.model(&copy).unwrap().title, "Adressage IPv4 (copie)");
        let poll = e.save_poll(PollModel { id: String::new(), question: "Où en êtes-vous ?".into(), options: vec!["Fini".into(), " ".into(), "Bloqué".into()], multiple: false }).unwrap();
        assert_eq!(e.library.poll(&poll).unwrap().options, ["Fini", "Bloqué"]);

        // Un collègue importe l'export sur une autre console (MODELE-4).
        let text = e.export_library(&[id.clone()], &[poll]).unwrap();
        let (mut other, _, other_dir) = engine();
        assert_eq!(other.import_library(&text).unwrap(), (1, 1));
        let m = other.library.models[0].clone();
        assert_eq!(Model { id: id.clone(), ..m.clone() }, *e.library.model(&id).unwrap());
        assert_eq!(std::fs::read(Store::new(other_dir).blob(&m.files[0].sha256)).unwrap(), b"%PDF du TP");
        assert!(other.import_library("{}").is_err());
        let damaged = text.replace(&base64::engine::general_purpose::STANDARD.encode(b"%PDF du TP"), &base64::engine::general_purpose::STANDARD.encode(b"autre"));
        assert!(other.import_library(&damaged).is_err());

        // Le magasin ne garde que les fichiers encore utiles.
        e.delete_model(&id);
        e.delete_model(&copy);
        e.sweep_blobs();
        assert!(!e.store.blob(&a.sha256).exists());
    }

    #[test]
    fn tp_reaches_the_room_then_newcomers_and_can_be_replaced() {
        let (mut e, rec, dir) = engine();
        let sid = in_session(&mut e, &["a", "b"]);
        let f = attachment(&mut e, &dir, "topologie.pkt", b"pkt");
        let reseau = model(&mut e, "Réseau", vec![f]);
        let vlan = model(&mut e, "VLAN", vec![]);
        rec.take();

        e.launch_tp(&[], &reseau, T0).unwrap();
        let sent = tps(&rec);
        assert_eq!(sent.len(), 2);
        let tp = sent[0].1.clone().unwrap();
        assert_eq!((tp.title.as_str(), tp.steps.len(), tp.expected, tp.files[0].name.as_str()), ("Réseau", 3, 2, "topologie.pkt"));

        // Modifier le modèle ne change pas le TP en cours (TP-1).
        let mut m = e.library.model(&reseau).unwrap().clone();
        m.title = "Réseau v2".into();
        e.save_model(m).unwrap();

        // Un poste qui rejoint la séance reçoit le TP (TP-2).
        let p = presence(&e, "c");
        e.handle(ToConsole::Online { agent: p }, T0);
        assert_eq!(tps(&rec), vec![("c".to_string(), Some(tp.clone()))]);

        // Avancement annoncé par le poste (TP-8).
        progress(&mut e, "a", &sid, &tp.id, vec![0], true, vec![]);
        let v = e.view();
        let a = v.room.as_ref().unwrap().stations[0].tp.clone().unwrap();
        assert_eq!((a.step, a.steps, a.late, a.objective.as_deref(), a.title.as_str()), (Some(1), 3, true, Some("Tester"), "Réseau"));
        let run = &v.session.as_ref().unwrap().tps[0];
        // Un poste qui n'a encore rien annoncé est à la première étape.
        assert_eq!((run.stations, run.at_step.clone(), run.late, run.all), (3, vec![2, 1, 0], 1, true));

        // Un autre TP sur un poste remplace le premier (TP-4) ; arrêt sur un
        // autre (TP-9).
        e.launch_tp(&["b".into()], &vlan, T0).unwrap();
        assert_eq!(tps(&rec).iter().map(|(m, t)| (m.as_str(), t.as_ref().map(|t| t.title.clone()))).collect::<Vec<_>>(), [("b", Some("VLAN".into()))]);
        e.stop_tp(&["a".into()], T0).unwrap();
        assert_eq!(tps(&rec), vec![("a".to_string(), None)]);
        let titles: Vec<Option<String>> = e.view().room.unwrap().stations.iter().map(|s| s.tp.as_ref().map(|t| t.title.clone())).collect();
        assert_eq!(titles, [None, Some("VLAN".into()), Some("Réseau".into())]);
    }

    /// Le poste `member` répond à chaque demande de morceau avec `data`.
    fn serve_rendu(e: &mut Engine, rec: &Recorder, member: &str, data: &[u8]) -> Vec<Command> {
        let mut others = vec![];
        for _ in 0..20 {
            let mut fetched = false;
            for (_, cmd) in rec.ids() {
                match cmd {
                    Command::Fetch { file, offset, len } => {
                        fetched = true;
                        let end = (offset as usize + len as usize).min(data.len());
                        e.handle_chunk(member, &encode_chunk(&file, offset, &data[offset as usize..end]), T0 + 1000);
                    }
                    c => others.push(c),
                }
            }
            if !fetched {
                break;
            }
        }
        others
    }

    fn offer(e: &mut Engine, member: &str, sid: &str, tp: &str, id: &str, name: &str, data: &[u8]) {
        let file = FileOffer { id: id.into(), name: name.into(), size: data.len() as u64, sha256: files::sha256_hex(data) };
        e.handle(ToConsole::FromAgent { from: member.into(), msg: AgentMsg::Submit { rendu: RenduOffer { session: sid.into(), tp: tp.into(), file } } }, T0);
    }

    #[test]
    fn rendus_are_pulled_then_stored_by_session_tp_and_station() {
        let (mut e, rec, _) = engine();
        let sid = in_session(&mut e, &["a"]);
        let m = model(&mut e, "Adressage IPv4", vec![]);
        e.launch_tp(&[], &m, T0).unwrap();
        let tp = e.session.as_ref().unwrap().tps[0].id.clone();
        rec.take();

        // Plusieurs morceaux, tirés un par un (IMPL.md, 3.1).
        let v1: Vec<u8> = (0..(CHUNK_SIZE as usize * 2 + 10)).map(|i| i as u8).collect();
        offer(&mut e, "a", &sid, &tp, "r1", "compte-rendu.odt", &v1);
        let after = serve_rendu(&mut e, &rec, "a", &v1);
        assert_eq!(after, [Command::Receipt { rendu: "r1".into(), error: None }]);
        let a = e.archive.sessions.last().unwrap().clone();
        let dir = e.store.rendus.join(&a.folder).join("Adressage IPv4").join("poste 01 (PC-a)");
        assert!(a.folder.ends_with(" B204"), "{}", a.folder);
        assert_eq!(std::fs::read(dir.join("compte-rendu.odt")).unwrap(), v1);

        // Même nom rendu de nouveau : les deux versions restent (RENDU-5).
        offer(&mut e, "a", &sid, &tp, "r2", "compte-rendu.odt", b"v2");
        serve_rendu(&mut e, &rec, "a", b"v2");
        let mut names: Vec<String> = std::fs::read_dir(&dir).unwrap().map(|x| x.unwrap().file_name().to_string_lossy().to_string()).collect();
        names.sort();
        assert_eq!(names.len(), 2);
        assert_eq!(std::fs::read(dir.join("compte-rendu.odt")).unwrap(), b"v2");
        assert!(names[0].starts_with("compte-rendu (") && names[0].ends_with(").odt"), "{names:?}");
        assert_eq!(e.view().room.unwrap().stations[0].tp.as_ref().unwrap().rendus, 1, "deux versions d'un même rendu");

        // Accusé perdu : le rendu proposé de nouveau n'est pas rangé deux fois.
        offer(&mut e, "a", &sid, &tp, "r2", "compte-rendu.odt", b"v2");
        assert_eq!(rec.take(), [(Target::Members(vec!["a".into()]), Command::Receipt { rendu: "r2".into(), error: None })]);

        // Trop volumineux (RENDU-7), ou abîmé en route.
        let big = FileOffer { id: "r3".into(), name: "film.mp4".into(), size: MAX_RENDU_BYTES + 1, sha256: String::new() };
        e.handle(ToConsole::FromAgent { from: "a".into(), msg: AgentMsg::Submit { rendu: RenduOffer { session: sid.clone(), tp: tp.clone(), file: big } } }, T0);
        match &rec.take()[..] {
            [(_, Command::Receipt { error: Some(m), .. })] => assert_eq!(m, "« film.mp4 » est trop volumineux (50 Mo) : la limite est de 50 Mo."),
            other => panic!("{other:?}"),
        }

        // Après la clôture, un rendu déposé pendant la séance arrive encore
        // (RENDU-3, RENDU-8) ; le transfert reprend s'il est interrompu.
        e.close_session(T0 + 2000, false).unwrap();
        rec.take();
        offer(&mut e, "a", &sid, &tp, "r4", "annexe.txt", b"annexe en retard");
        let first = rec.ids();
        assert!(matches!(&first[..], [(_, Command::Fetch { offset: 0, .. })]));
        e.rendus_tick(T0 + 2000 + FETCH_TIMEOUT_MS);
        assert!(matches!(&rec.ids()[..], [(_, Command::Fetch { offset: 0, .. })]), "morceau redemandé");
        offer(&mut e, "a", &sid, &tp, "r4", "annexe.txt", b"annexe en retard");
        serve_rendu(&mut e, &rec, "a", b"annexe en retard");
        assert_eq!(std::fs::read(dir.join("annexe.txt")).unwrap(), b"annexe en retard");
        assert_eq!(e.rendus_dir(Some("a")).unwrap(), dir);
    }

    #[test]
    fn distributed_files_reach_targets_and_are_served_by_chunks() {
        let (mut e, rec, dir) = engine();
        let sid = in_session(&mut e, &["a", "b"]);
        let f = attachment(&mut e, &dir, "cours.pdf", b"contenu du cours");
        rec.take();
        e.distribute(&["a".into()], vec![f.clone()], T0).unwrap();
        let sent = rec.take();
        let offer = match &sent[..] {
            [(Target::Members(m), Command::Files { files, .. })] if m == &["a".to_string()] => files[0].clone(),
            other => panic!("{other:?}"),
        };
        assert_eq!((offer.name.as_str(), offer.size), ("cours.pdf", 16));

        // Le poste tire le fichier par morceaux.
        e.handle(ToConsole::FromAgent { from: "a".into(), msg: AgentMsg::Fetch { file: offer.id.clone(), offset: 8, len: CHUNK_SIZE } }, T0);
        let frame = rec.1.lock().unwrap().pop().unwrap();
        let (kind, member, payload) = decode_console_frame(&frame).unwrap();
        assert_eq!((kind, member), (BinKind::Chunk, "a"));
        assert_eq!(decode_chunk(payload), Some((offer.id.as_str(), 8, &b"du cours"[..])));
        // Un fichier qui ne lui est pas destiné : il reçoit l'état à jour.
        e.handle(ToConsole::FromAgent { from: "b".into(), msg: AgentMsg::Fetch { file: offer.id.clone(), offset: 0, len: 10 } }, T0);
        assert!(rec.1.lock().unwrap().is_empty());

        // Toute la séance, y compris un poste qui arrive ensuite (DISTRIB-4).
        e.distribute(&[], vec![f], T0).unwrap();
        rec.take();
        let p = presence(&e, "c");
        e.handle(ToConsole::Online { agent: p }, T0);
        let late: Vec<usize> = rec.take().into_iter().filter_map(|(_, c)| match c {
            Command::Files { files, .. } => Some(files.len()),
            _ => None,
        }).collect();
        assert_eq!(late, [1]);

        progress(&mut e, "a", &sid, "", vec![], false, vec![offer.id.clone()]);
        let v = e.view();
        let st = &v.room.as_ref().unwrap().stations;
        assert_eq!((st[0].files_total, st[0].files_received, st[1].files_total), (2, 1, 1));
        let d = &v.session.as_ref().unwrap().files;
        assert_eq!((d[0].targets, d[0].received, d[1].all, d[1].targets), (1, 1, true, 3));
    }

    #[test]
    fn poll_results_are_live_and_shown_without_names() {
        let (mut e, rec, _) = engine();
        in_session(&mut e, &["a", "b", "c"]);
        rec.take();
        let q = PollModel { id: String::new(), question: "Où en êtes-vous ?".into(), options: vec!["Fini".into(), "En cours".into(), "Bloqué".into()], multiple: false };
        e.launch_poll(&[], q, Some(5), T0).unwrap();
        assert_eq!(e.library.polls.len(), 1, "enregistré dans la bibliothèque (SONDAGE-6)");
        let polls = |rec: &Recorder| -> Vec<(MemberId, Option<PollInfo>)> {
            rec.take().into_iter().filter_map(|(to, c)| match (to, c) {
                (Target::Members(m), Command::Poll { poll, .. }) => Some((m[0].clone(), poll)),
                _ => None,
            }).collect()
        };
        let sent = polls(&rec);
        assert_eq!(sent.len(), 3);
        let id = sent[0].1.as_ref().unwrap().id.clone();
        assert_eq!(sent[0].1.as_ref().unwrap().closes_in_ms, Some(5 * 60_000));

        let answer = |e: &mut Engine, m: &str, c: Vec<u16>| e.handle(ToConsole::FromAgent { from: m.into(), msg: AgentMsg::Answer { poll: id.clone(), choices: c } }, T0);
        answer(&mut e, "a", vec![0]);
        answer(&mut e, "b", vec![2, 0]);
        answer(&mut e, "b", vec![1]);
        answer(&mut e, "c", vec![9]);
        let v = e.view().session.unwrap().poll.unwrap();
        assert_eq!((v.counts.clone(), v.answered, v.targets), (vec![1, 1, 0], 2, 3));
        assert_eq!(v.details.iter().map(|d| d.station.as_str()).collect::<Vec<_>>(), ["poste 01", "poste 02"]);
        assert_eq!(e.view().room.unwrap().stations.iter().map(|s| s.answered).collect::<Vec<_>>(), [Some(true), Some(true), Some(false)]);

        // Résultats montrés aux élèves : des comptes, aucun nom (SONDAGE-5).
        e.show_poll(true, T0).unwrap();
        let shown = polls(&rec);
        let text = serde_json::to_string(&shown).unwrap();
        assert_eq!(shown[0].1.as_ref().unwrap().results, Some(vec![1, 1, 0]));
        assert!(!text.contains("poste 0") && !text.contains("PC-"), "{text}");
        assert_eq!(shown.iter().find(|(m, _)| m == "a").unwrap().1.as_ref().unwrap().answer, Some(vec![0]));

        // Durée limite atteinte : clos ; une réponse tardive est ignorée.
        assert!(e.tick(T0 + 5 * 60_000));
        let closed = polls(&rec);
        assert!(closed.iter().all(|(_, p)| p.as_ref().is_some_and(|p| !p.open && p.results.is_some())));
        answer(&mut e, "c", vec![2]);
        assert_eq!(e.view().session.unwrap().poll.unwrap().answered, 2);
        e.show_poll(false, T0).unwrap();
        assert!(polls(&rec).iter().all(|(_, p)| p.is_none()), "clos et caché : plus rien chez l'élève");
    }
}
