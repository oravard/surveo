//! Prototype 2 de la phase 2 (IMPL.md) : capture d'écran et miniatures.
//! Code jetable : il sert à mesurer, pas à être repris tel quel.
//!
//! Usage : capture [--backend wayland|x11|windows] [--duration 30]
//!                 [--width 320] [--quality 70] [--interval-ms 1000]
//!                 [--out derniere-miniature.jpg] [--oublier-jeton]

mod thumb;
#[cfg(target_os = "linux")]
mod portal;
#[cfg(target_os = "linux")]
mod x11;
#[cfg(windows)]
mod windows;

use anyhow::Result;
use std::path::PathBuf;

pub struct Args {
    pub backend: String,
    pub duration: u64,
    pub width: u32,
    pub quality: u8,
    pub interval_ms: u64,
    pub out: PathBuf,
    pub forget: bool,
    pub continuous: bool,
}

fn arg(name: &str) -> Option<String> {
    let mut it = std::env::args();
    while let Some(a) = it.next() {
        if a == name {
            return it.next();
        }
    }
    None
}

fn args() -> Result<Args> {
    let default_backend = if cfg!(windows) {
        "windows"
    } else if std::env::var_os("WAYLAND_DISPLAY").is_some() {
        "wayland"
    } else {
        "x11"
    };
    Ok(Args {
        backend: arg("--backend").unwrap_or_else(|| default_backend.into()),
        duration: arg("--duration").map(|s| s.parse()).transpose()?.unwrap_or(30),
        width: arg("--width").map(|s| s.parse()).transpose()?.unwrap_or(320),
        quality: arg("--quality").map(|s| s.parse()).transpose()?.unwrap_or(70),
        interval_ms: arg("--interval-ms").map(|s| s.parse()).transpose()?.unwrap_or(1000),
        out: PathBuf::from(arg("--out").unwrap_or_else(|| "derniere-miniature.jpg".into())),
        forget: std::env::args().any(|a| a == "--oublier-jeton"),
        continuous: std::env::args().any(|a| a == "--continu"),
    })
}

#[cfg(target_os = "linux")]
#[tokio::main]
async fn main() -> Result<()> {
    let a = args()?;
    println!("Capture « {} » pendant {} s, miniature de {} px de large", a.backend, a.duration, a.width);
    match a.backend.as_str() {
        "wayland" => portal::run(&a).await,
        "x11" => x11::run(&a),
        other => anyhow::bail!("source de capture inconnue sous Linux : {other}"),
    }
}

#[cfg(windows)]
fn main() -> Result<()> {
    let a = args()?;
    println!("Capture « {} » pendant {} s, miniature de {} px de large", a.backend, a.duration, a.width);
    windows::run(&a)
}
