//! Linux : proxy de l'utilisateur par gsettings (schéma GNOME, lu par
//! les applications GNOME et, selon le bureau, par les navigateurs) et
//! démarrage de session par un fichier dans ~/.config/autostart.

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Command;

const KEYS: [(&str, &str); 7] = [
    ("org.gnome.system.proxy", "mode"),
    ("org.gnome.system.proxy", "autoconfig-url"),
    ("org.gnome.system.proxy", "ignore-hosts"),
    ("org.gnome.system.proxy.http", "host"),
    ("org.gnome.system.proxy.http", "port"),
    ("org.gnome.system.proxy.https", "host"),
    ("org.gnome.system.proxy.https", "port"),
];

/// Valeurs brutes des clés gsettings (texte GVariant), restituées telles quelles.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Snapshot {
    pub values: Vec<String>,
}

impl Snapshot {
    pub fn describe(&self) -> String {
        KEYS.iter()
            .zip(&self.values)
            .map(|((s, k), v)| format!("{}.{k}={v}", s.trim_start_matches("org.gnome.system.")))
            .collect::<Vec<_>>()
            .join(" · ")
    }
}

fn gsettings(args: &[&str]) -> Result<String> {
    let out = Command::new("gsettings").args(args).output().context("gsettings introuvable")?;
    if !out.status.success() {
        bail!("gsettings {:?} : {}", args, String::from_utf8_lossy(&out.stderr).trim());
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

pub fn read() -> Result<Snapshot> {
    Ok(Snapshot { values: KEYS.iter().map(|(s, k)| gsettings(&["get", s, k])).collect::<Result<_>>()? })
}

fn write(s: &Snapshot) -> Result<()> {
    // Les adresses avant le mode, pour ne jamais activer un proxy à moitié réglé.
    for i in [1, 2, 3, 4, 5, 6, 0] {
        let (schema, key) = KEYS[i];
        gsettings(&["set", schema, key, &s.values[i]])?;
    }
    Ok(())
}

pub fn ours(port: u16, pac: bool) -> Snapshot {
    Snapshot {
        values: vec![
            if pac { "'auto'".into() } else { "'manual'".into() },
            if pac { format!("'http://127.0.0.1:{port}/proxy.pac'") } else { "''".into() },
            "['localhost', '127.0.0.0/8', '::1']".into(),
            "'127.0.0.1'".into(),
            port.to_string(),
            "'127.0.0.1'".into(),
            port.to_string(),
        ],
    }
}

pub fn apply_local(port: u16, pac: bool) -> Result<()> {
    write(&ours(port, pac))
}

pub fn restore(s: &Snapshot) -> Result<()> {
    write(s)
}

pub fn is_ours(port: u16, pac: bool) -> bool {
    let o = ours(port, pac);
    read().map(|s| s.values[0] == o.values[0] && s.values[1] == o.values[1] && s.values[3] == o.values[3]).unwrap_or(false)
}

pub fn data_dir() -> PathBuf {
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".local/share"));
    base.join("surveo-proto")
}

pub fn installed_exe() -> PathBuf {
    data_dir().join("sans-admin")
}

fn autostart_file() -> PathBuf {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".config"));
    base.join("autostart").join("surveo-proto.desktop")
}

pub fn register_autostart(exe: &std::path::Path) -> Result<String> {
    let f = autostart_file();
    std::fs::create_dir_all(f.parent().unwrap())?;
    std::fs::write(
        &f,
        format!("[Desktop Entry]\nType=Application\nName=Surveo (prototype)\nExec=\"{}\" demarrage\nNoDisplay=true\nX-GNOME-Autostart-enabled=true\n", exe.display()),
    )?;
    Ok(f.display().to_string())
}

pub fn unregister_autostart() -> Result<()> {
    let _ = std::fs::remove_file(autostart_file());
    Ok(())
}

pub fn is_elevated() -> bool {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| s.lines().find(|l| l.starts_with("Uid:")).map(|l| l.split_whitespace().nth(2) == Some("0")))
        .unwrap_or(false)
}
