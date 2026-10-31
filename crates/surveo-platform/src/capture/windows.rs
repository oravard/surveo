//! Capture sous Windows avec la bibliothèque xcap (écran principal), dans
//! la session de l'utilisateur, sans droits d'administration (IMPL.md, 1.1).

use super::{sleep_unless_stopped, CaptureEvent, CaptureOptions, Sink};
use crate::thumb::{Order, Thumbnailer};
use anyhow::{anyhow, Result};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

pub fn run(opts: &CaptureOptions, stop: &AtomicBool, sink: &Sink) -> Result<()> {
    let monitors = xcap::Monitor::all()?;
    let monitor = monitors
        .iter()
        .find(|m| m.is_primary().unwrap_or(false))
        .or_else(|| monitors.first())
        .ok_or_else(|| anyhow!("aucun écran trouvé"))?;
    let mut thumbs = Thumbnailer::new(opts.width, opts.quality);
    while !stop.load(Ordering::Relaxed) {
        let t0 = Instant::now();
        let img = monitor.capture_image()?;
        let (w, h) = (img.width(), img.height());
        let mut pixels = img.into_raw();
        sink(CaptureEvent::Frame(thumbs.make(w, h, &mut pixels, Order::Rgba)?));
        if !sleep_unless_stopped(opts.interval.saturating_sub(t0.elapsed()), stop) {
            break;
        }
    }
    Ok(())
}
