//! Rattachement, identité et installation du poste (SPEC.md, 7.2).
//!
//! L'agent range tout dans un seul dossier de l'utilisateur
//! ([`surveo_platform::data_dir`]) :
//!
//! | Fichier | Contenu |
//! |---|---|
//! | `surveo-rattachement.json` | paquet de rattachement (INST-3) |
//! | `identite.json` | identité du poste, signée par le paquet (secret) |
//! | `relais.json` | dernier relais joint (DECOUV-1, niveau 1) |
//! | `jeton-capture.txt` | jeton de restauration du portail (Wayland) |
//! | `remise-en-etat.json` | réglages modifiés pendant la séance et leur valeur d'origine (IMPL.md, 2.2) |
//! | `agent.lock` | verrou d'instance unique, avec le numéro du processus |
//!
//! Installé sans droits d'administration, l'exécutable y est copié et
//! inscrit au démarrage de session (INST-1).

use anyhow::{bail, Context, Result};
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use surveo_identity::{load_json, new_member_id, save_json, Identity, IdentityFile, Package};

pub const PACKAGE_FILE: &str = "surveo-rattachement.json";
const IDENTITY_FILE: &str = "identite.json";
const RELAY_FILE: &str = "relais.json";
const LOCK_FILE: &str = "agent.lock";
pub const TOKEN_FILE: &str = "jeton-capture.txt";
/// Nom de l'inscription au démarrage de session.
const AUTOSTART: &str = "surveo-agent";
/// Bibliothèque à livrer avec un exécutable Tauri sous Windows.
const WEBVIEW_LOADER: &str = "WebView2Loader.dll";

/// Cherche le paquet de rattachement dans le dossier de l'agent, puis à
/// côté de l'exécutable ; le copie dans le dossier de l'agent.
pub fn load_package(dir: &Path) -> Result<Package> {
    let own = dir.join(PACKAGE_FILE);
    if own.exists() {
        return load_json(&own);
    }
    let beside = exe_dir()?.join(PACKAGE_FILE);
    if !beside.exists() {
        bail!("fichier de rattachement {PACKAGE_FILE} introuvable");
    }
    let p: Package = load_json(&beside)?;
    save_json(&own, &p, true)?;
    Ok(p)
}

/// Identité du poste : relue, ou créée au premier démarrage et signée par
/// la clé d'installation du paquet. Une identité d'un autre établissement
/// ou d'une autre salle (nouveau paquet) est remplacée.
pub fn load_or_enroll(dir: &Path, package: &Package) -> Result<Identity> {
    let path = dir.join(IDENTITY_FILE);
    if let Ok(f) = load_json::<IdentityFile>(&path) {
        let cert = &f.credential.cert;
        if cert.etab == package.etab.id && cert.room.as_ref() == Some(&package.room) {
            return Identity::from_file(&f);
        }
        log::info!("nouveau rattachement : identité du poste renouvelée");
    }
    let id = Identity::enroll_agent(package, &new_member_id("poste"))?;
    save_json(&path, &id.to_file(), true)?;
    Ok(id)
}

pub fn last_relay(dir: &Path) -> Option<String> {
    load_json(&dir.join(RELAY_FILE)).ok()
}

pub fn save_last_relay(dir: &Path, addr: &str) {
    if last_relay(dir).as_deref() != Some(addr) {
        let _ = save_json(&dir.join(RELAY_FILE), &addr, false);
    }
}

fn exe_dir() -> Result<PathBuf> {
    let exe = std::env::current_exe()?;
    Ok(exe.parent().context("dossier de l'exécutable")?.to_path_buf())
}

fn exe_name() -> &'static str {
    if cfg!(windows) { "surveo-agent.exe" } else { "surveo-agent" }
}

/// L'exécutable en cours est-il celui du dossier de l'agent ?
pub fn is_installed(dir: &Path) -> bool {
    match (exe_dir(), dir.canonicalize()) {
        (Ok(a), Ok(b)) => a.canonicalize().map(|a| a == b).unwrap_or(false),
        _ => false,
    }
}

/// Installation sans droits d'administration (INST-1, INST-6) : copie de
/// l'exécutable et du paquet dans le dossier de l'agent, inscription au
/// démarrage de session. Rend le chemin de l'exécutable installé.
pub fn install(dir: &Path) -> Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let src = exe_dir()?;
    let dest = dir.join(exe_name());
    std::fs::copy(std::env::current_exe()?, &dest).with_context(|| format!("copie vers {}", dest.display()))?;
    if src.join(WEBVIEW_LOADER).exists() {
        std::fs::copy(src.join(WEBVIEW_LOADER), dir.join(WEBVIEW_LOADER))?;
    }
    if src.join(PACKAGE_FILE).exists() {
        let p: Package = load_json(&src.join(PACKAGE_FILE))?;
        save_json(&dir.join(PACKAGE_FILE), &p, true)?;
    }
    surveo_platform::autostart::register(AUTOSTART, "Surveo (agent)", &dest)?;
    Ok(dest)
}

/// Désinstallation (INST-5) : arrête l'agent en cours, retire
/// l'inscription au démarrage et efface le dossier de l'agent, donc le
/// rattachement.
pub fn uninstall(dir: &Path) -> Result<()> {
    surveo_platform::autostart::unregister(AUTOSTART)?;
    if let Ok(pid) = std::fs::read_to_string(dir.join(LOCK_FILE)) {
        if let Ok(pid) = pid.trim().parse::<u32>() {
            if pid != std::process::id() {
                stop_process(pid);
            }
        }
    }
    if !dir.exists() {
        return Ok(());
    }
    if cfg!(windows) && is_installed(dir) {
        // Un exécutable en cours ne peut pas s'effacer lui-même sous Windows.
        std::process::Command::new("cmd")
            .args(["/C", "ping", "127.0.0.1", "-n", "3", ">", "nul", "&", "rmdir", "/S", "/Q"])
            .arg(dir)
            .spawn()?;
        return Ok(());
    }
    std::thread::sleep(std::time::Duration::from_millis(500));
    std::fs::remove_dir_all(dir).with_context(|| format!("effacement de {}", dir.display()))
}

fn stop_process(pid: u32) {
    let status = if cfg!(windows) {
        std::process::Command::new("taskkill").args(["/PID", &pid.to_string(), "/F"]).status()
    } else {
        std::process::Command::new("kill").arg(pid.to_string()).status()
    };
    if let Err(e) = status {
        log::warn!("arrêt de l'agent {pid} impossible : {e}");
    }
}

/// Verrou d'instance unique : un seul agent par compte et par dossier.
/// Rend `None` si un autre agent tourne déjà.
pub fn lock(dir: &Path) -> Result<Option<File>> {
    std::fs::create_dir_all(dir)?;
    let mut f = std::fs::OpenOptions::new().create(true).truncate(false).write(true).open(dir.join(LOCK_FILE))?;
    match f.try_lock() {
        Ok(()) => {
            f.set_len(0)?;
            write!(f, "{}", std::process::id())?;
            Ok(Some(f))
        }
        Err(std::fs::TryLockError::WouldBlock) => Ok(None),
        Err(std::fs::TryLockError::Error(e)) => Err(e.into()),
    }
}
