//! Capacités propres à chaque système (IMPL.md, « Capacités par
//! plateforme ») : capture d'écran ([`capture`]), démarrage de session
//! ([`autostart`]), dossiers et description du poste.
//!
//! Tout fonctionne sans droits d'administration (SPEC.md, INST-6).

pub mod autostart;
pub mod capture;
mod thumb;

use std::path::PathBuf;

/// Composant de Surveo, pour le rangement de ses fichiers.
#[derive(Clone, Copy, Debug)]
pub enum Component {
    Agent,
    Console,
}

/// Dossier de l'utilisateur où un composant range son état et, pour
/// l'agent installé sans droits d'administration, son exécutable :
/// `~/.local/share/surveo/<composant>` sous Linux,
/// `%LOCALAPPDATA%\Surveo\<Composant>` sous Windows.
pub fn data_dir(c: Component) -> PathBuf {
    #[cfg(windows)]
    {
        let base = std::env::var_os("LOCALAPPDATA").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));
        base.join("Surveo").join(match c {
            Component::Agent => "Agent",
            Component::Console => "Console",
        })
    }
    #[cfg(not(windows))]
    {
        let base = std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".local/share"));
        base.join("surveo").join(match c {
            Component::Agent => "agent",
            Component::Console => "console",
        })
    }
}

/// Nom de l'ordinateur, qui ne sert que de libellé (SPEC.md, 1.1).
pub fn hostname() -> String {
    #[cfg(windows)]
    let name = std::env::var("COMPUTERNAME").ok();
    #[cfg(not(windows))]
    let name = std::fs::read_to_string("/proc/sys/kernel/hostname").ok().or_else(|| std::env::var("HOSTNAME").ok());
    name.map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).unwrap_or_else(|| "poste".into())
}

/// Système et, sous Linux, bureau et type de session.
pub fn os_label() -> String {
    #[cfg(windows)]
    {
        "Windows".into()
    }
    #[cfg(not(windows))]
    {
        let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
        let session = if env_set("WAYLAND_DISPLAY") {
            "Wayland"
        } else if env_set("DISPLAY") {
            "X11"
        } else {
            "sans affichage"
        };
        if desktop.is_empty() { format!("Linux ({session})") } else { format!("Linux ({session}, {desktop})") }
    }
}

/// Variable d'environnement présente et non vide. Une variable vide ne
/// désigne aucun affichage : sans ce test, `WAYLAND_DISPLAY=` ferait
/// passer la capture par le portail de la session Wayland.
#[cfg_attr(windows, allow(dead_code))]
pub(crate) fn env_set(name: &str) -> bool {
    std::env::var_os(name).is_some_and(|v| !v.is_empty())
}
