//! Capture sous X11 : lecture directe de l'écran racine, sans accord
//! demandé à l'utilisateur.

use crate::thumb::{Order, Stats, Thumbnailer};
use crate::Args;
use anyhow::{bail, Result};
use std::time::{Duration, Instant};
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{ConnectionExt, ImageFormat};

pub fn run(args: &Args) -> Result<()> {
    let (conn, screen_num) = x11rb::connect(None)?;
    let screen = &conn.setup().roots[screen_num];
    let (w, h) = (screen.width_in_pixels, screen.height_in_pixels);
    let bpp = conn
        .setup()
        .pixmap_formats
        .iter()
        .find(|f| f.depth == screen.root_depth)
        .map(|f| f.bits_per_pixel)
        .unwrap_or(0);
    if bpp != 32 {
        bail!("profondeur d'écran non traitée par le prototype ({bpp} bits par pixel)");
    }
    println!("Écran X11 {w}×{h}, {} bits", screen.root_depth);

    let mut thumbs = Thumbnailer::new(args.width, args.quality);
    let mut stats = Stats::new("x11", args.out.clone());
    let started = Instant::now();
    let mut next_report = Instant::now() + Duration::from_secs(10);
    while started.elapsed() < Duration::from_secs(args.duration) {
        let t0 = Instant::now();
        let mut img = conn.get_image(ImageFormat::Z_PIXMAP, screen.root, 0, 0, w, h, !0)?.reply()?;
        let capture_ms = t0.elapsed().as_secs_f64() * 1000.0;
        stats.seen();
        // Pixels X11 en 32 bits petit-boutiste : ordre B, G, R, X.
        let t = thumbs.make(w as u32, h as u32, &mut img.data, Order::Bgra)?;
        stats.record(w as u32, h as u32, capture_ms, &t);
        if Instant::now() >= next_report {
            stats.report();
            next_report += Duration::from_secs(10);
        }
        std::thread::sleep(Duration::from_millis(args.interval_ms).saturating_sub(t0.elapsed()));
    }
    stats.summary();
    Ok(())
}
