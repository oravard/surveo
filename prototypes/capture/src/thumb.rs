//! Chaîne commune : image brute de l'écran → miniature réduite → JPEG,
//! avec la mesure de chaque étape et du CPU du processus.

use anyhow::Result;
use cpu_time::ProcessTime;
use fast_image_resize::images::Image;
use fast_image_resize::{FilterType, PixelType, ResizeAlg, ResizeOptions, Resizer};
use std::time::{Duration, Instant};

/// Ordre des canaux de l'image source (4 octets par pixel).
#[derive(Clone, Copy, Debug)]
pub enum Order {
    Bgra,
    Rgba,
}

pub struct Thumbnailer {
    pub width: u32,
    pub quality: u8,
    resizer: Resizer,
}

pub struct Thumb {
    pub jpeg: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub resize_ms: f64,
    pub encode_ms: f64,
}

impl Thumbnailer {
    pub fn new(width: u32, quality: u8) -> Self {
        Self { width, quality, resizer: Resizer::new() }
    }

    /// `pixels` : image source contiguë (sans marge en fin de ligne).
    pub fn make(&mut self, w: u32, h: u32, pixels: &mut [u8], order: Order) -> Result<Thumb> {
        let tw = self.width.min(w);
        let th = ((h as u64 * tw as u64) / w as u64).max(1) as u32;
        let t0 = Instant::now();
        let src = Image::from_slice_u8(w, h, pixels, PixelType::U8x4)?;
        let mut dst = Image::new(tw, th, PixelType::U8x4);
        self.resizer.resize(
            &src,
            &mut dst,
            &ResizeOptions::new().resize_alg(ResizeAlg::Convolution(FilterType::Bilinear)),
        )?;
        let t1 = Instant::now();
        let mut jpeg = Vec::with_capacity(32 * 1024);
        let color = match order {
            Order::Bgra => jpeg_encoder::ColorType::Bgra,
            Order::Rgba => jpeg_encoder::ColorType::Rgba,
        };
        jpeg_encoder::Encoder::new(&mut jpeg, self.quality).encode(dst.buffer(), tw as u16, th as u16, color)?;
        let t2 = Instant::now();
        Ok(Thumb {
            jpeg,
            width: tw,
            height: th,
            resize_ms: (t1 - t0).as_secs_f64() * 1000.0,
            encode_ms: (t2 - t1).as_secs_f64() * 1000.0,
        })
    }
}

/// Copie une image dont les lignes ont une marge (`stride` > largeur × 4)
/// dans un tampon contigu.
pub fn compact(src: &[u8], w: u32, h: u32, stride: usize) -> Vec<u8> {
    let row = w as usize * 4;
    let mut out = Vec::with_capacity(row * h as usize);
    for y in 0..h as usize {
        out.extend_from_slice(&src[y * stride..y * stride + row]);
    }
    out
}

/// Statistiques d'une session de capture.
pub struct Stats {
    pub source: String,
    pub out: std::path::PathBuf,
    start_wall: Instant,
    start_cpu: ProcessTime,
    win_wall: Instant,
    win_cpu: Duration,
    pub frames_seen: u64,
    win_frames_seen: u64,
    capture_ms: Vec<f64>,
    resize_ms: Vec<f64>,
    encode_ms: Vec<f64>,
    bytes: Vec<f64>,
    last_size: (u32, u32, u32, u32),
    compositor: Option<Compositor>,
    comp_start: u64,
    comp_win: u64,
}

impl Stats {
    pub fn new(source: &str, out: std::path::PathBuf) -> Self {
        let start_cpu = ProcessTime::now();
        Self {
            source: source.into(),
            out,
            start_wall: Instant::now(),
            win_cpu: start_cpu.elapsed(),
            start_cpu,
            win_wall: Instant::now(),
            frames_seen: 0,
            win_frames_seen: 0,
            capture_ms: vec![],
            resize_ms: vec![],
            encode_ms: vec![],
            bytes: vec![],
            last_size: (0, 0, 0, 0),
            // Le compositeur ne copie l'écran pour nous que sous Wayland.
            comp_start: Compositor::find().map(|c| c.ticks()).unwrap_or(0),
            comp_win: Compositor::find().map(|c| c.ticks()).unwrap_or(0),
            compositor: if source == "wayland" { Compositor::find() } else { None },
        }
    }

    pub fn seen(&mut self) {
        self.frames_seen += 1;
        self.win_frames_seen += 1;
    }

    pub fn record(&mut self, src_w: u32, src_h: u32, capture_ms: f64, t: &Thumb) {
        self.capture_ms.push(capture_ms);
        self.resize_ms.push(t.resize_ms);
        self.encode_ms.push(t.encode_ms);
        self.bytes.push(t.jpeg.len() as f64);
        self.last_size = (src_w, src_h, t.width, t.height);
        let _ = std::fs::write(&self.out, &t.jpeg);
    }

