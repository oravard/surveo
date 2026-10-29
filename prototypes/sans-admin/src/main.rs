//! Prototype 3 de la phase 2 (IMPL.md) : installation, démarrage de
//! session, proxy utilisateur et filtrage, sans droits d'administration.
//! Code jetable : il sert à vérifier, pas à être repris tel quel.
//!
//! Commandes :
//!   sans-admin installer
//!   sans-admin filtrer bloquer|autoriser SITE,SITE…  [--duree 300] [--pac]
//!   sans-admin filtrer tout                          [--duree 300] [--pac]
//!   sans-admin demarrage      (lancé à l'ouverture de session)
//!   sans-admin etat
//!   sans-admin desinstaller

mod proxy;
#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
use linux as sys;
#[cfg(windows)]
mod windows;
#[cfg(windows)]
use windows as sys;

use anyhow::{bail, Context, Result};
use proxy::{Policy, Tally};
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Journal des modifications en cours (SPEC.md, RESTR-4) : écrit AVANT de
/// toucher au réglage, effacé APRÈS l'avoir rétabli. S'il existe au
/// démarrage, la séance précédente s'est mal terminée (RESTR-3).
#[derive(Serialize, Deserialize)]
struct Journal {
    origine: sys::Snapshot,
    port: u16,
    pac: bool,
    debut: u64,
    fin_prevue: u64,
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn journal_path() -> std::path::PathBuf {
    sys::data_dir().join("journal.json")
}

fn read_journal() -> Option<Journal> {
    serde_json::from_str(&std::fs::read_to_string(journal_path()).ok()?).ok()
}

/// Trace horodatée (secondes Unix) dans le dossier de l'agent, pour
/// vérifier après coup ce qui s'est passé à l'ouverture de session.
fn log(msg: &str) {
    let _ = std::fs::create_dir_all(sys::data_dir());
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(sys::data_dir().join("journal.log")) {
        let _ = writeln!(f, "[{}] {msg}", now());
    }
}

/// Rétablit le réglage d'origine et vérifie qu'il est identique.
fn restore(j: &Journal) -> Result<bool> {
    sys::restore(&j.origine)?;
    let after = sys::read()?;
    let same = after == j.origine;
    std::fs::remove_file(journal_path()).ok();
    Ok(same)
}

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("installer") => install(),
        Some("desinstaller") => uninstall(),
        Some("demarrage") => startup(),
        Some("etat") => status(),
        Some("filtrer") => filter(&args[1..]).await,
        _ => {
            println!("Commandes : installer · filtrer bloquer|autoriser SITES [--duree S] · filtrer tout · demarrage · etat · desinstaller");
            Ok(())
        }
    }
}

fn install() -> Result<()> {
    println!("Processus avec droits d'administration : {}", if sys::is_elevated() { "OUI" } else { "non" });
    let src = std::env::current_exe()?;
    let dest = sys::installed_exe();
    std::fs::create_dir_all(dest.parent().unwrap())?;
    if src != dest {
        std::fs::copy(&src, &dest).with_context(|| format!("copie vers {}", dest.display()))?;
    }
    println!("Agent copié dans {}", dest.display());
    let entry = sys::register_autostart(&dest)?;
    println!("Démarrage à l'ouverture de session inscrit : {entry}");
    log("installation");
    println!("Installation terminée, sans demande d'élévation.");
    Ok(())
}

fn uninstall() -> Result<()> {
    if let Some(j) = read_journal() {
        let same = restore(&j)?;
        println!("Réglage d'origine rétabli ({})", if same { "identique" } else { "DIFFÉRENT" });
    }
    sys::unregister_autostart()?;
    let _ = std::fs::remove_file(sys::data_dir().join("journal.log"));
    let exe = sys::installed_exe();
    match std::fs::remove_file(&exe) {
        Ok(()) => println!("Agent supprimé ({})", exe.display()),
        Err(_) => println!("Supprimez à la main {} s'il existe encore (programme en cours d'exécution)", exe.display()),
    }
    let _ = std::fs::remove_dir(sys::data_dir());
    println!("Démarrage de session désinscrit. Désinstallation terminée.");
    Ok(())
}

fn startup() -> Result<()> {
    match read_journal() {
        Some(j) => {
            let same = restore(&j)?;
            let msg = format!(
                "démarrage : séance précédente mal terminée (débutée à {}), réglage d'origine rétabli ({})",
                j.debut,
                if same { "identique" } else { "DIFFÉRENT" }
            );
            log(&msg);
            println!("{msg}");
        }
        None => {
            log("démarrage : rien à rétablir");
            println!("Démarrage : rien à rétablir.");
        }
    }
    Ok(())
}

