//! Capture sous X11 : lecture directe de l'écran racine, sans accord
//! demandé à l'utilisateur (IMPL.md, 1.1).

use super::{sleep_unless_stopped, CaptureEvent, CaptureOptions, Sink};
use crate::thumb::{Order, Thumbnailer};
use anyhow::{bail, Result};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{ConnectionExt, ImageFormat};

pub fn run(opts: &CaptureOptions, stop: &AtomicBool, sink: &Sink) -> Result<()> {
    let (conn, screen_num) = x11rb::connect(None)?;
    let screen = &conn.setup().roots[screen_num];
    let (w, h) = (screen.width_in_pixels, screen.height_in_pixels);
    let bpp = conn.setup().pixmap_formats.iter().find(|f| f.depth == screen.root_depth).map(|f| f.bits_per_pixel).unwrap_or(0);
    if bpp != 32 {
        bail!("profondeur d'écran non prise en charge ({bpp} bits par pixel)");
    }
    let mut thumbs = Thumbnailer::new(opts.width, opts.quality);
    while !stop.load(Ordering::Relaxed) {
        let t0 = Instant::now();
        let mut img = conn.get_image(ImageFormat::Z_PIXMAP, screen.root, 0, 0, w, h, !0)?.reply()?;
        // Pixels X11 en 32 bits petit-boutiste : ordre B, G, R, X.
        sink(CaptureEvent::Frame(thumbs.make(w as u32, h as u32, &mut img.data, Order::Bgra)?));
        if !sleep_unless_stopped(opts.interval.saturating_sub(t0.elapsed()), stop) {
            break;
        }
    }
    Ok(())
}