    /// Bilan de la fenêtre écoulée depuis le dernier appel.
    pub fn report(&mut self) {
        let cpu = self.start_cpu.elapsed();
        let dt = self.win_wall.elapsed().as_secs_f64();
        let pct = (cpu - self.win_cpu).as_secs_f64() / dt * 100.0;
        let n = self.resize_ms.len();
        let comp = match &self.compositor {
            Some(c) => {
                let now = c.ticks();
                let v = format!(" · compositeur {:.1} %", (now - self.comp_win) as f64 / 100.0 / dt * 100.0);
                self.comp_win = now;
                v
            }
            None => String::new(),
        };
        println!(
            "[{}] images reçues {:>5.1}/s · miniatures {} · CPU {:>4.1} % d'un cœur{}",
            self.source,
            self.win_frames_seen as f64 / dt,
            n,
            pct,
            comp
        );
        self.win_cpu = cpu;
        self.win_wall = Instant::now();
        self.win_frames_seen = 0;
    }

    /// Bilan final, comparé au critère du prototype (IMPL.md, phase 2).
    pub fn summary(&mut self) {
        let wall = self.start_wall.elapsed().as_secs_f64();
        let cpu_pct = self.start_cpu.elapsed().as_secs_f64() / wall * 100.0;
        let n = self.resize_ms.len();
        let (sw, sh, tw, th) = self.last_size;
        println!("\nBilan [{}] sur {:.0} s :", self.source, wall);
        if n == 0 {
            println!("  Aucune miniature produite.");
            return;
        }
        println!("  Écran {sw}×{sh} → miniature {tw}×{th}, {n} miniatures ({:.2}/s)", n as f64 / wall);
        println!(
            "  Par miniature (médiane) : capture {:.1} ms · réduction {:.1} ms · JPEG {:.1} ms · taille {:.1} Ko",
            median(&mut self.capture_ms),
            median(&mut self.resize_ms),
            median(&mut self.encode_ms),
            median(&mut self.bytes) / 1024.0
        );
        println!(
            "  CPU moyen du processus : {:.2} % d'un cœur → critère < 5 % : {}",
            cpu_pct,
            if cpu_pct < 5.0 { "OK" } else { "ÉCHEC" }
        );
        if let Some(c) = &self.compositor {
            println!(
                "  Compositeur « {} » pendant la capture : {:.1} % d'un cœur",
                c.name,
                (c.ticks() - self.comp_start) as f64 / 100.0 / wall * 100.0
            );
        }
        if self.source == "wayland" {
            println!("  Images envoyées par le compositeur : {:.1}/s", self.frames_seen as f64 / wall);
        }
        println!("  Dernière miniature : {}", self.out.display());
    }
}

pub fn median(v: &mut [f64]) -> f64 {
    if v.is_empty() {
        return f64::NAN;
    }
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[v.len() / 2]
}

/// Compositeur graphique (Linux) dont on suit la consommation, car c'est
/// lui qui copie l'écran pour nous sous Wayland.
pub struct Compositor {
    pub name: String,
    pid: u32,
}

#[cfg(target_os = "linux")]
impl Compositor {
    pub fn find() -> Option<Self> {
        const NAMES: [&str; 6] = ["cosmic-comp", "gnome-shell", "kwin_wayland", "sway", "Hyprland", "weston"];
        for e in std::fs::read_dir("/proc").ok()?.flatten() {
            let Ok(pid) = e.file_name().to_string_lossy().parse::<u32>() else { continue };
            if let Ok(comm) = std::fs::read_to_string(format!("/proc/{pid}/comm")) {
                let comm = comm.trim();
                if NAMES.contains(&comm) {
                    return Some(Self { name: comm.to_string(), pid });
                }
            }
        }
        None
    }

    /// Temps CPU cumulé, en centièmes de seconde.
    pub fn ticks(&self) -> u64 {
        let s = std::fs::read_to_string(format!("/proc/{}/stat", self.pid)).unwrap_or_default();
        let after = s.rsplit_once(')').map(|x| x.1).unwrap_or("");
        let f: Vec<&str> = after.split_whitespace().collect();
        let get = |i: usize| f.get(i).and_then(|v| v.parse::<u64>().ok()).unwrap_or(0);
        get(11) + get(12)
    }

    pub async fn cpu_over(&self, d: Duration) -> f64 {
        let a = self.ticks();
        tokio::time::sleep(d).await;
        (self.ticks() - a) as f64 / 100.0 / d.as_secs_f64() * 100.0
    }
}

#[cfg(not(target_os = "linux"))]
impl Compositor {
    pub fn find() -> Option<Self> {
        None
    }
    pub fn ticks(&self) -> u64 {
        let _ = (&self.name, self.pid);
        0
    }
}