fn status() -> Result<()> {
    println!("Processus avec droits d'administration : {}", if sys::is_elevated() { "OUI" } else { "non" });
    println!("Réglage de proxy actuel : {}", sys::read()?.describe());
    match read_journal() {
        Some(j) => println!(
            "Journal présent : une séance n'a pas été terminée proprement (proxy local sur le port {}). Origine : {}",
            j.port,
            j.origine.describe()
        ),
        None => println!("Aucun journal : aucune modification en cours."),
    }
    if let Ok(t) = std::fs::read_to_string(sys::data_dir().join("journal.log")) {
        println!("Dernières traces :");
        for l in t.lines().rev().take(5).collect::<Vec<_>>().into_iter().rev() {
            println!("  {l}");
        }
    }
    Ok(())
}

fn parse_policy(args: &[String]) -> Result<Policy> {
    let list = |i: usize| -> Vec<String> {
        args.get(i).map(|s| s.split(',').map(|x| x.trim().to_ascii_lowercase()).filter(|x| !x.is_empty()).collect()).unwrap_or_default()
    };
    Ok(match args.first().map(String::as_str) {
        Some("bloquer") => Policy::Block(list(1)),
        Some("autoriser") => Policy::Allow(list(1)),
        Some("tout") => Policy::All,
        _ => bail!("filtrer bloquer|autoriser SITE,SITE… ou filtrer tout"),
    })
}

async fn filter(args: &[String]) -> Result<()> {
    let policy = Arc::new(parse_policy(args)?);
    let pac = args.iter().any(|a| a == "--pac");
    let secs: u64 = args.iter().position(|a| a == "--duree").and_then(|i| args.get(i + 1)).map(|s| s.parse()).transpose()?.unwrap_or(300);

    if let Some(j) = read_journal() {
        let same = restore(&j)?;
        println!("Séance précédente mal terminée : réglage d'origine rétabli d'abord ({})", if same { "identique" } else { "DIFFÉRENT" });
    }

    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0)).await?;
    let port = listener.local_addr()?.port();
    let origine = sys::read()?;
    println!("Réglage d'origine : {}", origine.describe());

    let j = Journal { origine, port, pac, debut: now(), fin_prevue: now() + secs };
    std::fs::create_dir_all(sys::data_dir())?;
    std::fs::write(journal_path(), serde_json::to_string_pretty(&j)?)?;
    sys::apply_local(port, pac)?;
    log(&format!("filtrage posé sur le port {port} ({policy:?}), fin prévue dans {secs} s"));
    println!(
        "Proxy local 127.0.0.1:{port} posé {} · politique {policy:?} · fin prévue dans {secs} s",
        if pac { "par script PAC (repli direct si l'agent s'arrête)" } else { "en proxy fixe" }
    );
    println!("Ouvrez des sites dans Edge, Chrome ou Firefox. Ctrl+C pour terminer la séance avant l'heure.\n");

    let tally = Arc::new(Mutex::new(Tally::default()));
    tokio::spawn(proxy::serve(listener, policy.clone(), tally.clone()));

    // Surveillance du contournement (SPEC.md, MODE-3).
    let watch = tokio::spawn(async move {
        let mut was_ours = true;
        loop {
            tokio::time::sleep(Duration::from_secs(2)).await;
            let now_ours = sys::is_ours(port, pac);
            if was_ours && !now_ours {
                let d = sys::read().map(|s| s.describe()).unwrap_or_default();
                println!("⚠ Sortie de restriction détectée : le réglage de proxy a été modifié ({d})");
                log("sortie de restriction détectée");
            } else if !was_ours && now_ours {
                println!("Réglage de proxy de la séance de nouveau en place.");
            }
            was_ours = now_ours;
        }
    });

    tokio::select! {
        _ = tokio::time::sleep(Duration::from_secs(secs)) => println!("\nFin prévue de la séance atteinte."),
        _ = tokio::signal::ctrl_c() => println!("\nSéance terminée par l'enseignant."),
    }
    watch.abort();

    let same = restore(&j)?;
    log(&format!("filtrage levé, réglage d'origine rétabli ({})", if same { "identique" } else { "DIFFÉRENT" }));
    println!("Réglage après rétablissement : {}", sys::read()?.describe());
    println!("Réglage d'origine rétabli à l'identique : {}", if same { "OK" } else { "ÉCHEC" });
    println!("\nConnexions vues par le proxy :");
    tally.lock().unwrap().print();
    Ok(())
}
