//! Capture sous Windows avec la bibliothèque xcap (écran principal),
//! dans la session de l'utilisateur, sans droits d'administration.

use crate::thumb::{Order, Stats, Thumbnailer};
use crate::Args;
use anyhow::{anyhow, Result};
use std::time::{Duration, Instant};

pub fn run(args: &Args) -> Result<()> {
    let monitors = xcap::Monitor::all()?;
    let monitor = monitors
        .iter()
        .find(|m| m.is_primary().unwrap_or(false))
        .or_else(|| monitors.first())
        .ok_or_else(|| anyhow!("aucun écran trouvé"))?;
    println!("Écran Windows « {} »", monitor.name().unwrap_or_default());

    let mut thumbs = Thumbnailer::new(args.width, args.quality);
    let mut stats = Stats::new("windows", args.out.clone());
    let started = Instant::now();
    let mut next_report = Instant::now() + Duration::from_secs(10);
    while started.elapsed() < Duration::from_secs(args.duration) {
        let t0 = Instant::now();
        let img = monitor.capture_image()?;
        let (w, h) = (img.width(), img.height());
        let mut pixels = img.into_raw();
        let capture_ms = t0.elapsed().as_secs_f64() * 1000.0;
        stats.seen();
        let t = thumbs.make(w, h, &mut pixels, Order::Rgba)?;
        stats.record(w, h, capture_ms, &t);
        if Instant::now() >= next_report {
            stats.report();
            next_report += Duration::from_secs(10);
        }
        std::thread::sleep(Duration::from_millis(args.interval_ms).saturating_sub(t0.elapsed()));
    }
    stats.summary();
    Ok(())
}
