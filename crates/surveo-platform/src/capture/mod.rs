//! Capture de l'écran en miniatures JPEG, sans droits d'administration
//! (IMPL.md, 1.1) :
//! - Wayland : portail ScreenCast puis flux PipeWire ; l'accord de
//!   l'utilisateur est demandé une fois, puis mémorisé par un jeton de
//!   restauration ; le flux est suspendu entre deux miniatures ;
//! - X11 : lecture de l'écran racine ;
//! - Windows : bibliothèque xcap, écran principal.
//!
//! La capture tourne sur son propre fil d'exécution et s'arrête quand la
//! [`Capture`] est abandonnée.

#[cfg(target_os = "linux")]
mod wayland;
#[cfg(windows)]
mod windows;
#[cfg(target_os = "linux")]
mod x11;

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

pub struct CaptureOptions {
    /// Largeur de la miniature, en pixels.
    pub width: u32,
    pub quality: u8,
    pub interval: Duration,
    /// Fichier du jeton de restauration du portail (Wayland).
    pub token_file: PathBuf,
    /// Miniatures de démonstration plutôt que l'écran réel (essais).
    pub synthetic: Option<usize>,
}

impl CaptureOptions {
    /// Miniature de 320 pixels, une par seconde (IMPL.md, 1.1).
    pub fn thumbnails(token_file: PathBuf) -> Self {
        Self { width: 320, quality: 70, interval: Duration::from_secs(1), token_file, synthetic: None }
    }
}

#[derive(Debug)]
pub enum CaptureEvent {
    Frame(Vec<u8>),
    /// La capture a échoué ou a été refusée ; elle est arrêtée.
    Failed(String),
}

pub type Sink = Arc<dyn Fn(CaptureEvent) + Send + Sync>;

/// Capture en cours ; elle s'arrête quand on l'abandonne.
pub struct Capture {
    stop: Arc<AtomicBool>,
}

impl Drop for Capture {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

/// Démarre la capture ; `sink` reçoit les miniatures sur le fil de la
/// capture.
pub fn start(opts: CaptureOptions, sink: Sink) -> Capture {
    let stop = Arc::new(AtomicBool::new(false));
    let flag = stop.clone();
    std::thread::Builder::new()
        .name("surveo-capture".into())
        .spawn(move || {
            if let Some(seed) = opts.synthetic {
                return synthetic(seed, &opts, &flag, &sink);
            }
            if let Err(e) = run(&opts, &flag, &sink) {
                log::warn!("capture impossible : {e:#}");
                sink(CaptureEvent::Failed(format!("{e:#}")));
            }
        })
        .expect("fil de capture");
    Capture { stop }
}

#[cfg(target_os = "linux")]
fn run(opts: &CaptureOptions, stop: &Arc<AtomicBool>, sink: &Sink) -> anyhow::Result<()> {
    if crate::env_set("WAYLAND_DISPLAY") {
        wayland::run(opts, stop, sink)
    } else if crate::env_set("DISPLAY") {
        x11::run(opts, stop, sink)
    } else {
        anyhow::bail!("aucun affichage graphique")
    }
}

#[cfg(windows)]
fn run(opts: &CaptureOptions, stop: &Arc<AtomicBool>, sink: &Sink) -> anyhow::Result<()> {
    windows::run(opts, stop, sink)
}

#[cfg(not(any(target_os = "linux", windows)))]
fn run(_: &CaptureOptions, _: &Arc<AtomicBool>, _: &Sink) -> anyhow::Result<()> {
    anyhow::bail!("système non pris en charge")
}

fn synthetic(seed: usize, opts: &CaptureOptions, stop: &AtomicBool, sink: &Sink) {
    let mut frame = 0;
    while !stop.load(Ordering::Relaxed) {
        let t0 = Instant::now();
        frame += 1;
        sink(CaptureEvent::Frame(crate::thumb::synthetic(seed, frame, opts.width, opts.quality)));
        if !sleep_unless_stopped(opts.interval.saturating_sub(t0.elapsed()), stop) {
            break;
        }
    }
}

/// Attend `d`, par petites tranches ; rend `false` si l'arrêt est demandé.
#[cfg_attr(not(any(target_os = "linux", windows)), allow(dead_code))]
fn sleep_unless_stopped(d: Duration, stop: &AtomicBool) -> bool {
    let end = Instant::now() + d;
    loop {
        if stop.load(Ordering::Relaxed) {
            return false;
        }
        let left = end.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return true;
        }
        std::thread::sleep(left.min(Duration::from_millis(100)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn synthetic_capture_delivers_jpeg_until_dropped() {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut opts = CaptureOptions::thumbnails(PathBuf::new());
        opts.synthetic = Some(3);
        opts.interval = Duration::from_millis(20);
        let tx = std::sync::Mutex::new(tx);
        let cap = start(opts, Arc::new(move |e| {
            let _ = tx.lock().unwrap().send(e);
        }));
        match rx.recv_timeout(Duration::from_secs(2)).unwrap() {
            CaptureEvent::Frame(j) => assert_eq!(&j[..2], &[0xFF, 0xD8]),
            e => panic!("{e:?}"),
        }
        drop(cap);
        std::thread::sleep(Duration::from_millis(200));
        while rx.try_recv().is_ok() {}
        std::thread::sleep(Duration::from_millis(100));
        assert!(rx.try_recv().is_err(), "la capture continue après l'arrêt");
    }
}
